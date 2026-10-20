// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use nectarlink_core::{DeviceId, MediaAction};

use crate::state::PlayerView;

#[derive(Debug, Clone)]
pub struct Shown {
    pub device: DeviceId,
    pub device_name: String,
    pub view: PlayerView,
}

pub fn start<F: Fn(DeviceId, String, MediaAction, Option<u64>) + Send + Sync + 'static>(_on_command: F) {}

pub fn show(_current: Option<Shown>) {}
