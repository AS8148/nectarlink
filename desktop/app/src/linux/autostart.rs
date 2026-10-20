// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::{io, path::Path};

pub fn command(exe: &Path) -> String {
    format!("\"{}\" --minimized", exe.display())
}
pub fn set(_enabled: bool, _exe: &Path) -> io::Result<()> {
    Ok(())
}
pub fn is_enabled() -> bool {
    false
}
pub fn set_enabled(_enabled: bool) -> Result<(), String> {
    Ok(())
}
pub fn is_installed(_exe: &Path) -> bool {
    false
}
