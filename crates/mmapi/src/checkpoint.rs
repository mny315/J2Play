//! Mixer checkpoints retain decoded audio and playback cursors, never a device sink.

use super::{
    AudioSink, BTreeMap, EmuError, OneShot, Player, PlayerState, Runtime, encoded_media_bytes,
    media_error,
};
use crate::{Clip, EncodedMedia, PlayerEvent};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Serialize)]
struct CheckpointRef<'a> {
    players: &'a BTreeMap<u64, Player>,
    next_handle: u64,
    exclusive_player: Option<u64>,
    last_tick_micros: i64,
    render_frame_remainder: u64,
    one_shot: &'a Option<OneShot>,
}

#[derive(Deserialize)]
struct Checkpoint {
    players: BTreeMap<u64, Player>,
    next_handle: u64,
    exclusive_player: Option<u64>,
    last_tick_micros: i64,
    render_frame_remainder: u64,
    one_shot: Option<OneShot>,
}

impl<S: AudioSink> Runtime<S> {
    /// Captures players, pending media events and mixer position at a VM boundary.
    ///
    /// # Errors
    /// Returns an error if the encoded component exceeds its byte budget.
    pub fn encode_checkpoint(&self) -> Result<Vec<u8>, EmuError> {
        self.encode_checkpoint_cancellable(&|| false)
    }

    /// Captures media state with a host deadline or cancellation signal.
    ///
    /// # Errors
    /// Returns an encoding, size or cancellation error.
    pub fn encode_checkpoint_cancellable(
        &self,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EmuError> {
        save_state::encode_cancellable(
            &CheckpointRef {
                players: &self.players,
                next_handle: self.next_handle,
                exclusive_player: self.exclusive_player,
                last_tick_micros: self.last_tick_micros,
                render_frame_remainder: self.render_frame_remainder,
                one_shot: &self.one_shot,
            },
            save_state::MAX_COMPONENT_BYTES,
            cancelled,
        )
    }

    /// Restores players into the current sink after validating its resource limits.
    ///
    /// # Errors
    /// Returns an error for corrupt state, handles or media exceeding runtime limits.
    pub fn restore_checkpoint(&mut self, bytes: &[u8]) -> Result<(), EmuError> {
        let saved: Checkpoint = save_state::decode(bytes)?;
        if saved.players.len() > self.limits.max_handle_history
            || saved.next_handle == 0
            || saved.next_handle > i64::MAX as u64 + 1
            || saved.render_frame_remainder >= 1_000_000
            || saved
                .exclusive_player
                .is_some_and(|handle| !saved.players.contains_key(&handle))
            || saved.one_shot.as_ref().is_some_and(|tone| {
                tone.samples.len() > self.limits.max_pcm_frames
                    || tone.rendered_frames > tone.samples.len()
            })
        {
            return Err(invalid());
        }
        let mut encoded = 0_usize;
        let mut pcm = saved.one_shot.as_ref().map_or(0, |tone| tone.samples.len());
        let mut live = 0;
        let mut started = 0;
        let mut started_midi = 0;
        for (&handle, player) in &saved.players {
            let input_bytes = encoded_media_bytes(&player.encoded);
            let frames = player.clip.as_ref().map_or(0, |clip| clip.samples.len());
            if handle == 0
                || handle >= saved.next_handle
                || input_bytes > self.limits.max_input_bytes
                || frames > self.limits.max_pcm_frames
                || player
                    .clip
                    .as_ref()
                    .is_some_and(|clip| clip.duration_micros < 0)
                || player.events.len() > self.limits.max_pending_events
                || !(0..=100).contains(&player.volume)
                || player.position_micros < 0
                || player.loop_count == 0
                || player.loop_count < -1
                || !valid_player_storage(player)
            {
                return Err(invalid());
            }
            encoded = encoded.checked_add(input_bytes).ok_or_else(invalid)?;
            pcm = pcm.checked_add(frames).ok_or_else(invalid)?;
            live += usize::from(player.state != PlayerState::Closed);
            if player.state == PlayerState::Started {
                started += 1;
                started_midi += usize::from(player.content_type == "audio/midi");
            }
        }
        if encoded > self.limits.max_total_input_bytes
            || pcm > self.limits.max_total_pcm_frames
            || live > self.limits.max_players
            || started > self.limits.max_started_players
            || started_midi > self.limits.max_started_midi_players
        {
            return Err(invalid());
        }
        self.players = saved.players;
        self.next_handle = saved.next_handle;
        self.exclusive_player = saved.exclusive_player;
        self.last_tick_micros = saved.last_tick_micros;
        self.render_frame_remainder = saved.render_frame_remainder;
        self.one_shot = saved.one_shot;
        self.total_encoded_bytes = encoded;
        self.total_pcm_frames = pcm;
        self.mix_scratch.clear();
        self.output_scratch.clear();
        Ok(())
    }
}

fn valid_player_storage(player: &Player) -> bool {
    let realized = matches!(
        player.state,
        PlayerState::Realized | PlayerState::Prefetched | PlayerState::Started
    );
    player.clip.is_some() == realized
        && match player.encoded {
            EncodedMedia::Bytes(_) => player.state == PlayerState::Unrealized,
            EncodedMedia::ToneDevice(_) => {
                player.state != PlayerState::Closed && player.content_type == "audio/x-tone-seq"
            }
            EncodedMedia::MidiDevice => {
                player.state != PlayerState::Closed && player.content_type == "audio/midi"
            }
            EncodedMedia::Decoded => realized,
            EncodedMedia::Closed => player.state == PlayerState::Closed,
        }
}

#[derive(Deserialize)]
pub(super) struct SavedPlayer {
    content_type: String,
    encoded: EncodedMedia,
    clip: Option<Clip>,
    state: PlayerState,
    volume: i32,
    muted: bool,
    loop_count: i32,
    completed_loops: u64,
    position_micros: i64,
    started_at_micros: i64,
    start_position_micros: i64,
    rendered_frames: usize,
    events: VecDeque<PlayerEvent>,
    last_event_data: i64,
}

impl TryFrom<SavedPlayer> for Player {
    type Error = EmuError;

    fn try_from(saved: SavedPlayer) -> Result<Self, Self::Error> {
        Ok(Self {
            content_type: crate::media_decode::canonical_content_type(&saved.content_type)?,
            encoded: saved.encoded,
            clip: saved.clip,
            state: saved.state,
            volume: saved.volume,
            muted: saved.muted,
            loop_count: saved.loop_count,
            completed_loops: saved.completed_loops,
            position_micros: saved.position_micros,
            started_at_micros: saved.started_at_micros,
            start_position_micros: saved.start_position_micros,
            rendered_frames: saved.rendered_frames,
            events: saved.events,
            last_event_data: saved.last_event_data,
        })
    }
}

impl<'de> Deserialize<'de> for Player {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        SavedPlayer::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/mmapi/checkpoint.rs"]
mod tests;

fn invalid() -> EmuError {
    media_error(
        "checkpoint-media",
        "The checkpoint contains invalid media state",
    )
}
