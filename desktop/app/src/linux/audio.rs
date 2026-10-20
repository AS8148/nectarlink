// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use nectarlink_core::AudioOutputDevice;

pub fn speaker_state() -> Option<(u8, bool)> {
    None
}
pub fn set_speaker_volume(_level: u8) -> Result<(), String> {
    Ok(())
}
pub fn set_speaker_mute(_muted: bool) -> Result<(), String> {
    Ok(())
}
pub fn speaker_volume_up() -> Result<(u8, bool), String> {
    Ok((50, false))
}
pub fn speaker_volume_down() -> Result<(u8, bool), String> {
    Ok((50, false))
}
pub fn toggle_speaker_mute() -> Result<bool, String> {
    Ok(false)
}
pub fn mic_muted() -> Option<bool> {
    None
}
pub fn toggle_mic_mute() -> Result<bool, String> {
    Ok(false)
}
pub fn output_devices() -> Vec<AudioOutputDevice> {
    Vec::new()
}
pub fn set_default_output_device(_id: &str) -> Result<(), String> {
    Ok(())
}
pub fn watch(_changed: fn()) {}
