// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::{io, path::Path};

pub struct PrimaryInstance;

pub enum Instance {
    Primary(PrimaryInstance),
    Secondary,
}

pub fn acquire(_data_dir: &Path) -> io::Result<Instance> {
    Ok(Instance::Primary(PrimaryInstance))
}

pub fn wake(_data_dir: &Path) {}

impl PrimaryInstance {
    pub fn watch(&'static self, _on_activate: impl Fn() + Send + 'static) {}
}
