// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub fn encode_png(_bitmap: &Bitmap) -> io::Result<Vec<u8>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "not supported on Linux yet"))
}
pub fn encode_jpeg(_bitmap: &Bitmap, _quality: u8) -> io::Result<Vec<u8>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "not supported on Linux yet"))
}
pub fn decode(_bytes: &[u8]) -> io::Result<Bitmap> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "not supported on Linux yet"))
}
pub fn scale_to(bitmap: &Bitmap, _max: u32) -> Bitmap {
    bitmap.clone()
}
pub fn crop_square(bitmap: &Bitmap) -> Bitmap {
    bitmap.clone()
}
pub fn crop_square_jpeg(_bytes: &[u8], _max_side: u32, _quality: u8) -> io::Result<Vec<u8>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "not supported on Linux yet"))
}
pub fn bmp_from_dib(_dib: &[u8]) -> Option<Vec<u8>> {
    None
}
pub fn dibv5(_bitmap: &Bitmap) -> Vec<u8> {
    Vec::new()
}
