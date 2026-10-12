//! Bounded PCM delivery, overflow counters and lifecycle flush markers.

use super::transport_error;
use crate::{AttemptId, SessionId};
use diagnostics::EmuError;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

const DEFAULT_AUDIO_FRAMES: usize = 22_050 * 2;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AudioStats {
    pub queued_frames: usize,
    pub dropped_frames: u64,
    pub underrun_frames: u64,
}

/// One consistent platform-sink view of the current audio generation. The
/// flush revision also changes for pauses that keep the same VM attempt alive.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AudioStatus {
    pub active_generation: Option<(SessionId, AttemptId)>,
    pub queued_frames: usize,
    pub flush_revision: u64,
}

#[derive(Default)]
struct AudioState {
    active: Option<(SessionId, AttemptId)>,
    suspended: bool,
    samples: VecDeque<i16>,
    dropped: u64,
    underrun: u64,
    flush_revision: u64,
}

/// Bounded mono 22.05 kHz PCM ring. Overflow drops the oldest samples so live
/// audio catches up; underrun is explicitly filled with silence.
#[derive(Clone)]
pub struct AudioMailbox {
    inner: Arc<Mutex<AudioState>>,
    capacity: usize,
}

impl Default for AudioMailbox {
    fn default() -> Self {
        Self::new(DEFAULT_AUDIO_FRAMES)
    }
}

impl AudioMailbox {
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        let _ = std::panic::catch_unwind(|| {
            let _guard = self.inner.lock().unwrap();
            panic!("audio mailbox failure fixture");
        });
    }

    #[must_use]
    pub fn new(capacity_frames: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AudioState::default())),
            capacity: capacity_frames.max(1),
        }
    }

    pub(crate) fn activate(
        &self,
        session_id: SessionId,
        attempt_id: AttemptId,
    ) -> Result<(), EmuError> {
        let mut state = self.lock()?;
        let flush_revision = state.flush_revision.saturating_add(1);
        *state = AudioState {
            active: Some((session_id, attempt_id)),
            flush_revision,
            ..AudioState::default()
        };
        Ok(())
    }

    pub(crate) fn publish(
        &self,
        session_id: SessionId,
        attempt_id: AttemptId,
        samples: &[i16],
    ) -> Result<bool, EmuError> {
        let mut state = self.lock()?;
        if state.active != Some((session_id, attempt_id)) {
            return Ok(false);
        }
        if state.suspended {
            return Ok(true);
        }
        let incoming = samples.len().min(self.capacity);
        let retained_start = samples.len().saturating_sub(incoming);
        let overflow = state
            .samples
            .len()
            .saturating_add(incoming)
            .saturating_sub(self.capacity);
        state.samples.drain(..overflow);
        state.dropped = state
            .dropped
            .saturating_add(u64::try_from(overflow + retained_start).unwrap_or(u64::MAX));
        state.samples.extend(&samples[retained_start..]);
        Ok(true)
    }

    /// Drains available mono frames and fills the remainder with silence.
    /// Returns the number of real frames copied before the underrun fill.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn read_or_silence(&self, output: &mut [i16]) -> Result<usize, EmuError> {
        let mut state = self.lock()?;
        let available = copy_queued_audio(&mut state, output);
        output[available..].fill(0);
        state.underrun = state.underrun.saturating_add(
            u64::try_from(output.len().saturating_sub(available)).unwrap_or(u64::MAX),
        );
        Ok(available)
    }

    /// Drains only available frames and returns their generation/flush marker
    /// from the same lock. The rest of `output` is unchanged; empty polls do not
    /// count as an audio underrun. Platform sinks flush before sending samples
    /// when this marker changes.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn read_available(&self, output: &mut [i16]) -> Result<(AudioStatus, usize), EmuError> {
        let mut state = self.lock()?;
        let status = audio_status(&state);
        let copied = copy_queued_audio(&mut state, output);
        Ok((status, copied))
    }

    /// Returns current bounded-ring counters.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn stats(&self) -> Result<AudioStats, EmuError> {
        let state = self.lock()?;
        Ok(AudioStats {
            queued_frames: state.samples.len(),
            dropped_frames: state.dropped,
            underrun_frames: state.underrun,
        })
    }

    /// Returns the generation currently allowed to publish audio. Platform
    /// sinks use changes in this value to flush buffered samples atomically.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn active_generation(&self) -> Result<Option<(SessionId, AttemptId)>, EmuError> {
        let state = self.lock()?;
        Ok(if state.suspended { None } else { state.active })
    }

    /// Returns the active generation, queued samples, and platform flush
    /// revision under one lock.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn status(&self) -> Result<AudioStatus, EmuError> {
        let state = self.lock()?;
        Ok(audio_status(&state))
    }

    pub(crate) fn set_suspended(
        &self,
        session_id: SessionId,
        attempt_id: AttemptId,
        suspended: bool,
    ) -> Result<(), EmuError> {
        let mut state = self.lock()?;
        if state.active == Some((session_id, attempt_id)) && state.suspended != suspended {
            state.suspended = suspended;
            state.samples.clear();
            state.flush_revision = state.flush_revision.saturating_add(1);
        }
        Ok(())
    }

    pub(crate) fn clear(&self) -> Result<(), EmuError> {
        let mut state = self.lock()?;
        let flush_revision = state.flush_revision.saturating_add(1);
        *state = AudioState {
            flush_revision,
            ..AudioState::default()
        };
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, AudioState>, EmuError> {
        self.inner
            .lock()
            .map_err(|_| transport_error("audio-lock", "audio mailbox lock is poisoned"))
    }
}

fn audio_status(state: &AudioState) -> AudioStatus {
    AudioStatus {
        active_generation: if state.suspended { None } else { state.active },
        queued_frames: state.samples.len(),
        flush_revision: state.flush_revision,
    }
}

fn copy_queued_audio(state: &mut AudioState, output: &mut [i16]) -> usize {
    let available = output.len().min(state.samples.len());
    let (first, second) = state.samples.as_slices();
    let first_count = available.min(first.len());
    output[..first_count].copy_from_slice(&first[..first_count]);
    output[first_count..available].copy_from_slice(&second[..available - first_count]);
    state.samples.drain(..available);
    available
}
