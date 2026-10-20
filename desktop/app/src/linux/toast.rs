// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::path::PathBuf;

pub const AUMID: &str = "Nectarlink.Desktop";
pub const CALL_ANSWER: &str = "answer";
pub const CALL_DECLINE: &str = "decline";
pub const CALL_END: &str = "end";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToastEvent {
    Opened { device: String, key: String },
    Action { device: String, key: String, action: String },
    Reply { device: String, key: String, action: String, text: String },
    Dismissed { device: String, key: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub device: String,
    pub key: String,
    pub title: String,
    pub body: String,
    pub attribution: String,
    pub icon: Option<PathBuf>,
    pub image: Option<PathBuf>,
    pub actions: Vec<(String, String)>,
    pub reply: Option<(String, String)>,
    pub silent: bool,
    pub progress: Option<Progress>,
    pub call: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub status: String,
    pub value: Option<f64>,
    pub label: String,
}

pub fn enabled() -> bool {
    false
}
pub fn set_process_id() {}
pub fn start<F: Fn(ToastEvent) + Send + Sync + 'static>(_icon: PathBuf, _on_event: F) {}
pub fn show(_toast: Toast) {}
pub fn update_progress(_device: &str, _key: &str, _progress: Progress) {}
pub fn remove(_device: &str, _key: &str) {}
pub fn remove_device(_device: &str) {}
pub fn unregister() {}
