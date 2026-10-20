// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::path::Path;

pub fn open_url(_url: &str) -> Result<(), String> {
    Ok(())
}
pub fn open_mailto(_address: &str) -> Result<(), String> {
    Ok(())
}
pub fn open_maps(_query: &str) -> Result<(), String> {
    Ok(())
}
pub fn is_url_scheme_registered(_scheme: &str) -> bool {
    false
}
pub fn open_file(_path: &Path) -> Result<(), String> {
    Ok(())
}
pub fn open_folder(_path: &Path) -> Result<(), String> {
    Ok(())
}
pub fn select_in_explorer(_path: &Path) -> Result<(), String> {
    Ok(())
}
pub fn run(_program: &Path, _arguments: &str) -> Result<(), String> {
    Ok(())
}
pub fn lock() -> Result<(), String> {
    Ok(())
}
pub fn lock_workstation() -> Result<(), String> {
    Ok(())
}
pub fn sleep() -> Result<(), String> {
    Ok(())
}
pub fn sleep_system() -> Result<(), String> {
    Ok(())
}
pub fn take_screenshot() -> Result<(), String> {
    Ok(())
}
pub fn launch_app(_path: &Path) -> Result<(), String> {
    Ok(())
}
pub fn launch_executable(_path: &Path, _args: &[&str]) -> Result<(), String> {
    Ok(())
}
pub fn run_command(_command: &str) -> Result<(), String> {
    Ok(())
}
pub fn recycle_file(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| e.to_string())
}
