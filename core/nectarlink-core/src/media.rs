// SPDX-License-Identifier: MPL-2.0
//! Media control between devices (docs/protocol/media.md).
//!
//! A device that shares its players (`media.control`) keeps every allowed
//! device that shows them (`media.remote`) up to date: the full list when a
//! session starts or the user changes what that device may see, then after
//! each change. Artwork goes once per key and session. The other side turns
//! that into [`NodeEvent::MediaChanged`] and sends the user's commands back.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use nectarlink_protocol::{
    DeviceId, Envelope, ErrorCode,
    messages::{
        MediaCommand, MediaPlayer, MediaState, MediaWatch, media_actions, media_limits::PLAYERS, media_watch,
        types,
    },
};

use crate::{Error, Result, events::NodeEvent, node::Shared, session::Session};

/// How long a Play on phone offer stays open for the phone to take.
const WATCH_OFFER: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// The device toggle that allows media for a device.
pub(crate) const TOGGLE: &str = "media";
/// Offered by devices that show and control other devices' players.
const REMOTE: &str = "media.remote";
/// Offered by devices that share their own players.
const SHARES: &str = "media.control";

/// A command for a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaAction {
    Play,
    Pause,
    Next,
    Previous,
    /// To the position given with it.
    Seek,
}

impl MediaAction {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaAction::Play => media_actions::PLAY,
            MediaAction::Pause => media_actions::PAUSE,
            MediaAction::Next => media_actions::NEXT,
            MediaAction::Previous => media_actions::PREVIOUS,
            MediaAction::Seek => media_actions::SEEK,
        }
    }

    pub fn parse(action: &str) -> Option<MediaAction> {
        Some(match action {
            media_actions::PLAY => MediaAction::Play,
            media_actions::PAUSE => MediaAction::Pause,
            media_actions::NEXT => MediaAction::Next,
            media_actions::PREVIOUS => MediaAction::Previous,
            media_actions::SEEK => MediaAction::Seek,
            _ => return None,
        })
    }
}

/// Why a media command didn't run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaError {
    /// The player is gone.
    NotFound,
    /// The player (or this device) can't do that.
    Unsupported,
    /// Anything else; the string is for logs.
    Failed(String),
}

/// This device's players, as the app last reported them, with their
/// artwork kept apart for sending once per session.
#[derive(Default)]
pub(crate) struct Players {
    inner: Mutex<PlayersState>,
}

#[derive(Default)]
struct PlayersState {
    /// Set once the app reports players: only then does this device send.
    active: bool,
    players: Vec<MediaPlayer>,
    art: HashMap<String, Vec<u8>>,
}

impl Players {
    fn lock(&self) -> std::sync::MutexGuard<'_, PlayersState> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set(&self, players: Vec<MediaPlayer>) {
        let mut state = self.lock();
        state.active = true;
        state.players.clear();
        let mut art = HashMap::new();
        for mut player in players.into_iter().filter_map(MediaPlayer::sanitized).take(PLAYERS) {
            if let (Some(key), Some(bytes)) = (&player.art_key, player.art.take()) {
                art.insert(key.clone(), bytes);
            } else if let Some(key) = &player.art_key
                && let Some(bytes) = state.art.remove(key)
            {
                // Artwork is sent with every report; keep what's still in use.
                art.insert(key.clone(), bytes);
            }
            state.players.push(player);
        }
        state.art = art;
    }

    /// The players for one session, artwork attached where it hasn't had it.
    fn for_session(&self, session: &Session) -> MediaState {
        let state = self.lock();
        let mut sent = session.sent_art();
        let players = state
            .players
            .iter()
            .map(|player| {
                let mut player = player.clone();
                if let Some(key) = &player.art_key
                    && let Some(art) = state.art.get(key)
                    && sent.insert(key.clone())
                {
                    player.art = Some(art.clone());
                }
                player
            })
            .collect();
        MediaState { players }
    }

    /// Returns the title of the currently playing media item on this device, if any.
    pub(crate) fn active_title(&self) -> Option<String> {
        let state = self.lock();
        state
            .players
            .iter()
            .find(|p| p.playing && p.title.as_ref().is_some_and(|t| !t.trim().is_empty()))
            .or_else(|| state.players.iter().find(|p| p.title.as_ref().is_some_and(|t| !t.trim().is_empty())))
            .and_then(|p| p.title.clone())
    }
}

impl Shared {
    fn shows_media(&self, peer: &DeviceId) -> bool {
        self.store.get_peer(peer).ok().flatten().is_some_and(|p| p.caps.contains(REMOTE))
    }

    /// Sends this device's players to one device: all of them when the
    /// user allows it, none (which clears that device) when not.
    pub(crate) async fn send_media_state(&self, session: &Arc<Session>) {
        if !self.players.lock().active || !self.shows_media(&session.peer) {
            return;
        }
        let state = if self.toggle_on(&session.peer, TOGGLE) {
            self.players.for_session(session)
        } else {
            MediaState { players: Vec::new() }
        };
        if let Ok(env) = Envelope::new(types::MEDIA_STATE, &state) {
            let _ = session.send(env).await;
        }
    }

    pub(crate) async fn media_changed(&self, players: Vec<MediaPlayer>) {
        self.players.set(players);
        for session in self.live_sessions() {
            if self.toggle_on(&session.peer, TOGGLE) {
                self.send_media_state(&session).await;
            }
        }
    }

    /// Brings both sides in line after the user allowed or stopped media
    /// for a device.
    pub(crate) fn media_toggled(self: &Arc<Self>, peer: DeviceId, enabled: bool) {
        let Some(session) = self.session(&peer) else {
            if !enabled {
                self.emit(NodeEvent::MediaChanged { device: peer, players: Vec::new() });
            }
            return;
        };
        let shared = self.clone();
        self.runtime.spawn(async move {
            // What this device shares: everything, or nothing.
            shared.send_media_state(&session).await;
            if enabled {
                // What the other one shares: ask for it.
                let _ = session.send(Envelope::empty(types::MEDIA_SYNC)).await;
            } else {
                shared.emit(NodeEvent::MediaChanged { device: peer, players: Vec::new() });
            }
        });
    }
}

/// Handles a `media.*` message on a session. Returns false for other types.
pub(crate) async fn handle(shared: &Arc<Shared>, session: &Arc<Session>, env: &Envelope) -> Result<bool> {
    let peer = session.peer;
    match env.t.as_str() {
        types::MEDIA_STATE => {
            if shared.toggle_on(&peer, TOGGLE) {
                let state: MediaState = env.body()?;
                let players =
                    state.players.into_iter().filter_map(MediaPlayer::sanitized).take(PLAYERS).collect();
                shared.emit(NodeEvent::MediaChanged { device: peer, players });
            }
        }
        types::MEDIA_SYNC => shared.send_media_state(session).await,
        types::MEDIA_WATCH => {
            let reply = watch_offered(shared, &peer, env.body()?).await;
            session.send(reply.reply_to(env.id)).await?;
        }
        types::MEDIA_COMMAND => {
            let command: MediaCommand = env.body()?;
            let reply = run_command(shared, &peer, command).await;
            session.send(reply.reply_to(env.id)).await?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// Play on phone, on the PC: offers `peer` what `player` plays. The offer
/// is what lets that phone start mirroring with `watch` (see
/// [`watch_allowed`]).
pub(crate) async fn offer_watch(
    shared: &Arc<Shared>,
    peer: DeviceId,
    player: String,
    title: Option<String>,
) -> Result<()> {
    let watch = MediaWatch { player, title };
    if !watch.is_valid() {
        return Err(Error::Internal("not a player to offer".into()));
    }
    if !shared.local_capabilities().iter().any(|c| c == media_watch::STREAM) {
        return Err(Error::Unsupported);
    }
    if !shared.store.get_peer(&peer)?.is_some_and(|p| p.caps.contains(media_watch::WATCH)) {
        return Err(Error::Unsupported);
    }
    if !shared.toggle_on(&peer, TOGGLE) {
        return Err(Error::Denied);
    }
    let session = shared.session(&peer).ok_or(Error::Offline)?;
    let player = watch.player.clone();
    shared
        .watch_offers
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(peer, (player, std::time::Instant::now()));
    let env = Envelope::new(types::MEDIA_WATCH, &watch)?;
    let answer =
        session.request(env, crate::session::REQUEST_TIMEOUT).await.and_then(|r| Ok(r.expect(types::OK)?));
    if answer.is_err() {
        shared.watch_offers.lock().unwrap_or_else(|e| e.into_inner()).remove(&peer);
    }
    answer.map(|_| ())
}

/// Whether `peer` may watch `player`: this PC offered it, and the phone
/// takes it (or comes back to it) in time. Taking it renews the offer.
pub(crate) fn watch_allowed(shared: &Shared, peer: &DeviceId, player: &str) -> bool {
    let mut offers = shared.watch_offers.lock().unwrap_or_else(|e| e.into_inner());
    match offers.get_mut(peer) {
        Some((offered, at)) if offered == player && at.elapsed() < WATCH_OFFER => {
            *at = std::time::Instant::now();
            true
        }
        _ => false,
    }
}

/// Whether `peer` has a Play on phone offer from this PC open (so this PC
/// may open the stream to it).
pub(crate) fn watch_open(shared: &Shared, peer: &DeviceId) -> bool {
    shared
        .watch_offers
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(peer)
        .is_some_and(|(_, at)| at.elapsed() < WATCH_OFFER)
}

/// Watching stopped: the offer stays open for a while from now.
pub(crate) fn renew_watch(shared: &Shared, peer: &DeviceId) {
    if let Some((_, at)) = shared.watch_offers.lock().unwrap_or_else(|e| e.into_inner()).get_mut(peer) {
        *at = std::time::Instant::now();
    }
}

/// The phone's side of `media.watch`: show the offer.
async fn watch_offered(shared: &Arc<Shared>, peer: &DeviceId, watch: MediaWatch) -> Envelope {
    if !shared.local_capabilities().iter().any(|c| c == media_watch::WATCH) {
        return Envelope::error(ErrorCode::Unsupported, "this device doesn't show a PC's playback");
    }
    if !shared.toggle_on(peer, TOGGLE) {
        return Envelope::error(ErrorCode::Denied, "media is off for this device");
    }
    if !watch.is_valid() {
        return Envelope::error(ErrorCode::BadMessage, "invalid watch offer");
    }
    let platform = shared.platform.clone();
    let peer = *peer;
    let MediaWatch { player, title } = watch;
    match tokio::task::spawn_blocking(move || platform.watch_offered(&peer, &player, title.as_deref()))
        .await
        .unwrap_or_else(|e| Err(e.to_string()))
    {
        Ok(()) => Envelope::empty(types::OK),
        Err(reason) => {
            tracing::warn!(reason, "can't show a watch offer");
            Envelope::error(ErrorCode::Internal, "can't show it")
        }
    }
}

async fn run_command(shared: &Arc<Shared>, peer: &DeviceId, command: MediaCommand) -> Envelope {
    if !shared.local_capabilities().iter().any(|c| c == SHARES) {
        return Envelope::error(ErrorCode::Unsupported, "media control is not available");
    }
    if !shared.toggle_on(peer, TOGGLE) {
        return Envelope::error(ErrorCode::Denied, "media is off for this device");
    }
    let Some(action) = MediaAction::parse(&command.action) else {
        return Envelope::error(ErrorCode::Unsupported, "unknown media action");
    };
    if action == MediaAction::Seek && command.position.is_none() {
        return Envelope::error(ErrorCode::BadMessage, "seek needs a position");
    }
    let platform = shared.platform.clone();
    let MediaCommand { player, position, .. } = command;
    let ran = tokio::task::spawn_blocking(move || platform.media_command(&player, action, position))
        .await
        .unwrap_or_else(|e| Err(MediaError::Failed(e.to_string())));
    match ran {
        Ok(()) => Envelope::empty(types::OK),
        Err(MediaError::NotFound) => Envelope::error(ErrorCode::NotFound, "no such player"),
        Err(MediaError::Unsupported) => Envelope::error(ErrorCode::Unsupported, "the player can't do that"),
        Err(MediaError::Failed(reason)) => {
            tracing::warn!(reason, "media command failed");
            Envelope::error(ErrorCode::Internal, "failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_round_trip() {
        for action in [
            MediaAction::Play,
            MediaAction::Pause,
            MediaAction::Next,
            MediaAction::Previous,
            MediaAction::Seek,
        ] {
            assert_eq!(MediaAction::parse(action.as_str()), Some(action));
        }
        assert_eq!(MediaAction::parse("rewind"), None);
    }
}
