// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use nectarlink_core::{MediaAction, MediaError, MediaPlayer};

pub fn says_video(_id: &str) -> bool {
    false
}
pub fn start<F: Fn(Vec<MediaPlayer>) + Send + Sync + 'static>(_on_change: F) {}
pub fn command(_player: &str, _action: MediaAction, _position: Option<u64>) -> Result<(), MediaError> {
    Ok(())
}
pub fn pause_for_call() {}
pub fn resume_after_call() {}
