// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::path::Path;

pub fn get(_url: &str, _limit: usize) -> Result<Vec<u8>, String> {
    Err("not supported on Linux yet".into())
}
pub fn get_retrying(_url: &str, _limit: usize) -> Result<Vec<u8>, String> {
    Err("not supported on Linux yet".into())
}
pub fn download(
    _url: &str,
    _file: &Path,
    _limit: u64,
    _progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    Err("not supported on Linux yet".into())
}
pub fn get_json(_url: &str) -> Result<String, String> {
    Err("not supported on Linux yet".into())
}
pub fn download_to_file(_url: &str, _dest: &Path, _on_progress: impl FnMut(u64, u64)) -> Result<(), String> {
    Err("not supported on Linux yet".into())
}
