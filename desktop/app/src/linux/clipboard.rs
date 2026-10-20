// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::{path::PathBuf, sync::Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Image {
    Png(Vec<u8>),
    Bitmap(Vec<u8>),
}

impl Image {
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        match self {
            Image::Png(bytes) => Ok(bytes.clone()),
            Image::Bitmap(_) => Err("not supported on Linux yet".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clip {
    Text { text: String, html: Option<String>, rtf: Option<String> },
    Image(Image),
    Private,
    Empty,
}

static FALLBACK_TEXT: Mutex<Option<String>> = Mutex::new(None);

pub fn start<F: Fn(Clip) + Send + Sync + 'static>(_on_copy: F) {}

pub fn read() -> Clip {
    match FALLBACK_TEXT.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        Some(text) => Clip::Text { text, html: None, rtf: None },
        None => Clip::Empty,
    }
}

pub fn read_for_paste() -> Clip {
    read()
}

pub fn read_image_png() -> Option<Vec<u8>> {
    None
}

pub fn write(text: &str) -> Result<(), String> {
    crate::mirror::note_remote_clipboard(text);
    *FALLBACK_TEXT.lock().unwrap_or_else(|e| e.into_inner()) = Some(text.to_owned());
    Ok(())
}

pub fn write_rich(text: &str, _html: Option<&str>, _rtf: Option<&str>) -> Result<(), String> {
    write(text)
}

pub fn write_private(text: &str) -> Result<(), String> {
    write(text)
}

pub fn write_image(_mime: &str, _bytes: &[u8]) -> Result<(), String> {
    Ok(())
}

pub fn read_files() -> Option<Vec<PathBuf>> {
    None
}

pub fn write_files(_paths: &[PathBuf]) -> bool {
    false
}
