// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::path::PathBuf;

pub const CLSID_STR: &str = "{8E14549A-DB61-4309-AFA1-3578E927E935}";
pub const FRIENDLY_NAME: &str = "NectarLink Virtual Camera";
pub const REGISTER_ARG: &str = "--register-vcam";
pub const UNREGISTER_ARG: &str = "--unregister-vcam";

pub fn dll_path() -> Option<PathBuf> {
    None
}
pub fn is_registered() -> bool {
    false
}
pub fn ensure_registered() -> bool {
    false
}
pub fn register_hklm() -> Result<(), String> {
    Err("not supported on Linux yet".into())
}
pub fn unregister_hklm() -> Result<(), String> {
    Ok(())
}
pub fn run_elevated(_arg: &'static str, on_done: impl FnOnce(bool) + Send + 'static) {
    on_done(false);
}

#[derive(Debug)]
pub struct VirtualCameraHandle;

impl VirtualCameraHandle {
    pub fn start() -> Result<Self, String> {
        Err("not supported on Linux yet".into())
    }
}
