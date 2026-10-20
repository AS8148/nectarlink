// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::{path::Path, process::ExitCode};

pub const ARG: &str = "--widget-provider";
pub const WIDGET_ID: &str = "NectarlinkPhoneStatus";

pub fn write_state_snapshot(_data_dir: &Path) {}
pub fn run_provider(_data_dir: &Path) -> ExitCode {
    ExitCode::SUCCESS
}
