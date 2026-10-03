//! Bounded Mobile Media API audio core.

mod checkpoint;
mod media_decode;
mod midi;
mod mixing;
mod native_api;
mod playback;
mod preparation;
mod smaf;
mod synthesis;
mod tone;
mod wav;

use media_decode::{
    canonical_content_type, decode_player, micros_to_frames, require_open, require_realized,
    resample_mono,
};
use midi::{MidiEvent, MidiKind, decode_midi, render_midi_events};
use mixing::{mix_one_shot, mix_player};
use synthesis::{
    append_synth_note, drum_duration_frames, mix_drum, mix_midi_note, note_phase_step,
    soft_limit_i16,
};
use tone::{decode_tone_sequence, prepare_tone_sequence};
use wav::decode_wav;

pub use native_api::register_natives;

#[cfg(test)]
use native_api::{content_type_from_path, java_handle};

use diagnostics::{Category, EmuError};
use natives::{NativeRegistry, NativeSignature, NativeValue};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Fixed deterministic mixer rate used by the reference/null backend.
pub const OUTPUT_SAMPLE_RATE: u32 = 22_050;

fn check_decode_cancellation(cancelled: &dyn Fn() -> bool) -> Result<(), EmuError> {
    if cancelled() {
        Err(media_error(
            "execution-cancelled",
            "audio preparation was cancelled by the host",
        ))
    } else {
        Ok(())
    }
}

const MAX_MIDI_TRACKS: u16 = 64;
const MAX_MIDI_TRUNCATED_TAIL_BYTES: usize = 4 * 1024;
const MAX_MIDI_ZERO_PADDING_BYTES: usize = 4 * 1024;
/// MMAPI's unknown-time sentinel, in microseconds.
pub const TIME_UNKNOWN: i64 = -1;

/// Resource limits applied before decoding or rendering untrusted media.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_players: usize,
    pub max_started_players: usize,
    pub max_started_midi_players: usize,
    pub max_handle_history: usize,
    pub max_input_bytes: usize,
    pub max_total_input_bytes: usize,
    pub max_pcm_frames: usize,
    pub max_total_pcm_frames: usize,
    pub max_midi_events: usize,
    pub max_midi_voice_frames: usize,
    pub max_tone_events: usize,
    pub max_pending_events: usize,
    pub max_tick_frames: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            // Preloaded sound banks need more slots than simultaneous playback.
            max_players: 32,
            max_started_players: 4,
            max_started_midi_players: 1,
            max_handle_history: 4_096,
            max_input_bytes: 8 * 1024 * 1024,
            max_total_input_bytes: 32 * 1024 * 1024,
            max_pcm_frames: OUTPUT_SAMPLE_RATE as usize * 10 * 60,
            max_total_pcm_frames: OUTPUT_SAMPLE_RATE as usize * 10 * 60,
            max_midi_events: 100_000,
            // Allow 32 voices across the same ten-minute duration as PCM.
            max_midi_voice_frames: OUTPUT_SAMPLE_RATE as usize * 10 * 60 * 32,
            max_tone_events: 10_000,
            max_pending_events: 1_024,
            max_tick_frames: OUTPUT_SAMPLE_RATE as usize * 60,
        }
    }
}

/// Observable MMAPI `Player` lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum PlayerState {
    Closed = 0,
    Unrealized = 100,
    Realized = 200,
    Prefetched = 300,
    Started = 400,
}

/// Events queued by the host core for Java `PlayerListener` delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PlayerEventKind {
    Started,
    Stopped,
    EndOfMedia,
    Closed,
}

impl PlayerEventKind {
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Started => 1,
            Self::Stopped => 2,
            Self::EndOfMedia => 3,
            Self::Closed => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlayerEvent {
    pub kind: PlayerEventKind,
    pub media_time_micros: i64,
}

/// A sink consumes signed mono PCM without imposing an audio-device dependency.
pub trait AudioSink {
    /// Accepts one contiguous block at [`OUTPUT_SAMPLE_RATE`].
    ///
    /// # Errors
    /// Returns a platform diagnostic if the backend cannot accept the block.
    fn write(&mut self, samples: &[i16]) -> Result<(), EmuError>;
}

/// Headless backend that discards samples while retaining bounded statistics.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NullAudioSink {
    frames_written: u64,
    peak: i16,
}

impl NullAudioSink {
    #[must_use]
    pub const fn frames_written(&self) -> u64 {
        self.frames_written
    }

    #[must_use]
    pub const fn peak(&self) -> i16 {
        self.peak
    }
}

impl AudioSink for NullAudioSink {
    fn write(&mut self, samples: &[i16]) -> Result<(), EmuError> {
        self.frames_written = self
            .frames_written
            .saturating_add(u64::try_from(samples.len()).unwrap_or(u64::MAX));
        for &sample in samples {
            self.peak = self.peak.max(sample.saturating_abs());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
enum EncodedMedia {
    Bytes(
        #[serde(
            serialize_with = "save_state::serialize_bytes",
            deserialize_with = "save_state::deserialize_bytes"
        )]
        Arc<[u8]>,
    ),
    ToneDevice(
        #[serde(
            serialize_with = "save_state::serialize_optional_bytes",
            deserialize_with = "save_state::deserialize_optional_bytes"
        )]
        Option<Vec<u8>>,
    ),
    MidiDevice,
    Decoded,
    Closed,
}

fn encoded_media_bytes(media: &EncodedMedia) -> usize {
    match media {
        EncodedMedia::Bytes(bytes) => bytes.len(),
        EncodedMedia::ToneDevice(Some(sequence)) => sequence.len(),
        EncodedMedia::ToneDevice(None)
        | EncodedMedia::MidiDevice
        | EncodedMedia::Decoded
        | EncodedMedia::Closed => 0,
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Clip {
    // Finished PCM retains exactly the frames charged to the media budget,
    // including after in-place downsampling or decoder buffer growth.
    samples: Box<[i16]>,
    duration_micros: i64,
}

impl Clip {
    fn duration_micros(&self) -> i64 {
        self.duration_micros
    }
}

#[derive(Clone, Debug, serde::Serialize)]
struct Player {
    content_type: &'static str,
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

impl Player {
    fn new(content_type: &'static str, encoded: EncodedMedia) -> Self {
        Self {
            content_type,
            encoded,
            clip: None,
            state: PlayerState::Unrealized,
            volume: 100,
            muted: false,
            loop_count: 1,
            completed_loops: 0,
            position_micros: 0,
            started_at_micros: 0,
            start_position_micros: 0,
            rendered_frames: 0,
            events: VecDeque::new(),
            last_event_data: 0,
        }
    }

    fn duration_micros(&self) -> i64 {
        self.clip
            .as_ref()
            .map_or(TIME_UNKNOWN, Clip::duration_micros)
    }

    fn queue(
        &mut self,
        limits: Limits,
        kind: PlayerEventKind,
        media_time_micros: i64,
    ) -> Result<(), EmuError> {
        if self.events.len() >= limits.max_pending_events {
            return Err(media_error(
                "event-queue-limit",
                "MMAPI listener event queue limit reached",
            ));
        }
        self.events.push_back(PlayerEvent {
            kind,
            media_time_micros,
        });
        Ok(())
    }

    fn queue_bounded(&mut self, limits: Limits, kind: PlayerEventKind, media_time_micros: i64) {
        if limits.max_pending_events == 0 {
            return;
        }
        if self.events.len() >= limits.max_pending_events {
            self.events.pop_front();
        }
        self.events.push_back(PlayerEvent {
            kind,
            media_time_micros,
        });
    }
}

/// Per-suite audio runtime. Time advances only when the host polls it, so no VM
/// thread is blocked waiting for media or an audio device.
pub struct Runtime<S: AudioSink> {
    sink: S,
    limits: Limits,
    decode_cancelled: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    players: BTreeMap<u64, Player>,
    next_handle: u64,
    exclusive_player: Option<u64>,
    last_tick_micros: i64,
    render_frame_remainder: u64,
    one_shot: Option<OneShot>,
    mix_scratch: Vec<i32>,
    output_scratch: Vec<i16>,
    total_encoded_bytes: usize,
    total_pcm_frames: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct OneShot {
    samples: Box<[i16]>,
    rendered_frames: usize,
}

#[allow(clippy::missing_errors_doc)]
impl<S: AudioSink> Runtime<S> {
    #[must_use]
    pub fn new(sink: S, limits: Limits) -> Self {
        Self {
            sink,
            limits,
            decode_cancelled: None,
            players: BTreeMap::new(),
            next_handle: 1,
            exclusive_player: None,
            last_tick_micros: 0,
            render_frame_remainder: 0,
            one_shot: None,
            mix_scratch: Vec::new(),
            output_scratch: Vec::new(),
            total_encoded_bytes: 0,
            total_pcm_frames: 0,
        }
    }

    #[must_use]
    /// Installs the host's inexpensive cancellation check for lengthy synthesis.
    /// Cancelled realization leaves the encoded player available for a retry.
    pub fn with_decode_cancellation(
        mut self,
        cancelled: impl Fn() -> bool + Send + Sync + 'static,
    ) -> Self {
        self.decode_cancelled = Some(Box::new(cancelled));
        self
    }

    #[must_use]
    pub const fn sink(&self) -> &S {
        &self.sink
    }

    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// Drops players whose Java wrappers were found unreachable by the VM.
    ///
    /// Explicitly closed players remain queryable while their wrappers are
    /// live. Once a wrapper is unreachable, removing the complete entry also
    /// releases encoded/decoded budgets and bounded handle-history capacity.
    pub fn retain_handles(&mut self, live_handles: &[u64]) -> usize {
        let live_handles = live_handles.iter().copied().collect::<HashSet<_>>();
        if self
            .exclusive_player
            .is_some_and(|handle| !live_handles.contains(&handle))
        {
            self.exclusive_player = None;
        }
        let mut released_encoded_bytes = 0_usize;
        let mut released_pcm_frames = 0_usize;
        let mut released_players = 0_usize;
        self.players.retain(|handle, player| {
            if live_handles.contains(handle) {
                return true;
            }
            released_encoded_bytes =
                released_encoded_bytes.saturating_add(encoded_media_bytes(&player.encoded));
            released_pcm_frames = released_pcm_frames
                .saturating_add(player.clip.as_ref().map_or(0, |clip| clip.samples.len()));
            released_players = released_players.saturating_add(1);
            false
        });
        self.total_encoded_bytes = self
            .total_encoded_bytes
            .saturating_sub(released_encoded_bytes);
        self.total_pcm_frames = self.total_pcm_frames.saturating_sub(released_pcm_frames);
        released_players
    }

    /// Creates an unrealized player from bounded encoded bytes.
    ///
    /// # Errors
    /// Rejects unsupported content types, oversized input, or player exhaustion.
    pub fn create_from_bytes(&mut self, content_type: &str, data: &[u8]) -> Result<u64, EmuError> {
        let content_type = canonical_content_type(content_type)?;
        if data.len() > self.limits.max_input_bytes {
            return Err(media_error("media-limit", "media input exceeds byte limit"));
        }
        let total_encoded_bytes = self
            .total_encoded_bytes
            .checked_add(data.len())
            .filter(|total| *total <= self.limits.max_total_input_bytes)
            .ok_or_else(|| {
                media_error(
                    "media-limit",
                    "suite media input exceeds aggregate byte limit",
                )
            })?;
        let handle =
            self.insert_player(|| Player::new(content_type, EncodedMedia::Bytes(Arc::from(data))))?;
        self.total_encoded_bytes = total_encoded_bytes;
        Ok(handle)
    }

    /// Creates an unrealized tone-sequence device player.
    ///
    /// # Errors
    /// Returns `player-limit` when the suite has too many live players.
    pub fn create_tone_player(&mut self) -> Result<u64, EmuError> {
        self.insert_player(|| Player::new("audio/x-tone-seq", EncodedMedia::ToneDevice(None)))
    }

    /// Creates an empty MIDI device player.
    ///
    /// # Errors
    /// Returns `player-limit` when the suite has too many live players.
    pub fn create_midi_player(&mut self) -> Result<u64, EmuError> {
        self.insert_player(|| Player::new("audio/midi", EncodedMedia::MidiDevice))
    }

    fn insert_player(&mut self, create: impl FnOnce() -> Player) -> Result<u64, EmuError> {
        let live_players = self
            .players
            .values()
            .filter(|player| player.state != PlayerState::Closed)
            .count();
        if live_players >= self.limits.max_players
            || self.players.len() >= self.limits.max_handle_history
        {
            return Err(media_error("player-limit", "MMAPI player limit reached"));
        }
        let handle = self.next_handle;
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .filter(|_| i64::try_from(handle).is_ok())
            .ok_or_else(|| media_error("player-limit", "MMAPI handle space exhausted"))?;
        self.players.insert(handle, create());
        Ok(handle)
    }

    pub fn content_type(&self, handle: u64) -> Result<&str, EmuError> {
        let player = self.player(handle)?;
        require_realized(player)?;
        Ok(player.content_type)
    }

    pub fn duration(&self, handle: u64) -> Result<i64, EmuError> {
        let player = self.player(handle)?;
        require_open(player)?;
        Ok(player.duration_micros())
    }

    pub fn volume(&self, handle: u64) -> Result<i32, EmuError> {
        let player = self.player(handle)?;
        require_open(player)?;
        Ok(player.volume)
    }

    pub fn set_volume(&mut self, handle: u64, volume: i32) -> Result<i32, EmuError> {
        let player = self.player_mut(handle)?;
        require_open(player)?;
        player.volume = volume.clamp(0, 100);
        Ok(player.volume)
    }

    pub fn muted(&self, handle: u64) -> Result<bool, EmuError> {
        let player = self.player(handle)?;
        require_open(player)?;
        Ok(player.muted)
    }

    pub fn set_muted(&mut self, handle: u64, muted: bool) -> Result<(), EmuError> {
        let player = self.player_mut(handle)?;
        require_open(player)?;
        player.muted = muted;
        Ok(())
    }

    fn player(&self, handle: u64) -> Result<&Player, EmuError> {
        self.players
            .get(&handle)
            .ok_or_else(|| media_error("player-closed", "unknown MMAPI player handle"))
    }

    fn player_mut(&mut self, handle: u64) -> Result<&mut Player, EmuError> {
        self.players
            .get_mut(&handle)
            .ok_or_else(|| media_error("player-closed", "unknown MMAPI player handle"))
    }
}

fn media_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Api, code, message)
}

fn clamp_i16(sample: i32) -> i16 {
    i16::try_from(sample.clamp(i32::from(i16::MIN), i32::from(i16::MAX)))
        .expect("sample is clamped to i16")
}

#[cfg(test)]
#[path = "../../../tests/unit/mmapi/mod.rs"]
mod tests;
