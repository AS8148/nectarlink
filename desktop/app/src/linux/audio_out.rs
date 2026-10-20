// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_variables)]

use std::io;

pub struct Player;

impl Player {
    pub fn start(_rate: u32, _channels: u16) -> io::Result<Player> {
        Ok(Player)
    }
    pub fn start_on(_device: Option<String>, _rate: u32, _channels: u16) -> io::Result<Player> {
        Ok(Player)
    }
    pub fn start_for_device(_rate: u32, _channels: u16, _device_id: Option<&str>) -> io::Result<Player> {
        Ok(Player)
    }
    pub fn push(&self, _data: Vec<u8>) {}
}
