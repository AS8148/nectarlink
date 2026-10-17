// SPDX-License-Identifier: GPL-3.0-or-later
//! Phone as microphone (docs/protocol/mic.md), on this PC: asks a phone for
//! its microphone and plays it into an output device. Played into a virtual
//! audio cable (VB-Audio's CABLE Input, Voicemeeter), the cable's other end
//! is a microphone every app can pick; played to the speakers, it's for
//! listening in. A virtual microphone of NectarLink's own needs an audio
//! driver and is a later, separate step (GitHub #7).

use std::sync::{Arc, Mutex};

use nectarlink_core::{AudioOutputDevice, DeviceId, Error, LinkState, MicSink, MirrorAudioConfig, NodeEvent};

use crate::{
    bridge::app::{describe, show_message},
    core_host,
    state::Changes,
    win::audio_out::Player,
};

/// Where the phone's microphone is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Off,
    /// Asked; waiting for the phone's user to tap Start.
    Asking(DeviceId),
    /// Its sound is arriving.
    Live(DeviceId),
}

static PHASE: Mutex<Phase> = Mutex::new(Phase::Off);
/// The output device chosen in Settings (an endpoint ID), if any.
static OUTPUT: Mutex<Option<String>> = Mutex::new(None);

fn set_phase(phase: Phase) {
    let mut current = PHASE.lock().unwrap_or_else(|e| e.into_inner());
    if *current != phase {
        tracing::info!(?phase, "phone microphone");
        *current = phase;
        drop(current);
        core_host::host().hub.changed(Changes::MIC);
    }
}

pub fn phase() -> Phase {
    *PHASE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Reads the chosen output from the settings.
pub fn init(settings: &crate::settings::Settings) {
    *OUTPUT.lock().unwrap_or_else(|e| e.into_inner()) = settings.phone_mic_output.clone();
}

/// The output chosen in Settings, if any (kept when other settings save).
pub fn chosen_output() -> Option<String> {
    OUTPUT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Names that virtual audio cables give their playback end.
const CABLES: &[&str] = &["cable input", "vb-audio", "voicemeeter input", "virtual cable"];

/// The output to play into: the one chosen, while it's there; else a
/// virtual audio cable, if there is one; else `None` (the default output).
pub(crate) fn preferred(devices: &[AudioOutputDevice], chosen: Option<&str>) -> Option<String> {
    if let Some(id) = chosen
        && devices.iter().any(|d| d.id == id)
    {
        return Some(id.to_owned());
    }
    devices.iter().find(|d| is_cable(&d.name)).map(|d| d.id.clone())
}

/// Whether an output with this name is a virtual audio cable.
pub fn is_cable(name: &str) -> bool {
    let name = name.to_lowercase();
    CABLES.iter().any(|cable| name.contains(cable))
}

/// The output devices, and the one the microphone plays into.
pub fn outputs() -> (Vec<AudioOutputDevice>, Option<String>) {
    let devices = crate::win::audio::output_devices();
    let chosen = OUTPUT.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let output = preferred(&devices, chosen.as_deref())
        .or_else(|| devices.iter().find(|d| d.is_default).map(|d| d.id.clone()));
    (devices, output)
}

/// Plays the microphone into this output from now on (and the next time).
pub fn set_output(id: String) {
    *OUTPUT.lock().unwrap_or_else(|e| e.into_inner()) = Some(id.clone());
    let dir = &core_host::host().data_dir;
    let mut settings = crate::settings::Settings::load(dir);
    settings.phone_mic_output = Some(id);
    if let Err(e) = settings.save(dir) {
        tracing::warn!(error = %e, "can't save the microphone's output");
    }
    core_host::host().hub.changed(Changes::MIC);
}

/// The phone whose microphone to use: the one asked or listened to, else
/// the first online one that can lend it, else the first online one (which
/// says so if it can't).
pub fn phone() -> Option<(DeviceId, String)> {
    let current = match phase() {
        Phase::Asking(id) | Phase::Live(id) => Some(id),
        Phase::Off => None,
    };
    core_host::host().hub.read(|s| {
        let can = |id: &DeviceId| {
            s.matrices.get(id).and_then(|m| m.state("media.phone_mic"))
                == Some(nectarlink_core::FeatureState::Available)
        };
        s.devices
            .iter()
            .find(|d| Some(d.id) == current)
            .or_else(|| s.devices.iter().find(|d| matches!(d.link, LinkState::Online { .. }) && can(&d.id)))
            .or_else(|| s.devices.iter().find(|d| matches!(d.link, LinkState::Online { .. })))
            .map(|d| (d.id, d.info.name.clone()))
    })
}

/// Asks `phone` for its microphone; its user taps Start there.
pub fn start(phone: DeviceId) {
    let Some(node) = core_host::node() else { return };
    let name = core_host::host().hub.read(|s| s.name_of(&phone)).unwrap_or_else(|| "your phone".into());
    set_phase(Phase::Asking(phone));
    core_host::spawn(async move {
        match node.mic_start(phone).await {
            Ok(()) => show_message(format!("Tap Start on {name} to use its microphone.")),
            Err(e) => {
                set_phase(Phase::Off);
                show_message(match e {
                    Error::Unsupported => format!("Update NectarLink on {name} to use its microphone."),
                    Error::Denied => format!("The microphone is turned off for this PC on {name}."),
                    other => describe(&other),
                });
            }
        }
    });
}

/// Stops listening (and asking).
pub fn stop() {
    let (Phase::Asking(phone) | Phase::Live(phone)) = phase() else { return };
    set_phase(Phase::Off);
    if let Some(node) = core_host::node() {
        core_host::spawn(async move { node.mic_stop(phone).await });
    }
}

pub fn on_event(event: &NodeEvent) {
    match event {
        NodeEvent::Mic { device, on: true } => set_phase(Phase::Live(*device)),
        NodeEvent::Mic { device, on: false } if phase() == Phase::Live(*device) => set_phase(Phase::Off),
        NodeEvent::LinkChanged { device, link: LinkState::Offline { .. } }
        | NodeEvent::DeviceRemoved(device)
            if matches!(phase(), Phase::Asking(d) | Phase::Live(d) if d == *device) =>
        {
            set_phase(Phase::Off)
        }
        _ => {}
    }
}

/// Where a phone's microphone goes: [`Platform::mic_sink`](nectarlink_core::Platform::mic_sink).
pub fn sink(_peer: &DeviceId) -> Option<Arc<dyn MicSink>> {
    Some(Arc::new(Sink { player: Mutex::new(None) }))
}

struct Sink {
    player: Mutex<Option<Player>>,
}

impl MicSink for Sink {
    fn config(&self, config: MirrorAudioConfig) {
        let (_, output) = outputs();
        match Player::start_on(output, config.rate, u16::from(config.channels)) {
            Ok(player) => *self.player.lock().unwrap_or_else(|e| e.into_inner()) = Some(player),
            Err(e) => tracing::warn!(error = %e, "can't play the phone's microphone"),
        }
    }

    fn audio(&self, _time_us: u64, data: Vec<u8>) {
        if let Some(player) = self.player.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            player.push(data);
        }
    }

    fn ended(&self) {
        self.player.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str, is_default: bool) -> AudioOutputDevice {
        AudioOutputDevice { id: id.into(), name: name.into(), is_default }
    }

    #[test]
    fn a_virtual_cable_is_preferred_until_another_output_is_chosen() {
        let devices = [
            device("spk", "Speakers (Realtek Audio)", true),
            device("cable", "CABLE Input (VB-Audio Virtual Cable)", false),
        ];
        assert_eq!(preferred(&devices, None).as_deref(), Some("cable"));
        assert_eq!(preferred(&devices, Some("spk")).as_deref(), Some("spk"));
        // The chosen one was unplugged: the cable again.
        assert_eq!(preferred(&devices, Some("usb-headset")).as_deref(), Some("cable"));
        assert_eq!(
            preferred(&[device("vm", "Voicemeeter Input (VB-Audio Voicemeeter VAIO)", false)], None)
                .as_deref(),
            Some("vm")
        );
    }

    #[test]
    fn without_a_cable_it_is_the_default_output() {
        assert_eq!(preferred(&[device("spk", "Speakers", true)], None), None);
    }
}
