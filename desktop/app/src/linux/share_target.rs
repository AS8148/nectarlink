// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::path::{Path, PathBuf};
use tokio::sync::mpsc::UnboundedSender;

pub const ARG: &str = "--share";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SharedPayload {
    pub files: Vec<PathBuf>,
    pub url: Option<String>,
    pub text: Option<String>,
}

impl SharedPayload {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.url.is_none() && self.text.is_none()
    }
}

pub fn register_listener(_tx: UnboundedSender<SharedPayload>) -> Result<(), String> {
    Ok(())
}
pub fn take_activated_share(_data_dir: &Path) -> Option<SharedPayload> {
    None
}
pub fn parse_targets(targets: &[String]) -> SharedPayload {
    let mut payload = SharedPayload::default();
    for raw in targets {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("mailto:")
            || lower.starts_with("tel:")
            || lower.starts_with("nectarlink:")
        {
            if payload.url.is_none() {
                payload.url = Some(trimmed.to_owned());
            }
        } else {
            payload.files.push(PathBuf::from(trimmed));
        }
    }
    payload
}
