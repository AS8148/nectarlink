// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux Wi-Fi scan and connection helper for Instant Hotspot via NetworkManager (`nmcli`).

use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InRange {
    pub profile: String,
    pub connectable: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    NotInRange,
    InRange,
    Connected,
}

fn tidy(mut names: Vec<String>) -> Vec<String> {
    names.retain(|n| !n.trim().is_empty());
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    names.dedup();
    names
}

fn parse_saved_profiles(stdout: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in stdout.lines() {
        // Format from `nmcli -t -f TYPE,NAME connection show`:
        // 802-11-wireless:MyHotspot
        if let Some((kind, name)) = line.split_once(':')
            && (kind == "802-11-wireless" || kind == "wifi")
        {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                names.push(trimmed.to_string());
            }
        }
    }
    tidy(names)
}

pub fn saved_networks() -> Vec<String> {
    let Ok(out) = Command::new("nmcli").args(["-t", "-f", "TYPE,NAME", "connection", "show"]).output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_saved_profiles(&String::from_utf8_lossy(&out.stdout))
}

pub fn scan() {
    let _ = Command::new("nmcli").args(["device", "wifi", "rescan"]).output();
}

fn parse_in_range(stdout: &str) -> Vec<InRange> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        // Format from `nmcli -t -f ACTIVE,SSID device wifi list`:
        // yes:MyHotspot
        if let Some((active, ssid)) = line.split_once(':') {
            let ssid = ssid.trim();
            if !ssid.is_empty() {
                out.push(InRange {
                    profile: ssid.to_string(),
                    connectable: true,
                    connected: active.trim() == "yes",
                });
            }
        }
    }
    out
}

fn reach_in(networks: &[InRange], profile: &str) -> Reach {
    let mine = networks.iter().filter(|n| n.profile == profile);
    if mine.clone().any(|n| n.connected) {
        Reach::Connected
    } else if mine.clone().any(|n| n.connectable) {
        Reach::InRange
    } else {
        Reach::NotInRange
    }
}

pub fn reach(profile: &str) -> Reach {
    let Ok(out) = Command::new("nmcli").args(["-t", "-f", "ACTIVE,SSID", "device", "wifi", "list"]).output()
    else {
        return Reach::NotInRange;
    };
    if !out.status.success() {
        return Reach::NotInRange;
    }
    let networks = parse_in_range(&String::from_utf8_lossy(&out.stdout));
    reach_in(&networks, profile)
}

pub fn connect(profile: &str) -> Result<(), String> {
    let trimmed = profile.trim();
    if trimmed.is_empty() || trimmed.starts_with('-') || trimmed.chars().any(char::is_control) {
        return Err("invalid Wi-Fi profile name".into());
    }
    let out = Command::new("nmcli")
        .args(["connection", "up", "id", trimmed])
        .output()
        .map_err(|e| format!("failed to run nmcli: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Err(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("nmcli exited with {}", out.status)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_tidies_saved_wifi_profiles() {
        let sample =
            "802-11-wireless:Pixel 9a\n802-3-ethernet:Wired connection 1\nwifi:home\nwifi:Home\nwifi:home\n";
        let names = parse_saved_profiles(sample);
        assert_eq!(names, vec!["Home", "home", "Pixel 9a"]);
    }

    #[test]
    fn reach_in_reports_connected_in_range_or_not() {
        let sample = "yes:Home\nno:Pixel 9a\n";
        let list = parse_in_range(sample);
        assert_eq!(reach_in(&list, "Home"), Reach::Connected);
        assert_eq!(reach_in(&list, "Pixel 9a"), Reach::InRange);
        assert_eq!(reach_in(&list, "Office"), Reach::NotInRange);
    }
}
