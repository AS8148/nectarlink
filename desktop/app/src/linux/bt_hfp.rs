// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HfpStatus {
    pub available: bool,
    pub connected: bool,
    pub device_name: String,
    pub detail: String,
}

pub(crate) fn is_valid_bt_mac(mac: &str) -> bool {
    !mac.is_empty()
        && !mac.starts_with('-')
        && mac.bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'-')
}

pub(crate) fn parse_bluetoothctl_devices(output: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for raw_line in output.lines() {
        let line = raw_line.trim();
        let Some(rest) = line.strip_prefix("Device ") else {
            continue;
        };
        let rest = rest.trim();
        let mut parts = rest.splitn(2, char::is_whitespace);
        let Some(mac) = parts.next().map(str::trim).filter(|m| is_valid_bt_mac(m)) else {
            continue;
        };
        let name = parts.next().map(str::trim).unwrap_or_default().to_owned();
        out.push((mac.to_owned(), name));
    }
    out
}

pub(crate) fn parse_bluetoothctl_connected(info_output: &str) -> bool {
    info_output.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.strip_prefix("Connected:").map(|v| v.trim().eq_ignore_ascii_case("yes")).unwrap_or(false)
    })
}

pub fn probe_status(_phone_name: Option<&str>) -> HfpStatus {
    HfpStatus::default()
}
pub fn connect_audio(_phone_name: Option<&str>) -> Result<HfpStatus, String> {
    Ok(HfpStatus::default())
}
pub fn disconnect_audio() {}
pub fn open_bluetooth_settings() {}
