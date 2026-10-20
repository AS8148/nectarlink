// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::{path::PathBuf, time::Instant};

pub fn local_timestamp_filename(prefix: &str, ext: &str) -> String {
    format!("{prefix}.{ext}")
}

pub fn merge_sps_pps(_dest: &mut Vec<u8>, _incoming: &[u8]) {}

pub struct MirrorRecorder {
    started: Instant,
}

impl MirrorRecorder {
    pub fn new(
        _path: PathBuf,
        _width: u32,
        _height: u32,
        _fps: u32,
        _audio_cfg: Option<(u32, u8)>,
    ) -> MirrorRecorder {
        MirrorRecorder { started: Instant::now() }
    }
    pub fn started_at(&self) -> Instant {
        self.started
    }
    pub fn started_epoch_ms(&self) -> i64 {
        0
    }
    pub fn set_sps_pps(&mut self, _sps_pps: &[u8]) {}
    pub fn on_config(&mut self, _width: u32, _height: u32) {}
    pub fn on_audio_config(&mut self, _rate: u32, _channels: u8) {}
    pub fn on_packet(&mut self, _keyframe: bool, _time_us: u64, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }
    pub fn on_audio(&mut self, _time_us: u64, _pcm: &[u8]) -> Result<(), String> {
        Ok(())
    }
    pub fn finish(self) -> Result<PathBuf, String> {
        Err("not supported on Linux yet".into())
    }
}
