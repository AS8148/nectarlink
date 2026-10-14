// SPDX-License-Identifier: GPL-3.0-or-later
//! Paired Bluetooth device inspection helper (`docs/research/hfp.md`):
//! enumerates paired phone Bluetooth endpoints via `Windows.Devices.Bluetooth`
//! and `Windows.Devices.Enumeration` and inspects their link status.
//!
//! Note: Public Windows user-mode APIs do not expose a Bluetooth HFP
//! Hands-Free unit / SCO call-audio gateway for unpackaged apps; see
//! `docs/research/hfp.md` for details.

#![allow(dead_code)]

use std::sync::{Mutex, OnceLock};

use windows::{
    Devices::{
        Bluetooth::{BluetoothConnectionStatus, BluetoothDevice},
        Enumeration::DeviceInformation,
    },
    core::HSTRING,
};

/// Current Bluetooth pairing and link state for a phone endpoint in Windows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HfpStatus {
    /// Whether a paired Bluetooth endpoint for the phone is present in Windows.
    pub available: bool,
    /// Whether the Bluetooth link to this PC is currently connected.
    pub connected: bool,
    /// Display name of the paired Bluetooth endpoint, if available.
    pub device_name: String,
    /// Human-readable Bluetooth link status.
    pub detail: String,
}

static ACTIVE_CONNECTION: OnceLock<Mutex<Option<(String, BluetoothDevice)>>> = OnceLock::new();

fn active_slot() -> &'static Mutex<Option<(String, BluetoothDevice)>> {
    ACTIVE_CONNECTION.get_or_init(|| Mutex::new(None))
}

/// Selects the best matching `(device_id, device_name)` pair for `phone_name`.
///
/// Performs a case-insensitive substring match against `phone_name` when
/// provided, falling back to the first discovered endpoint if none match by name.
pub fn match_endpoint<'a>(
    endpoints: &'a [(String, String)],
    phone_name: Option<&str>,
) -> Option<&'a (String, String)> {
    if endpoints.is_empty() {
        return None;
    }
    if let Some(target) = phone_name.map(str::trim).filter(|s| !s.is_empty()) {
        let needle = target.to_ascii_lowercase();
        if let Some(found) = endpoints.iter().find(|(_, name)| {
            let hay = name.trim().to_ascii_lowercase();
            !hay.is_empty() && (hay.contains(&needle) || needle.contains(&hay))
        }) {
            return Some(found);
        }
    }
    endpoints.first()
}

fn enumerate_endpoints() -> windows::core::Result<Vec<(String, String)>> {
    let selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;
    let collection = DeviceInformation::FindAllAsyncAqsFilter(&selector)?.join()?;
    let mut out = Vec::new();
    for info in &collection {
        let id = info.Id()?.to_string_lossy();
        let name = info.Name().map(|n| n.to_string_lossy()).unwrap_or_default();
        if !id.is_empty() {
            out.push((id, name));
        }
    }
    Ok(out)
}

/// Probes whether a paired Bluetooth phone endpoint is available and/or
/// connected for `phone_name`.
pub fn probe_status(phone_name: Option<&str>) -> HfpStatus {
    if let Ok(mut guard) = active_slot().lock() {
        if let Some((name, dev)) = guard.as_ref()
            && dev.ConnectionStatus().ok() == Some(BluetoothConnectionStatus::Connected)
        {
            return HfpStatus {
                available: true,
                connected: true,
                device_name: name.clone(),
                detail: format!("Bluetooth connected ({name})."),
            };
        }
        *guard = None;
    }

    let endpoints = match enumerate_endpoints() {
        Ok(list) => list,
        Err(e) => {
            tracing::debug!(error = %e, "can't enumerate paired Bluetooth endpoints");
            Vec::new()
        }
    };

    if let Some((id, name)) = match_endpoint(&endpoints, phone_name) {
        let display =
            if name.trim().is_empty() { phone_name.unwrap_or("your phone").to_owned() } else { name.clone() };
        let connected = BluetoothDevice::FromIdAsync(&HSTRING::from(id))
            .ok()
            .and_then(|op| op.join().ok())
            .and_then(|dev| dev.ConnectionStatus().ok())
            == Some(BluetoothConnectionStatus::Connected);
        let detail = if connected {
            format!("Bluetooth connected ({display}).")
        } else {
            format!("Bluetooth paired with {display} (not currently connected).")
        };
        HfpStatus { available: true, connected, device_name: display, detail }
    } else {
        HfpStatus {
            available: false,
            connected: false,
            device_name: String::new(),
            detail: "No paired Bluetooth phone endpoint found in Windows.".into(),
        }
    }
}

/// Opens and retains a `BluetoothDevice` handle for `phone_name` (or the first
/// paired phone endpoint).
pub fn connect_audio(phone_name: Option<&str>) -> Result<HfpStatus, String> {
    if let Ok(guard) = active_slot().lock()
        && let Some((name, dev)) = guard.as_ref()
        && dev.ConnectionStatus().ok() == Some(BluetoothConnectionStatus::Connected)
    {
        return Ok(HfpStatus {
            available: true,
            connected: true,
            device_name: name.clone(),
            detail: format!("Bluetooth connected ({name})."),
        });
    }

    let endpoints = enumerate_endpoints().map_err(|e| e.to_string())?;
    let Some((id, name)) = match_endpoint(&endpoints, phone_name) else {
        return Err("No paired Bluetooth phone endpoint found in Windows.".into());
    };
    let display =
        if name.trim().is_empty() { phone_name.unwrap_or("your phone").to_owned() } else { name.clone() };

    let dev = BluetoothDevice::FromIdAsync(&HSTRING::from(id))
        .map_err(|e| format!("can't open Bluetooth device: {e}"))?
        .join()
        .map_err(|e| format!("can't resolve Bluetooth device: {e}"))?;

    // Request access and SDP records to query the Bluetooth link state.
    let _ = dev.RequestAccessAsync().and_then(|op| op.join());
    let _ = dev.SdpRecords();
    let connected = dev.ConnectionStatus().ok() == Some(BluetoothConnectionStatus::Connected);

    if let Ok(mut guard) = active_slot().lock() {
        *guard = Some((display.clone(), dev));
    }

    let detail = if connected {
        format!("Bluetooth connected ({display}).")
    } else {
        format!("Bluetooth paired with {display} (not currently connected).")
    };

    Ok(HfpStatus { available: true, connected, device_name: display, detail })
}

/// Releases any active `BluetoothDevice` handle held by this process.
pub fn disconnect_audio() {
    if let Ok(mut guard) = active_slot().lock() {
        *guard = None;
    }
}

/// Opens Windows Bluetooth & devices settings (`ms-settings:bluetooth`).
pub fn open_bluetooth_settings() {
    if let Err(e) = crate::win::shell::open_url("ms-settings:bluetooth") {
        tracing::warn!(error = %e, "can't open ms-settings:bluetooth");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_bluetooth_endpoint_by_case_insensitive_phone_name_or_falls_back() {
        let endpoints = vec![
            ("id-1".to_owned(), "Galaxy S24 Ultra".to_owned()),
            ("id-2".to_owned(), "Pixel 9 Pro".to_owned()),
        ];
        let matched = match_endpoint(&endpoints, Some("pixel 9")).unwrap();
        assert_eq!(matched.0, "id-2");
        assert_eq!(matched.1, "Pixel 9 Pro");

        let fallback = match_endpoint(&endpoints, Some("Unpaired iPhone")).unwrap();
        assert_eq!(fallback.0, "id-1");

        let first = match_endpoint(&endpoints, None).unwrap();
        assert_eq!(first.0, "id-1");

        let empty: Vec<(String, String)> = Vec::new();
        assert!(match_endpoint(&empty, Some("Pixel 9")).is_none());
    }

    #[test]
    fn probe_status_returns_structured_status_without_panicking() {
        let status = probe_status(Some("Pixel 9"));
        assert!(!status.detail.is_empty());
        disconnect_audio();
    }
}
