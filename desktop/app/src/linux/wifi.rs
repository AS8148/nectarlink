// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux Wi-Fi scan and connection helper for Instant Hotspot (stub).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    NotInRange,
    InRange,
    Connected,
}

pub fn saved_networks() -> Vec<String> {
    Vec::new()
}

pub fn scan() {}

pub fn reach(_profile: &str) -> Reach {
    Reach::NotInRange
}

pub fn connect(_profile: &str) -> Result<(), String> {
    Err("Wi-Fi hotspot connection not supported on Linux yet".into())
}
