// SPDX-License-Identifier: GPL-3.0-or-later
//! PC Notifications → Phone (§3.2 P2): forwards selected PC notifications to
//! connected phones when `forward_pc_notifications_to_phone` is enabled in
//! Settings (default `false`, privacy-first).

#[cfg(target_os = "linux")]
use std::sync::OnceLock;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use nectarlink_core::Notification;

use crate::{core_host, settings};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
#[cfg(target_os = "linux")]
static LISTENER_STARTED: OnceLock<()> = OnceLock::new();

/// Builds a sanitized [`Notification`] representing a PC notification.
pub fn build_pc_notification(
    key: &str,
    app_name: &str,
    title: &str,
    text: &str,
    when_ms: i64,
) -> Option<Notification> {
    let clean_app = if app_name.trim().is_empty() { "PC".to_owned() } else { app_name.trim().to_owned() };
    Notification {
        key: key.to_owned(),
        app: "pc.windows".to_owned(),
        app_name: clean_app,
        title: (!title.trim().is_empty()).then(|| title.trim().to_owned()),
        text: (!text.trim().is_empty()).then(|| text.trim().to_owned()),
        sub: None,
        when: when_ms,
        actions: Vec::new(),
        silent: false,
        icon: None,
        image: None,
        live: None,
        conversation: None,
    }
    .sanitized()
}

/// Returns `true` when the notification originated from NectarLink itself or a
/// blocked system self-source so we never create a forwarding loop.
#[cfg(any(target_os = "linux", test))]
pub fn is_blocked_self_source(app_name: &str) -> bool {
    let lower = app_name.trim().to_lowercase();
    lower.is_empty()
        || lower.contains("nectarlink")
        || lower == "notify-send"
        || lower == "org.freedesktop.notifications"
}

/// Parses a single-line or compact representation of a Freedesktop `Notify` D-Bus call
/// into `(app_name, summary, body)`.
///
/// Supports `dbus-monitor` `string "..."` fields from `org.freedesktop.Notifications.Notify`.
#[cfg(any(target_os = "linux", test))]
pub fn parse_dbus_notify_line(line: &str) -> Option<(String, String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut dbus_strings = Vec::new();
    let mut rest = trimmed;
    while let Some(idx) = rest.find("string \"") {
        let after = &rest[idx + "string \"".len()..];
        if let Some((val, tail)) = take_unescaped_quoted(after) {
            dbus_strings.push(val);
            rest = tail;
        } else {
            break;
        }
    }

    // In org.freedesktop.Notifications.Notify(app_name, replaces_id, app_icon, summary, body, ...):
    // string[0] = app_name, string[1] = app_icon, string[2] = summary, string[3] = body
    if dbus_strings.len() >= 4 {
        let app_name = dbus_strings[0].trim().to_string();
        let summary = dbus_strings[2].trim().to_string();
        let body = dbus_strings[3].trim().to_string();
        if summary.is_empty() && body.is_empty() {
            return None;
        }
        return Some((app_name, summary, body));
    }

    if dbus_strings.len() == 3 {
        let app_name = dbus_strings[0].trim().to_string();
        let summary = dbus_strings[1].trim().to_string();
        let body = dbus_strings[2].trim().to_string();
        if summary.is_empty() && body.is_empty() {
            return None;
        }
        return Some((app_name, summary, body));
    }

    None
}

#[cfg(any(target_os = "linux", test))]
fn take_unescaped_quoted(s: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut escaped = false;
    for (idx, ch) in s.char_indices() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some((out, &s[idx + 1..]));
        } else {
            out.push(ch);
        }
    }
    None
}

/// Forwards a captured PC notification if it is not from a blocked self-source,
/// has non-empty content, and `forward_pc_notifications_to_phone` is enabled.
#[cfg(any(target_os = "linux", test))]
pub fn forward_pc_toast(app_name: &str, title: &str, text: &str) -> bool {
    if is_blocked_self_source(app_name) {
        return false;
    }
    if title.trim().is_empty() && text.trim().is_empty() {
        return false;
    }
    forward_to_phones(app_name.trim(), title.trim(), text.trim())
}

/// Forwards a PC notification (`app_name`, `title`, `text`) to connected
/// phones when `forward_pc_notifications_to_phone` is enabled. Returns `true`
/// if the notification was queued for delivery.
pub fn forward_to_phones(app_name: &str, title: &str, text: &str) -> bool {
    if !settings::forward_pc_notifications_to_phone() {
        return false;
    }
    let seq = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let when_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0);
    let key = format!("pc:win:{when_ms}:{seq}");
    let Some(notification) = build_pc_notification(&key, app_name, title, text, when_ms) else {
        return false;
    };
    if let Some(node) = core_host::node() {
        core_host::spawn(async move {
            node.notification_posted(notification).await;
        });
    }
    true
}

/// Deserializes a Freedesktop `org.freedesktop.Notifications.Notify` D-Bus method call
/// message into `(app_name, summary, body)`, preserving multi-line notification bodies.
#[cfg(any(target_os = "linux", test))]
pub fn extract_notify_call(msg: &zbus::message::Message) -> Option<(String, String, String)> {
    let header = msg.header();
    if header.message_type() != zbus::message::Type::MethodCall {
        return None;
    }
    if header.interface().map(|i| i.as_str()) != Some("org.freedesktop.Notifications") {
        return None;
    }
    if header.member().map(|m| m.as_str()) != Some("Notify") {
        return None;
    }
    type NotifyBody = (
        String,
        u32,
        String,
        String,
        String,
        Vec<String>,
        std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        i32,
    );
    let (app_name, _replaces_id, _app_icon, summary, body, _actions, _hints, _timeout) =
        msg.body().deserialize::<NotifyBody>().ok()?;
    let app_name = app_name.trim().to_string();
    let summary = summary.trim().to_string();
    let body = body.trim().to_string();
    if summary.is_empty() && body.is_empty() {
        return None;
    }
    Some((app_name, summary, body))
}

/// Starts a background listener for desktop notifications on Linux via `zbus`.
#[cfg(target_os = "linux")]
pub fn start_listener() {
    LISTENER_STARTED.get_or_init(|| {
        if !cfg!(test) {
            let _ = std::thread::Builder::new().name("pc-notifications".into()).spawn(dbus_monitor_loop);
        }
    });
}

#[cfg(target_os = "linux")]
fn dbus_monitor_loop() {
    use std::time::Duration;

    loop {
        if !settings::forward_pc_notifications_to_phone() {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }

        let Ok(conn) = zbus::blocking::Connection::session() else {
            std::thread::sleep(Duration::from_secs(5));
            continue;
        };

        let rules = vec!["type='method_call',interface='org.freedesktop.Notifications',member='Notify'"];
        if conn
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus.Monitoring"),
                "BecomeMonitor",
                &(rules, 0u32),
            )
            .is_err()
        {
            std::thread::sleep(Duration::from_secs(5));
            continue;
        }

        for msg in zbus::blocking::MessageIterator::from(conn) {
            if !settings::forward_pc_notifications_to_phone() {
                break;
            }
            match msg {
                Ok(msg) => {
                    if let Some((app, title, body)) = extract_notify_call(&msg) {
                        let _ = forward_pc_toast(&app, &title, &body);
                    }
                }
                Err(_) => break,
            }
        }

        std::thread::sleep(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_to_phones_respects_privacy_toggle() {
        // Default is false (privacy-first), so forward_to_phones is a no-op.
        assert!(!settings::forward_pc_notifications_to_phone());
        assert!(!forward_to_phones("Explorer", "Download complete", "report.pdf"));

        // Verify payload construction when enabled.
        let notif = build_pc_notification(
            "pc:win:1",
            "Task Watcher",
            "Build finished",
            "cargo test succeeded",
            1_700_000_000_000,
        )
        .expect("valid PC notification");
        assert_eq!(notif.app, "pc.windows");
        assert_eq!(notif.app_name, "Task Watcher");
        assert_eq!(notif.title.as_deref(), Some("Build finished"));
        assert_eq!(notif.text.as_deref(), Some("cargo test succeeded"));
    }

    #[test]
    fn blocks_self_source_notifications() {
        assert!(is_blocked_self_source("NectarLink"));
        assert!(is_blocked_self_source("nectarlink-desktop"));
        assert!(is_blocked_self_source("notify-send"));
        assert!(is_blocked_self_source(""));
        assert!(!is_blocked_self_source("Firefox"));
        assert!(!is_blocked_self_source("Signal Desktop"));
    }

    #[test]
    fn parses_dbus_notify_lines() {
        let sample = r#"method call time=1700000.0 sender=:1.42 -> destination=org.freedesktop.Notifications serial=10 path=/org/freedesktop/Notifications; interface=org.freedesktop.Notifications; member=Notify string "Firefox" uint32 0 string "firefox" string "Download complete" string "ubuntu.iso" array [] array [] int32 -1"#;
        let parsed = parse_dbus_notify_line(sample);
        assert_eq!(
            parsed,
            Some(("Firefox".to_string(), "Download complete".to_string(), "ubuntu.iso".to_string()))
        );
    }

    #[test]
    fn forward_pc_toast_ignores_self_and_empty() {
        assert!(!forward_pc_toast("NectarLink", "Connected", "Pixel 9"));
        assert!(!forward_pc_toast("Firefox", "   ", ""));
    }

    #[test]
    fn extract_notify_call_preserves_multiline_body() {
        let hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> =
            std::collections::HashMap::new();
        let actions: Vec<&str> = vec!["default", "Open"];
        let msg = zbus::message::Message::method_call("/org/freedesktop/Notifications", "Notify")
            .unwrap()
            .interface("org.freedesktop.Notifications")
            .unwrap()
            .destination("org.freedesktop.Notifications")
            .unwrap()
            .build(&(
                "Signal Desktop",
                0u32,
                "signal-desktop",
                "Alice",
                "Hey there!\nAre we still on for 7pm?\nLet me know.",
                actions,
                hints,
                -1i32,
            ))
            .unwrap();

        let extracted = extract_notify_call(&msg);
        assert_eq!(
            extracted,
            Some((
                "Signal Desktop".to_string(),
                "Alice".to_string(),
                "Hey there!\nAre we still on for 7pm?\nLet me know.".to_string(),
            ))
        );
    }
}
