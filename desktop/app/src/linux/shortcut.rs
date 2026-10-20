// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    pub target: PathBuf,
    pub arguments: String,
    pub description: String,
}

pub fn write(path: &Path, shortcut: &Shortcut) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{}\n{}", shortcut.target.display(), shortcut.arguments))
}

pub fn target(path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().next().map(PathBuf::from)
}

pub fn arguments(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().nth(1).map(str::to_owned)
}

pub fn startup_dir() -> Option<PathBuf> {
    None
}
pub fn send_to_dir() -> Option<PathBuf> {
    None
}
pub fn recent_dir() -> Option<PathBuf> {
    None
}
