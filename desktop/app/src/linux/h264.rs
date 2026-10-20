// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::io;

#[derive(Debug, Clone)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub plane_rows: usize,
    pub nv12: Vec<u8>,
    pub colors: Colors,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Colors {
    pub bt601: bool,
    pub full_range: bool,
}

pub struct Decoder;

impl Decoder {
    pub fn new() -> io::Result<Decoder> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not supported on Linux yet"))
    }
    pub fn new_for_codec(_codec: &str) -> io::Result<Decoder> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not supported on Linux yet"))
    }
    pub fn new_gpu(_codec: &str) -> io::Result<Decoder> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not supported on Linux yet"))
    }
    pub fn on_gpu(&self) -> bool {
        false
    }
    pub fn codec(&self) -> &str {
        "h264"
    }
    pub fn decode(&mut self, _data: &[u8], _time_us: u64) -> io::Result<Vec<Picture>> {
        Ok(Vec::new())
    }
    pub fn flush(&mut self) {}
}

pub fn to_bgrx(_picture: &Picture, _width: u32, _height: u32) -> Vec<u8> {
    Vec::new()
}
