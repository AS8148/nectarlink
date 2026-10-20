// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::{io, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Firewall {
    pub on: bool,
    pub allowed: bool,
    pub blocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    pub name: String,
    pub trusted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Adapters {
    pub local_address: bool,
    pub vpn: bool,
}

pub fn firewall(_exe: &Path) -> io::Result<Firewall> {
    Ok(Firewall { on: false, allowed: true, blocked: false })
}
pub fn networks() -> io::Result<Vec<Network>> {
    Ok(Vec::new())
}
pub fn adapters() -> Adapters {
    Adapters { local_address: true, vpn: false }
}
pub fn fix_firewall(_exe: &Path) -> io::Result<bool> {
    Ok(false)
}
pub fn open_firewall_settings() -> bool {
    false
}
