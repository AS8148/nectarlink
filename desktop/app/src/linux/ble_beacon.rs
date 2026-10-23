// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux BLE proximity RSSI monitor via BlueZ D-Bus (`org.freedesktop.DBus.ObjectManager`).
//! Never spawns interactive `bluetoothctl` sessions or `scan on`, and matches only
//! paired NectarLink phones (or devices advertising the NectarLink BLE UUID /
//! manufacturer frame) when walk-away automation is enabled.

use std::collections::HashMap;
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nectarlink_core::DeviceId;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

pub const COMPANY_ID: u16 = 0xFFFF;
pub const MAGIC: &[u8; 5] = b"NCTR\x01";
pub const NEAR_RSSI_DBM: i16 = -72;
pub const WALK_AWAY_RSSI_DBM: i16 = -85;
pub const NECTARLINK_BLE_UUID: &str = "f81d4fae-7dec-11d0-a765-00a0c91e6bf6";

const POLL_INTERVAL: Duration = Duration::from_secs(2);

static RUNNING: AtomicBool = AtomicBool::new(false);
static STOP_MU: Mutex<bool> = Mutex::new(false);
static STOP_CV: Condvar = Condvar::new();
static WORKER: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

type ManagedObjects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedPhone {
    pub prefix: [u8; 8],
    pub name: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BluezDeviceCandidate {
    pub address: Option<String>,
    pub name: Option<String>,
    pub alias: Option<String>,
    pub rssi: Option<i16>,
    pub uuids: Vec<String>,
    pub mfg_payload: Option<Vec<u8>>,
}

#[derive(Debug, Default)]
pub struct RssiTracker {
    peers: HashMap<[u8; 8], PeerRssiState>,
}

#[derive(Debug, Clone, Default)]
struct PeerRssiState {
    samples: [i16; 4],
    count: usize,
    idx: usize,
    armed_near: bool,
}

impl RssiTracker {
    pub fn record(&mut self, prefix: [u8; 8], rssi: i16) -> bool {
        let state = self.peers.entry(prefix).or_default();
        state.samples[state.idx] = rssi;
        state.idx = (state.idx + 1) % state.samples.len();
        state.count = (state.count + 1).min(state.samples.len());
        if state.count < state.samples.len() {
            return false;
        }
        let sum: i32 = state.samples.iter().map(|&v| i32::from(v)).sum();
        let avg = (sum / (state.samples.len() as i32)) as i16;
        if avg >= NEAR_RSSI_DBM {
            state.armed_near = true;
            false
        } else if state.armed_near && avg <= WALK_AWAY_RSSI_DBM {
            state.armed_near = false;
            true
        } else {
            false
        }
    }
}

pub fn build_manufacturer_payload(device_id: &DeviceId) -> [u8; 13] {
    let mut out = [0u8; 13];
    out[0..5].copy_from_slice(MAGIC);
    out[5..13].copy_from_slice(&device_id.as_bytes()[0..8]);
    out
}

/// Validates the `NCTR\x01` magic header and extracts the peer device ID prefix.
/// Accepts either the 13-byte payload (5-byte magic + 8-byte prefix) or a 12-byte
/// payload (5-byte magic + 7-byte prefix, zero-padded in the 8th byte).
pub fn parse_manufacturer_payload(bytes: &[u8]) -> Option<[u8; 8]> {
    if bytes.len() < 12 || &bytes[0..5] != MAGIC {
        return None;
    }
    let mut prefix = [0u8; 8];
    if bytes.len() >= 13 {
        prefix.copy_from_slice(&bytes[5..13]);
    } else {
        prefix[0..7].copy_from_slice(&bytes[5..12]);
    }
    Some(prefix)
}

pub fn start(device_id: DeviceId) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    *STOP_MU.lock().unwrap_or_else(|e| e.into_inner()) = false;
    let handle = thread::Builder::new().name("ble-beacon".into()).spawn(move || run_worker(device_id)).ok();
    if let Ok(mut guard) = WORKER.lock() {
        *guard = handle;
    }
}

pub fn stop() {
    RUNNING.store(false, Ordering::SeqCst);
    {
        let mut stopped = STOP_MU.lock().unwrap_or_else(|e| e.into_inner());
        *stopped = true;
        STOP_CV.notify_all();
    }
    let handle = WORKER.lock().ok().and_then(|mut guard| guard.take());
    if let Some(h) = handle {
        let _ = h.join();
    }
}

fn walk_away_lock_enabled() -> bool {
    let rules = crate::settings::automation_rules();
    !crate::automations::matching_rules(&rules, crate::automations::TRIGGER_PHONE_WALKED_AWAY).is_empty()
}

fn paired_phones_from_hub() -> Vec<PairedPhone> {
    crate::core_host::host().hub.read(|s| {
        s.devices
            .iter()
            .map(|d| {
                let mut prefix = [0u8; 8];
                prefix.copy_from_slice(&d.id.as_bytes()[0..8]);
                PairedPhone { prefix, name: d.info.name.clone(), model: d.info.model.clone() }
            })
            .collect()
    })
}

fn fallback_peer_prefix(id_str: &str) -> [u8; 8] {
    let mut out = [0u8; 8];
    let bytes = id_str.as_bytes();
    let start = bytes.len().saturating_sub(8);
    for (dst, &src) in out.iter_mut().zip(&bytes[start..]) {
        *dst = src;
    }
    out
}

fn names_match(candidate: &str, phone_name: &str) -> bool {
    let c = candidate.trim();
    let p = phone_name.trim();
    !c.is_empty() && !p.is_empty() && c.eq_ignore_ascii_case(p)
}

/// Matches a BlueZ device candidate ONLY if:
/// 1. It carries a valid `NCTR\x01` manufacturer payload (not our own prefix), OR
/// 2. It advertises the NectarLink BLE service UUID (`f81d4fae-7dec-11d0-a765-00a0c91e6bf6`), OR
/// 3. Its `Name` or `Alias` matches a paired phone in `hub`.
///
/// Arbitrary Bluetooth peripherals (headphones, mice, keyboards, etc.) never match.
pub fn match_nectarlink_device(
    candidate: &BluezDeviceCandidate,
    own_prefix: &[u8; 8],
    paired_phones: &[PairedPhone],
) -> Option<[u8; 8]> {
    if let Some(mfg_bytes) = &candidate.mfg_payload
        && let Some(prefix) = parse_manufacturer_payload(mfg_bytes)
    {
        if &prefix == own_prefix {
            return None;
        }
        if paired_phones.is_empty() || paired_phones.iter().any(|p| p.prefix == prefix) {
            return Some(prefix);
        }
    }

    let find_matching_phone = || -> Option<&PairedPhone> {
        paired_phones.iter().find(|phone| {
            candidate.name.as_deref().is_some_and(|n| {
                names_match(n, &phone.name) || phone.model.as_deref().is_some_and(|m| names_match(n, m))
            }) || candidate.alias.as_deref().is_some_and(|a| {
                names_match(a, &phone.name) || phone.model.as_deref().is_some_and(|m| names_match(a, m))
            })
        })
    };

    if candidate.uuids.iter().any(|u| u.eq_ignore_ascii_case(NECTARLINK_BLE_UUID)) {
        if let Some(phone) = find_matching_phone() {
            return Some(phone.prefix);
        }
        if let Some(first) = paired_phones.first() {
            return Some(first.prefix);
        }
        if let Some(addr) = &candidate.address {
            let prefix = fallback_peer_prefix(addr);
            if &prefix != own_prefix {
                return Some(prefix);
            }
        }
        return None;
    }

    find_matching_phone().map(|p| p.prefix)
}

fn extract_string(val: &OwnedValue) -> Option<String> {
    <&str>::try_from(val).ok().map(str::to_owned)
}

fn extract_i16(val: &OwnedValue) -> Option<i16> {
    if let Ok(v) = i16::try_from(val) {
        return Some(v);
    }
    if let Ok(v) = i32::try_from(val) {
        return i16::try_from(v).ok();
    }
    None
}

fn extract_string_list(val: &OwnedValue) -> Vec<String> {
    if let Ok(list) = Vec::<String>::try_from(val.clone()) {
        return list;
    }
    let Value::Array(arr) = &**val else {
        return Vec::new();
    };
    arr.iter().filter_map(|item| <&str>::try_from(item).ok().map(str::to_owned)).collect()
}

fn extract_mfg_ffff(val: &OwnedValue) -> Option<Vec<u8>> {
    let Value::Dict(dict) = &**val else {
        return None;
    };
    for (k, v) in dict.iter() {
        let key_matches = u16::try_from(k).is_ok_and(|id| id == COMPANY_ID);
        if !key_matches {
            continue;
        }
        let inner = match v {
            Value::Value(inner) => inner.as_ref(),
            other => other,
        };
        if let Value::Array(bytes_arr) = inner {
            let mut out = Vec::with_capacity(bytes_arr.len());
            for item in bytes_arr.iter() {
                out.push(u8::try_from(item).ok()?);
            }
            return Some(out);
        }
    }
    None
}

fn is_adapter_powered(conn: &zbus::blocking::Connection) -> bool {
    if !super::dbus_name_has_owner(conn, "org.bluez") {
        return false;
    }
    let Ok(reply) = conn.call_method(
        Some("org.bluez"),
        "/",
        Some("org.freedesktop.DBus.ObjectManager"),
        "GetManagedObjects",
        &(),
    ) else {
        return false;
    };
    let Ok(objects) = reply.body().deserialize::<ManagedObjects>() else {
        return false;
    };
    objects.values().any(|ifaces| {
        ifaces
            .get("org.bluez.Adapter1")
            .and_then(|props| props.get("Powered"))
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false)
    })
}

fn poll_bluez_devices(conn: &zbus::blocking::Connection, own_prefix: &[u8; 8], tracker: &mut RssiTracker) {
    if !walk_away_lock_enabled() {
        return;
    }
    let paired_phones = paired_phones_from_hub();
    if paired_phones.is_empty() {
        return;
    }

    let Ok(reply) = conn.call_method(
        Some("org.bluez"),
        "/",
        Some("org.freedesktop.DBus.ObjectManager"),
        "GetManagedObjects",
        &(),
    ) else {
        return;
    };
    let Ok(objects) = reply.body().deserialize::<ManagedObjects>() else {
        return;
    };

    for ifaces in objects.values() {
        let Some(props) = ifaces.get("org.bluez.Device1") else {
            continue;
        };
        let Some(rssi) = props.get("RSSI").and_then(extract_i16) else {
            continue;
        };
        let candidate = BluezDeviceCandidate {
            address: props.get("Address").and_then(extract_string),
            name: props.get("Name").and_then(extract_string),
            alias: props.get("Alias").and_then(extract_string),
            rssi: Some(rssi),
            uuids: props.get("UUIDs").map(extract_string_list).unwrap_or_default(),
            mfg_payload: props.get("ManufacturerData").and_then(extract_mfg_ffff),
        };
        if let Some(prefix) = match_nectarlink_device(&candidate, own_prefix, &paired_phones)
            && tracker.record(prefix, rssi)
        {
            tracing::info!("BLE walk-away threshold reached for paired phone; firing trigger");
            crate::automations::fire_trigger(crate::automations::TRIGGER_PHONE_WALKED_AWAY);
        }
    }
}

fn run_worker(device_id: DeviceId) {
    let Ok(conn) = zbus::blocking::Connection::system() else {
        tracing::debug!("BLE beacon: system D-Bus unavailable; exiting worker");
        RUNNING.store(false, Ordering::SeqCst);
        return;
    };
    if !is_adapter_powered(&conn) {
        tracing::debug!("BLE beacon: no powered BlueZ adapter found; exiting worker");
        RUNNING.store(false, Ordering::SeqCst);
        return;
    }

    let mut own_prefix = [0u8; 8];
    own_prefix.copy_from_slice(&device_id.as_bytes()[0..8]);
    let mut tracker = RssiTracker::default();

    while RUNNING.load(Ordering::Relaxed) {
        poll_bluez_devices(&conn, &own_prefix, &mut tracker);

        let guard = STOP_MU.lock().unwrap_or_else(|e| e.into_inner());
        let _ = STOP_CV
            .wait_timeout_while(guard, POLL_INTERVAL, |stopped| !*stopped && RUNNING.load(Ordering::Relaxed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_13_byte_manufacturer_payload_with_magic_and_id_prefix() {
        let mut raw = [0u8; 32];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(3);
        }
        let id = DeviceId(raw);
        let payload = build_manufacturer_payload(&id);
        assert_eq!(payload.len(), 13);
        assert_eq!(&payload[0..5], b"NCTR\x01");
        assert_eq!(&payload[5..13], &raw[0..8]);
        assert_eq!(COMPANY_ID, 0xFFFF);
    }

    #[test]
    fn manufacturer_payload_roundtrips_through_parser() {
        let mut raw = [0u8; 32];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = (i as u8).wrapping_add(0x10);
        }
        let id = DeviceId(raw);
        let payload = build_manufacturer_payload(&id);
        let parsed = parse_manufacturer_payload(&payload).expect("valid payload should parse");
        assert_eq!(&parsed[..], &raw[0..8]);

        // Also supports 12-byte payloads (5-byte magic + 7-byte prefix).
        let parsed_12 = parse_manufacturer_payload(&payload[0..12]).expect("12-byte payload should parse");
        assert_eq!(&parsed_12[0..7], &raw[0..7]);
        assert_eq!(parsed_12[7], 0);
    }

    #[test]
    fn parser_rejects_short_or_wrong_magic_payloads() {
        assert_eq!(parse_manufacturer_payload(b""), None);
        assert_eq!(parse_manufacturer_payload(b"NCTR\x01short"), None);
        assert_eq!(parse_manufacturer_payload(b"NCTR\x0212345678"), None);
        assert_eq!(parse_manufacturer_payload(b"XXXX\x0112345678"), None);
    }

    #[test]
    fn rssi_tracker_fires_walk_away_only_after_armed_near() {
        let mut tracker = RssiTracker::default();
        let peer = [1, 2, 3, 4, 5, 6, 7, 8];
        for _ in 0..4 {
            assert!(!tracker.record(peer, -90));
        }
        for _ in 0..4 {
            assert!(!tracker.record(peer, -65));
        }
        assert!(!tracker.record(peer, -88));
        assert!(!tracker.record(peer, -90));
        assert!(!tracker.record(peer, -92));
        assert!(tracker.record(peer, -90));
        assert!(!tracker.record(peer, -92));
    }

    #[test]
    fn rssi_tracker_arms_at_near_threshold_and_fires_once_on_walk_away() {
        let mut tracker = RssiTracker::default();
        let peer = [9, 8, 7, 6, 5, 4, 3, 2];
        // Exactly at NEAR_RSSI_DBM (-72) for 4 samples arms the tracker.
        for _ in 0..4 {
            assert!(!tracker.record(peer, NEAR_RSSI_DBM));
        }
        // Dropping to WALK_AWAY_RSSI_DBM (-85) across 4 samples triggers once.
        assert!(!tracker.record(peer, WALK_AWAY_RSSI_DBM));
        assert!(!tracker.record(peer, WALK_AWAY_RSSI_DBM));
        assert!(!tracker.record(peer, WALK_AWAY_RSSI_DBM));
        assert!(tracker.record(peer, WALK_AWAY_RSSI_DBM));
        // Subsequent distant samples do not re-trigger without re-arming near.
        for _ in 0..4 {
            assert!(!tracker.record(peer, -95));
        }
    }

    #[test]
    fn matches_only_paired_phones_or_nectarlink_uuid_and_ignores_peripherals() {
        let own_prefix = [0xAA; 8];
        let phone_prefix = [1, 2, 3, 4, 5, 6, 7, 8];
        let paired = vec![PairedPhone {
            prefix: phone_prefix,
            name: "Pixel 9 Pro".into(),
            model: Some("Pixel 9 Pro".into()),
        }];

        // Arbitrary Bluetooth headphones / mice must NEVER match.
        let headphones = BluezDeviceCandidate {
            address: Some("11:22:33:44:55:66".into()),
            name: Some("Sony WH-1000XM5".into()),
            alias: Some("Sony WH-1000XM5".into()),
            rssi: Some(-90),
            uuids: vec!["0000110b-0000-1000-8000-00805f9b34fb".into()],
            mfg_payload: None,
        };
        assert_eq!(match_nectarlink_device(&headphones, &own_prefix, &paired), None);

        // Paired phone matched by Name/Alias.
        let by_name = BluezDeviceCandidate {
            address: Some("AA:BB:CC:DD:EE:FF".into()),
            name: Some("pixel 9 pro".into()),
            alias: None,
            rssi: Some(-65),
            uuids: Vec::new(),
            mfg_payload: None,
        };
        assert_eq!(match_nectarlink_device(&by_name, &own_prefix, &paired), Some(phone_prefix));

        // Device advertising the NectarLink BLE service UUID matches.
        let by_uuid = BluezDeviceCandidate {
            address: Some("AA:BB:CC:DD:EE:FF".into()),
            name: Some("Android".into()),
            alias: None,
            rssi: Some(-65),
            uuids: vec![NECTARLINK_BLE_UUID.to_uppercase()],
            mfg_payload: None,
        };
        assert_eq!(match_nectarlink_device(&by_uuid, &own_prefix, &paired), Some(phone_prefix));
    }

    #[test]
    fn start_and_stop_never_panic_when_ble_radio_is_absent_or_present() {
        let id = DeviceId([0x42; 32]);
        start(id);
        stop();
    }

    #[test]
    fn start_and_stop_are_idempotent_without_bluez() {
        let id = DeviceId([0x24; 32]);
        start(id);
        start(id);
        stop();
        stop();
    }
}
