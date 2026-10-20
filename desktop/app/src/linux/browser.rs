// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables, non_camel_case_types, clippy::upper_case_acronyms)]

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HWND(pub *mut core::ffi::c_void);
unsafe impl Send for HWND {}
unsafe impl Sync for HWND {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub hwnd: HWND,
    pub program: String,
    pub title: String,
}

pub fn programs_for(_player: &str) -> Vec<String> {
    Vec::new()
}
pub fn is_browser(_program: &str) -> bool {
    false
}
pub fn windows() -> Vec<Window> {
    Vec::new()
}
pub fn window_for(_player: &str, _title: &str) -> Option<Window> {
    None
}
pub fn use_physical_pixels() {}

pub struct Automation;

impl Automation {
    pub fn new() -> Result<Automation, String> {
        Err("not supported on Linux yet".into())
    }
    pub fn page_url(&self, _window: &Window) -> Option<String> {
        None
    }
    pub fn player_rect(&self, _window: &Window) -> Option<RECT> {
        None
    }
    pub fn page_rect(&self, _window: &Window) -> Option<RECT> {
        None
    }
}

pub fn is_minimized(_hwnd: HWND) -> bool {
    false
}
pub fn restore_behind(_hwnd: HWND) {}
pub fn frame_of(_hwnd: HWND) -> Option<RECT> {
    None
}
