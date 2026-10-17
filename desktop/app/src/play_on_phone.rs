// SPDX-License-Identifier: GPL-3.0-or-later
//! Play on phone, on this PC (docs/protocol/media.md §4): moves what one of
//! its players plays to a phone. A protected streaming service opens on
//! the phone in its app and the PC pauses; anything else the phone watches
//! streamed from the PC, which keeps playing it.

#![allow(unsafe_code)]

use crate::i18n::tr;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use nectarlink_core::{DeviceId, Error, FeatureState, LinkState, MediaAction, PlayOnPhone};
use windows::Win32::{
    Foundation::{HWND, RECT},
    System::Com::{COINIT_MULTITHREADED, CoInitializeEx},
};

use crate::{
    bridge::app::{describe, show_message},
    core_host,
    win::browser::{self, Automation, Window},
};

/// The phone to play on: the first one online that can watch a PC's
/// playback, else the first one online.
pub fn phone() -> Option<(DeviceId, String)> {
    core_host::host().hub.read(|s| {
        let online: Vec<_> =
            s.devices.iter().filter(|d| matches!(d.link, LinkState::Online { .. })).collect();
        let watches = |id: &DeviceId| {
            s.matrices.get(id).and_then(|m| m.state("media.play_on_phone")) == Some(FeatureState::Available)
        };
        online.iter().find(|d| watches(&d.id)).or_else(|| online.first()).map(|d| (d.id, d.info.name.clone()))
    })
}

/// What this PC's `player` plays: its title, and where it is, in seconds.
fn now_playing(player: &str) -> (String, Option<u32>) {
    let views = crate::media::local_views();
    let Some(view) = views.into_iter().find(|v| v.player.id == player) else {
        return (String::new(), None);
    };
    let position = view.player.position.map(|ms| {
        let since =
            if view.player.playing { view.at.elapsed().map_or(0, |d| d.as_millis() as u64) } else { 0 };
        ((ms + since) / 1000) as u32
    });
    (view.player.title.unwrap_or_default(), position.filter(|&s| s > 0))
}

/// Plays what this PC's `player` plays on a phone (the Play on phone
/// button on the PC's media card).
pub fn play(player: String) {
    let Some((phone, name)) = phone() else {
        return show_message(tr!("PlayOnPhone", "Connect your phone to play on it."));
    };
    let spawned = std::thread::Builder::new().name("play-on-phone".into()).spawn(move || {
        // SAFETY: COM for this short-lived thread (UI Automation).
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let (title, position) = now_playing(&player);
        let window = browser::window_for(&player, &title);
        let url = window
            .as_ref()
            .filter(|w| browser::is_browser(&w.program))
            .and_then(|w| Automation::new().ok()?.page_url(w));
        let plan = nectarlink_core::play_on_phone::plan(url.as_deref(), position);
        let Some(node) = core_host::node() else { return };
        core_host::spawn(async move {
            match plan {
                PlayOnPhone::Open { url, service } => match node.open_link(phone, url).await {
                    Ok(()) => {
                        crate::media::command(crate::media::LOCAL_DEVICE, player, MediaAction::Pause, None);
                        show_message(tr!(
                            "PlayOnPhone",
                            "Opening in %1 on %2. Tap the notification there.",
                            service,
                            name
                        ));
                    }
                    Err(e) => show_message(failed(&e, &name)),
                },
                PlayOnPhone::Stream { open_instead } => {
                    let shown = (!title.is_empty()).then_some(title);
                    match node.offer_watch(phone, player.clone(), shown).await {
                        Ok(()) => {
                            show_message(tr!("PlayOnPhone", "Tap the notification on %1 to watch.", name))
                        }
                        // A phone that can't watch yet: the page opens there instead.
                        Err(Error::Unsupported) if let Some(url) = open_instead => {
                            match node.open_link(phone, url).await {
                                Ok(()) => {
                                    crate::media::command(
                                        crate::media::LOCAL_DEVICE,
                                        player,
                                        MediaAction::Pause,
                                        None,
                                    );
                                    show_message(tr!(
                                        "PlayOnPhone",
                                        "Sent to %1. Tap the notification there.",
                                        name
                                    ));
                                }
                                Err(e) => show_message(failed(&e, &name)),
                            }
                        }
                        Err(e) => show_message(failed(&e, &name)),
                    }
                }
            }
        });
    });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "can't start Play on phone");
    }
}

fn failed(error: &Error, phone: &str) -> String {
    match error {
        Error::Unsupported => tr!("PlayOnPhone", "Update NectarLink on %1 to play on it.", phone),
        Error::Denied => tr!("PlayOnPhone", "Media is turned off for %1.", phone),
        other => describe(other),
    }
}

/// Where a watched player's picture is, kept up to date while a phone
/// watches it: the video moves when the page scrolls, the window moves or
/// goes full screen.
pub struct Watched {
    place: Arc<Mutex<Option<Place>>>,
    stop: Arc<AtomicBool>,
}

/// A watched player's picture: its window, and where in it the picture is.
#[derive(Debug, Clone, Copy)]
pub struct Place {
    /// The window's handle (as a number: handles go between threads).
    window: isize,
    /// The picture, in physical screen pixels.
    pub on_screen: RECT,
    /// The picture, in the window's own pixels (from its frame's top-left),
    /// as a capture of the window has it.
    pub in_window: RECT,
}

/// How often the picture's place is looked up again.
const FOLLOW_EVERY: Duration = Duration::from_millis(500);
/// How long a browser's player may take to show up in UI Automation
/// before the whole page is streamed instead.
const FIND_PLAYER: Duration = Duration::from_secs(2);

impl Place {
    /// The window the picture is in.
    pub fn window(&self) -> HWND {
        HWND(self.window as *mut core::ffi::c_void)
    }
}

impl Watched {
    /// Starts following `player`'s picture.
    pub fn follow(player: &str) -> Watched {
        let place = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (found, stopped, player) = (place.clone(), stop.clone(), player.to_owned());
        let spawned = std::thread::Builder::new().name("play-on-phone-follow".into()).spawn(move || {
            // SAFETY: COM for this thread, which ends with the watching.
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            // Places in the pixels the capture uses.
            browser::use_physical_pixels();
            let automation = Automation::new().ok();
            let mut window: Option<Window> = None;
            let started = std::time::Instant::now();
            while !stopped.load(Ordering::Relaxed) {
                // The window it played in, while it's still there; else find it again.
                if window.as_ref().is_none_or(|w| browser::frame_of(w.hwnd).is_none()) {
                    let (title, _) = now_playing(&player);
                    window = browser::window_for(&player, &title);
                }
                let place = window.as_ref().and_then(|w| {
                    let frame = browser::frame_of(w.hwnd)?;
                    let on_screen = match &automation {
                        // A browser builds what UI Automation sees of a page
                        // only once asked: give the player a moment to show
                        // up before streaming the whole page.
                        Some(a) if browser::is_browser(&w.program) => match a.player_rect(w) {
                            Some(player) => player,
                            None if started.elapsed() < FIND_PLAYER => return None,
                            None => a.page_rect(w).unwrap_or(frame),
                        },
                        _ => frame,
                    };
                    let in_window = RECT {
                        left: on_screen.left - frame.left,
                        top: on_screen.top - frame.top,
                        right: on_screen.right - frame.left,
                        bottom: on_screen.bottom - frame.top,
                    };
                    Some(Place { window: w.hwnd.0 as isize, on_screen, in_window })
                });
                if place.is_some() {
                    *found.lock().unwrap_or_else(|e| e.into_inner()) = place;
                }
                std::thread::sleep(FOLLOW_EVERY);
            }
        });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "can't follow the watched player");
        }
        Watched { place, stop }
    }

    /// Where the picture is now, once known.
    pub fn place(&self) -> Option<Place> {
        *self.place.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
