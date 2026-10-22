// SPDX-License-Identifier: GPL-3.0-or-later
//! Phone notifications as Freedesktop desktop notifications on Linux, with
//! action buttons, inline replies, progress updates, and D-Bus signal routing.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub const AUMID: &str = "Nectarlink.Desktop";
pub const CALL_ANSWER: &str = "answer";
pub const CALL_DECLINE: &str = "decline";
pub const CALL_END: &str = "end";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToastEvent {
    Opened { device: String, key: String },
    Action { device: String, key: String, action: String },
    Reply { device: String, key: String, action: String, text: String },
    Dismissed { device: String, key: String },
}

#[derive(Clone)]
pub struct Toast {
    pub device: String,
    pub key: String,
    pub title: String,
    pub body: String,
    pub attribution: String,
    pub icon: Option<PathBuf>,
    pub image: Option<PathBuf>,
    pub actions: Vec<(String, String)>,
    pub reply: Option<(String, String)>,
    pub silent: bool,
    pub progress: Option<Progress>,
    pub call: bool,
}

impl fmt::Debug for Toast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Toast")
            .field("device", &self.device)
            .field("key", &self.key)
            .field("title", &self.title)
            .field("body", &self.body)
            .field("attribution", &self.attribution)
            .field("icon", &self.icon)
            .field("image", &self.image)
            .field("actions", &self.actions.len())
            .field("reply", &self.reply.is_some())
            .field("silent", &self.silent)
            .field("progress", &self.progress)
            .field("call", &self.call)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub status: String,
    pub value: Option<f64>,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MonitorSignal {
    ActionInvoked { id: u32, action_key: String },
    NotificationReplied { id: u32, text: String },
    NotificationClosed { id: u32, reason: u32 },
}

#[derive(Clone, Debug)]
struct ToastSnapshot {
    title: String,
    body: String,
    attribution: String,
    icon_path: String,
    actions: Vec<(String, String)>,
    reply: Option<(String, String)>,
    silent: bool,
    call: bool,
}

struct Toaster {
    icon: PathBuf,
    on_event: Option<Box<dyn Fn(ToastEvent) + Send + Sync + 'static>>,
    active_ids: HashMap<(String, String), u32>,
    id_to_target: HashMap<u32, (String, String)>,
    reply_actions: HashMap<u32, String>,
    snapshots: HashMap<(String, String), ToastSnapshot>,
    next_fallback_id: u32,
}

impl Toaster {
    fn new(icon: PathBuf, on_event: Option<Box<dyn Fn(ToastEvent) + Send + Sync + 'static>>) -> Self {
        Self {
            icon,
            on_event,
            active_ids: HashMap::new(),
            id_to_target: HashMap::new(),
            reply_actions: HashMap::new(),
            snapshots: HashMap::new(),
            next_fallback_id: 100_000,
        }
    }

    fn record_shown(&mut self, device: String, key: String, id: u32, snapshot: ToastSnapshot) {
        let target = (device, key);
        if let Some(old_id) = self.active_ids.insert(target.clone(), id)
            && old_id != id
        {
            self.id_to_target.remove(&old_id);
            self.reply_actions.remove(&old_id);
        }
        self.id_to_target.insert(id, target.clone());
        if let Some((reply_action, _)) = &snapshot.reply {
            self.reply_actions.insert(id, reply_action.clone());
        } else {
            self.reply_actions.remove(&id);
        }
        self.snapshots.insert(target, snapshot);
    }

    fn take_by_target(&mut self, device: &str, key: &str) -> Option<u32> {
        let target = (device.to_string(), key.to_string());
        self.snapshots.remove(&target);
        if let Some(id) = self.active_ids.remove(&target) {
            self.id_to_target.remove(&id);
            self.reply_actions.remove(&id);
            Some(id)
        } else {
            None
        }
    }

    fn take_by_device(&mut self, device: &str) -> Vec<u32> {
        let keys: Vec<(String, String)> =
            self.active_ids.keys().filter(|(d, _)| d == device).cloned().collect();
        let mut ids = Vec::new();
        for target in keys {
            if let Some(id) = self.active_ids.remove(&target) {
                self.id_to_target.remove(&id);
                self.reply_actions.remove(&id);
                ids.push(id);
            }
            self.snapshots.remove(&target);
        }
        ids
    }

    fn clear_all(&mut self) -> Vec<u32> {
        let ids: Vec<u32> = self.active_ids.values().copied().collect();
        self.active_ids.clear();
        self.id_to_target.clear();
        self.reply_actions.clear();
        self.snapshots.clear();
        ids
    }

    fn handle_signal(&mut self, signal: MonitorSignal) -> Option<ToastEvent> {
        match signal {
            MonitorSignal::ActionInvoked { id, action_key } => {
                let (device, key) = self.id_to_target.get(&id)?.clone();
                if action_key == "default" || action_key.is_empty() {
                    Some(ToastEvent::Opened { device, key })
                } else {
                    Some(ToastEvent::Action { device, key, action: action_key })
                }
            }
            MonitorSignal::NotificationReplied { id, text } => {
                let (device, key) = self.id_to_target.get(&id)?.clone();
                let action =
                    self.reply_actions.get(&id).cloned().unwrap_or_else(|| "inline-reply".to_string());
                Some(ToastEvent::Reply { device, key, action, text })
            }
            MonitorSignal::NotificationClosed { id, reason } => {
                let (device, key) = self.id_to_target.remove(&id)?;
                self.active_ids.remove(&(device.clone(), key.clone()));
                self.reply_actions.remove(&id);
                self.snapshots.remove(&(device.clone(), key.clone()));
                // Freedesktop NotificationClosed reasons:
                // 1 = expired, 2 = dismissed by user, 3 = closed by CloseNotification, 4 = undefined
                if reason == 2 { Some(ToastEvent::Dismissed { device, key }) } else { None }
            }
        }
    }
}

static TOASTER: OnceLock<Mutex<Toaster>> = OnceLock::new();
static MONITOR_STARTED: OnceLock<()> = OnceLock::new();

pub fn enabled() -> bool {
    true
}

pub fn set_process_id() {}

pub fn start<F>(icon: PathBuf, on_event: F)
where
    F: Fn(ToastEvent) + Send + Sync + 'static,
{
    if cfg!(test) {
        if !icon.exists() {
            if let Some(parent) = icon.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = super::icon::write_png(&icon, 64);
        }
    } else {
        let bg_icon = icon.clone();
        let _ = std::thread::Builder::new().name("toast-icon".into()).spawn(move || {
            if !bg_icon.exists() {
                if let Some(parent) = bg_icon.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = super::icon::write_png(&bg_icon, 128);
            }
        });
    }

    let callback: Box<dyn Fn(ToastEvent) + Send + Sync + 'static> = Box::new(on_event);
    if let Some(mutex) = TOASTER.get() {
        if let Ok(mut guard) = mutex.lock() {
            guard.icon = icon;
            guard.on_event = Some(callback);
        }
    } else {
        let _ = TOASTER.set(Mutex::new(Toaster::new(icon, Some(callback))));
    }

    if !cfg!(test) {
        MONITOR_STARTED.get_or_init(|| {
            let _ = std::thread::Builder::new().name("toasts".into()).spawn(monitor_loop);
        });
    }
    crate::pc_notifications::start_listener();
}

pub fn show(toast: Toast) {
    if toast.device.trim().is_empty() || toast.key.trim().is_empty() {
        return;
    }

    let (replaces_id, default_icon) = {
        let Some(mutex) = TOASTER.get() else {
            let icon_str = toast
                .icon
                .as_ref()
                .or(toast.image.as_ref())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let title = format_summary(&toast.title, &toast.attribution);
            let body = format_body(&toast.body, &toast.attribution, toast.progress.as_ref());
            let _ = send_notification(
                0,
                &icon_str,
                &title,
                &body,
                &toast.actions,
                toast.reply.as_ref(),
                toast.progress.as_ref(),
                toast.silent,
                toast.call,
            );
            return;
        };
        let Ok(guard) = mutex.lock() else {
            return;
        };
        let existing = guard.active_ids.get(&(toast.device.clone(), toast.key.clone())).copied().unwrap_or(0);
        (existing, guard.icon.clone())
    };

    let chosen_icon = toast.icon.clone().or(toast.image.clone()).unwrap_or(default_icon);
    let icon_str = chosen_icon.to_string_lossy().to_string();
    let title = format_summary(&toast.title, &toast.attribution);
    let body = format_body(&toast.body, &toast.attribution, toast.progress.as_ref());

    let notified_id = send_notification(
        replaces_id,
        &icon_str,
        &title,
        &body,
        &toast.actions,
        toast.reply.as_ref(),
        toast.progress.as_ref(),
        toast.silent,
        toast.call,
    );

    if let Some(mutex) = TOASTER.get()
        && let Ok(mut guard) = mutex.lock()
    {
        let id = notified_id.unwrap_or_else(|| {
            if replaces_id != 0 {
                replaces_id
            } else {
                guard.next_fallback_id += 1;
                guard.next_fallback_id
            }
        });
        let snapshot = ToastSnapshot {
            title: toast.title,
            body: toast.body,
            attribution: toast.attribution,
            icon_path: icon_str,
            actions: toast.actions,
            reply: toast.reply,
            silent: toast.silent,
            call: toast.call,
        };
        guard.record_shown(toast.device, toast.key, id, snapshot);
    }
}

pub fn update_progress(device: &str, key: &str, progress: Progress) {
    let Some(mutex) = TOASTER.get() else {
        return;
    };
    let (replaces_id, snapshot) = {
        let Ok(guard) = mutex.lock() else {
            return;
        };
        let target = (device.to_string(), key.to_string());
        let Some(&id) = guard.active_ids.get(&target) else {
            return;
        };
        let Some(snap) = guard.snapshots.get(&target).cloned() else {
            return;
        };
        (id, snap)
    };

    let summary = format_summary(&snapshot.title, &snapshot.attribution);
    let body = format_body(&snapshot.body, &snapshot.attribution, Some(&progress));

    let new_id = send_notification(
        replaces_id,
        &snapshot.icon_path,
        &summary,
        &body,
        &snapshot.actions,
        snapshot.reply.as_ref(),
        Some(&progress),
        true,
        snapshot.call,
    );

    if let Some(id) = new_id
        && id != replaces_id
        && let Ok(mut guard) = mutex.lock()
    {
        guard.record_shown(device.to_string(), key.to_string(), id, snapshot);
    }
}

pub fn remove(device: &str, key: &str) {
    let Some(mutex) = TOASTER.get() else {
        return;
    };
    let id = {
        let Ok(mut guard) = mutex.lock() else {
            return;
        };
        guard.take_by_target(device, key)
    };
    if let Some(id) = id {
        close_notification(id);
    }
}

pub fn remove_device(device: &str) {
    let Some(mutex) = TOASTER.get() else {
        return;
    };
    let ids = {
        let Ok(mut guard) = mutex.lock() else {
            return;
        };
        guard.take_by_device(device)
    };
    for id in ids {
        close_notification(id);
    }
}

pub fn unregister() {
    let Some(mutex) = TOASTER.get() else {
        return;
    };
    let ids = {
        let Ok(mut guard) = mutex.lock() else {
            return;
        };
        guard.clear_all()
    };
    for id in ids {
        close_notification(id);
    }
}

fn format_summary(title: &str, attribution: &str) -> String {
    if !title.trim().is_empty() {
        title.trim().to_string()
    } else if !attribution.trim().is_empty() {
        attribution.trim().to_string()
    } else {
        "NectarLink".to_string()
    }
}

fn format_body(body: &str, attribution: &str, progress: Option<&Progress>) -> String {
    let mut parts = Vec::new();
    if !body.trim().is_empty() {
        parts.push(body.trim().to_string());
    }
    if let Some(prog) = progress {
        let mut prog_line = String::new();
        if !prog.status.trim().is_empty() {
            prog_line.push_str(prog.status.trim());
        }
        if !prog.label.trim().is_empty() {
            if !prog_line.is_empty() {
                prog_line.push_str(" — ");
            }
            prog_line.push_str(prog.label.trim());
        } else if let Some(val) = prog.value {
            let pct = (val.clamp(0.0, 1.0) * 100.0).round() as i32;
            if !prog_line.is_empty() {
                prog_line.push_str(&format!(" ({pct}%)"));
            } else {
                prog_line.push_str(&format!("{pct}%"));
            }
        }
        if !prog_line.is_empty() {
            parts.push(prog_line);
        }
    }
    if !attribution.trim().is_empty() {
        parts.push(attribution.trim().to_string());
    }
    parts.join("\n")
}

fn compute_progress_percent(progress: &Progress) -> i32 {
    progress.value.map(|v| (v.clamp(0.0, 1.0) * 100.0).round() as i32).unwrap_or(0)
}

fn build_actions_list(actions: &[(String, String)], reply: Option<&(String, String)>) -> Vec<String> {
    let mut items = Vec::with_capacity(2 + actions.len() * 2 + 2);
    items.push("default".to_string());
    items.push("Open".to_string());
    for (id, label) in actions {
        if !id.trim().is_empty() {
            items.push(id.clone());
            items.push(label.clone());
        }
    }
    if let Some((_action_id, placeholder)) = reply {
        let label = if placeholder.trim().is_empty() { "Reply" } else { placeholder.trim() };
        items.push("inline-reply".to_string());
        items.push(label.to_string());
    }
    items
}

fn build_hints_map(
    progress: Option<&Progress>,
    silent: bool,
    call: bool,
) -> HashMap<&'static str, zbus::zvariant::Value<'static>> {
    let mut hints = HashMap::new();
    let urgency: u8 = if call {
        2
    } else if silent {
        0
    } else {
        1
    };
    hints.insert("urgency", zbus::zvariant::Value::U8(urgency));
    if silent && !call {
        hints.insert("suppress-sound", zbus::zvariant::Value::Bool(true));
    }
    if let Some(prog) = progress {
        let pct = compute_progress_percent(prog);
        hints.insert("value", zbus::zvariant::Value::I32(pct));
    }
    hints
}

static DBUS_SESSION: OnceLock<Mutex<Option<zbus::blocking::Connection>>> = OnceLock::new();

fn with_session_connection<T, F>(mut op: F) -> Option<T>
where
    F: FnMut(&zbus::blocking::Connection) -> zbus::Result<T>,
{
    let mutex = DBUS_SESSION.get_or_init(|| Mutex::new(None));
    let mut guard = mutex.lock().ok()?;
    if guard.is_none() {
        *guard = zbus::blocking::Connection::session().ok();
    }
    if let Some(conn) = guard.as_ref() {
        match op(conn) {
            Ok(val) => return Some(val),
            Err(_) => {
                *guard = zbus::blocking::Connection::session().ok();
                if let Some(retry_conn) = guard.as_ref() {
                    return op(retry_conn).ok();
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn send_notification(
    replaces_id: u32,
    icon_path: &str,
    summary: &str,
    body: &str,
    actions: &[(String, String)],
    reply: Option<&(String, String)>,
    progress: Option<&Progress>,
    silent: bool,
    call: bool,
) -> Option<u32> {
    if cfg!(test) {
        return None;
    }

    let actions_list = build_actions_list(actions, reply);
    let hints_map = build_hints_map(progress, silent, call);
    let expire_timeout: i32 = if call { 0 } else { -1 };

    with_session_connection(|conn| {
        let reply_msg = conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &("NectarLink", replaces_id, icon_path, summary, body, &actions_list, &hints_map, expire_timeout),
        )?;
        let msg_body = reply_msg.body();
        msg_body.deserialize::<u32>().or_else(|_| msg_body.deserialize::<(u32,)>().map(|(id,)| id))
    })
}

fn close_notification(id: u32) {
    if cfg!(test) || id == 0 {
        return;
    }
    let _ = with_session_connection(|conn| {
        conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "CloseNotification",
            &(id,),
        )
        .map(|_| ())
    });
}

fn monitor_loop() {
    loop {
        let Ok(conn) = zbus::blocking::Connection::session() else {
            std::thread::sleep(Duration::from_secs(5));
            continue;
        };
        let Ok(rule) = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.freedesktop.Notifications")
            .and_then(|b| b.path("/org/freedesktop/Notifications"))
            .map(|b| b.build())
        else {
            return;
        };
        let Ok(iter) = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, None) else {
            std::thread::sleep(Duration::from_secs(5));
            continue;
        };

        for msg in iter {
            let Ok(msg) = msg else {
                break;
            };
            if let Some(signal) = parse_dbus_signal(&msg) {
                let event = TOASTER.get().and_then(|mutex| {
                    let mut guard = mutex.lock().ok()?;
                    guard.handle_signal(signal)
                });
                if let Some(event) = event
                    && let Some(mutex) = TOASTER.get()
                    && let Ok(guard) = mutex.lock()
                    && let Some(cb) = &guard.on_event
                {
                    cb(event);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// Deserializes a D-Bus signal from `org.freedesktop.Notifications` into a [`MonitorSignal`].
pub fn parse_dbus_signal(msg: &zbus::message::Message) -> Option<MonitorSignal> {
    let header = msg.header();
    if header.message_type() != zbus::message::Type::Signal {
        return None;
    }
    if header.interface().map(|i| i.as_str()) != Some("org.freedesktop.Notifications") {
        return None;
    }
    let member = header.member()?.as_str();
    match member {
        "ActionInvoked" => {
            let (id, action_key) = msg.body().deserialize::<(u32, String)>().ok()?;
            Some(MonitorSignal::ActionInvoked { id, action_key })
        }
        "NotificationReplied" => {
            let (id, text) = msg.body().deserialize::<(u32, String)>().ok()?;
            Some(MonitorSignal::NotificationReplied { id, text })
        }
        "NotificationClosed" => {
            let (id, reason) = msg.body().deserialize::<(u32, u32)>().ok()?;
            Some(MonitorSignal::NotificationClosed { id, reason })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_actions_and_hints_without_gvariant_text_mangling() {
        let actions = vec![("mark_read".to_string(), "Mark 'read' [1]".to_string())];
        let reply = ("direct_reply".to_string(), "Reply \"now\"".to_string());
        let list = build_actions_list(&actions, Some(&reply));
        assert_eq!(
            list,
            vec!["default", "Open", "mark_read", "Mark 'read' [1]", "inline-reply", "Reply \"now\""]
        );

        let prog = Progress { status: "Transferring".to_string(), value: Some(0.42), label: String::new() };
        let hints = build_hints_map(Some(&prog), true, false);
        assert_eq!(hints.get("urgency"), Some(&zbus::zvariant::Value::U8(0)));
        assert_eq!(hints.get("suppress-sound"), Some(&zbus::zvariant::Value::Bool(true)));
        assert_eq!(hints.get("value"), Some(&zbus::zvariant::Value::I32(42)));
    }

    #[test]
    fn parses_zbus_notification_signals() {
        let action_msg = zbus::message::Message::signal(
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            "ActionInvoked",
        )
        .unwrap()
        .build(&(15u32, "mark_read"))
        .unwrap();
        assert_eq!(
            parse_dbus_signal(&action_msg),
            Some(MonitorSignal::ActionInvoked { id: 15, action_key: "mark_read".to_string() })
        );

        let reply_msg = zbus::message::Message::signal(
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            "NotificationReplied",
        )
        .unwrap()
        .build(&(15u32, "Sounds good's!\nLine 2 [true]"))
        .unwrap();
        assert_eq!(
            parse_dbus_signal(&reply_msg),
            Some(MonitorSignal::NotificationReplied {
                id: 15,
                text: "Sounds good's!\nLine 2 [true]".to_string(),
            })
        );

        let closed_msg = zbus::message::Message::signal(
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            "NotificationClosed",
        )
        .unwrap()
        .build(&(15u32, 2u32))
        .unwrap();
        assert_eq!(
            parse_dbus_signal(&closed_msg),
            Some(MonitorSignal::NotificationClosed { id: 15, reason: 2 })
        );
    }

    #[test]
    fn toaster_tracks_lifecycle_and_routes_signals() {
        let mut toaster = Toaster::new(PathBuf::from("/tmp/icon.png"), None);
        let snap = ToastSnapshot {
            title: "Messages".to_string(),
            body: "Hi there".to_string(),
            attribution: "Messages · Pixel 9".to_string(),
            icon_path: "/tmp/icon.png".to_string(),
            actions: vec![("archive".to_string(), "Archive".to_string())],
            reply: Some(("direct_reply".to_string(), "Type a message".to_string())),
            silent: false,
            call: false,
        };

        toaster.record_shown("phone-1".to_string(), "notif-a".to_string(), 77, snap);

        assert_eq!(
            toaster.handle_signal(MonitorSignal::ActionInvoked { id: 77, action_key: "default".to_string() }),
            Some(ToastEvent::Opened { device: "phone-1".to_string(), key: "notif-a".to_string() })
        );

        assert_eq!(
            toaster.handle_signal(MonitorSignal::ActionInvoked { id: 77, action_key: "archive".to_string() }),
            Some(ToastEvent::Action {
                device: "phone-1".to_string(),
                key: "notif-a".to_string(),
                action: "archive".to_string(),
            })
        );

        assert_eq!(
            toaster
                .handle_signal(MonitorSignal::NotificationReplied { id: 77, text: "On my way".to_string() }),
            Some(ToastEvent::Reply {
                device: "phone-1".to_string(),
                key: "notif-a".to_string(),
                action: "direct_reply".to_string(),
                text: "On my way".to_string(),
            })
        );

        assert_eq!(
            toaster.handle_signal(MonitorSignal::NotificationClosed { id: 77, reason: 2 }),
            Some(ToastEvent::Dismissed { device: "phone-1".to_string(), key: "notif-a".to_string() })
        );

        assert!(toaster.active_ids.is_empty());
        assert!(toaster.id_to_target.is_empty());
    }
}
