// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux privilege check (stub).

use std::path::Path;

#[derive(Debug)]
pub enum Error {
    Declined,
    Other(String),
}

#[derive(Debug)]
pub struct Process;

impl Process {
    pub fn exit_code(&self) -> Option<u32> {
        Some(1)
    }
}

pub fn is_elevated() -> bool {
    // SAFETY: geteuid has no preconditions and is always safe to call on Linux.
    unsafe { libc::geteuid() == 0 }
}

pub fn start(_program: &Path, _params: &str) -> Result<Process, Error> {
    Err(Error::Other("elevated execution not supported on Linux yet".into()))
}

pub fn run_elevated(_exe: &Path, _args: &[&str]) -> Result<i32, String> {
    Err("elevated execution not supported on Linux yet".into())
}
