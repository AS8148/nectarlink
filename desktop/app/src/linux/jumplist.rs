// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::path::Path;

pub const ARG: &str = "--task";

pub fn register(_custom_data_dir: Option<&Path>) {}
pub fn unregister() {}
