// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use nectarlink_core::DeviceId;

pub fn start(_device_id: DeviceId) {
    let _ = crate::automations::TRIGGER_PHONE_WALKED_AWAY;
}
pub fn stop() {}
