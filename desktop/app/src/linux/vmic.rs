// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

pub fn is_available() -> bool {
    false
}
pub fn cleanup_stale_sources() {}

pub struct VirtualMic;

impl VirtualMic {
    pub fn new_for_phone(_rate: u32, _input_channels: u8, _phone_name: Option<&str>) -> Self {
        Self
    }
    pub fn is_active(&self) -> bool {
        false
    }
    pub fn set_gain_db(&mut self, _gain_db: f32) {}
    pub fn set_muted(&mut self, _muted: bool) {}
    pub fn push_pcm(&mut self, _pcm: &[u8]) -> Result<f32, String> {
        Ok(0.0)
    }
}
