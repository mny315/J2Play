//! MMAPI preparation operations on the suite-owned runtime.

use super::{
    AudioSink, EmuError, EncodedMedia, OUTPUT_SAMPLE_RATE, OneShot, PlayerState, Runtime,
    append_synth_note, check_decode_cancellation, decode_player, encoded_media_bytes, media_error,
    prepare_tone_sequence, require_open,
};

#[allow(clippy::missing_errors_doc)]
impl<S: AudioSink> Runtime<S> {
    /// Realizes media metadata and decoded PCM without taking a scarce device.
    pub fn realize(&mut self, handle: u64) -> Result<(), EmuError> {
        let mut limits = self.limits;
        limits.max_pcm_frames = limits.max_pcm_frames.min(
            limits
                .max_total_pcm_frames
                .saturating_sub(self.total_pcm_frames),
        );
        let (clip, released_encoded_bytes) = {
            let player = self.player(handle)?;
            require_open(player)?;
            if player.state != PlayerState::Unrealized {
                return Ok(());
            }
            let released = match &player.encoded {
                EncodedMedia::Bytes(bytes) => bytes.len(),
                _ => 0,
            };
            let cancelled = || self.decode_cancelled.as_ref().is_some_and(|check| check());
            check_decode_cancellation(&cancelled)?;
            (decode_player(player, limits, &cancelled)?, released)
        };
        let total_pcm_frames = self
            .total_pcm_frames
            .checked_add(clip.samples.len())
            .filter(|total| *total <= limits.max_total_pcm_frames)
            .ok_or_else(|| {
                media_error(
                    "media-limit",
                    "suite decoded audio exceeds aggregate PCM limit",
                )
            })?;
        let player = self.player_mut(handle)?;
        player.clip = Some(clip);
        if matches!(player.encoded, EncodedMedia::Bytes(_)) {
            player.encoded = EncodedMedia::Decoded;
        }
        player.state = PlayerState::Realized;
        self.total_pcm_frames = total_pcm_frames;
        self.total_encoded_bytes = self
            .total_encoded_bytes
            .saturating_sub(released_encoded_bytes);
        Ok(())
    }

    /// Acquires the logical audio resource. The null backend never fails here.
    pub fn prefetch(&mut self, handle: u64) -> Result<(), EmuError> {
        self.realize(handle)?;
        let player = self.player_mut(handle)?;
        if player.state == PlayerState::Realized {
            player.state = PlayerState::Prefetched;
        }
        Ok(())
    }

    pub fn set_tone_sequence(&mut self, handle: u64, sequence: &[u8]) -> Result<(), EmuError> {
        if sequence.len() > self.limits.max_input_bytes {
            return Err(media_error(
                "media-limit",
                "tone sequence exceeds byte limit",
            ));
        }
        let (state, old_encoded_bytes, old_pcm_frames) = {
            let player = self.player(handle)?;
            require_open(player)?;
            if matches!(player.state, PlayerState::Prefetched | PlayerState::Started) {
                return Err(media_error(
                    "illegal-state",
                    "tone sequence cannot change after prefetch",
                ));
            }
            if !matches!(player.encoded, EncodedMedia::ToneDevice(_)) {
                return Err(media_error(
                    "illegal-state",
                    "ToneControl is not available for this player",
                ));
            }
            (
                player.state,
                encoded_media_bytes(&player.encoded),
                player.clip.as_ref().map_or(0, |clip| clip.samples.len()),
            )
        };
        // Validate before changing the observable player.
        let cancelled = || self.decode_cancelled.as_ref().is_some_and(|check| check());
        let prepared =
            prepare_tone_sequence(sequence, self.limits, &cancelled).map_err(|error| {
                if error.code() == "execution-cancelled" {
                    return error;
                }
                media_error(
                    "illegal-argument",
                    format!("invalid ToneControl sequence: {error}"),
                )
            })?;
        let total_encoded_bytes = self
            .total_encoded_bytes
            .saturating_sub(old_encoded_bytes)
            .checked_add(sequence.len())
            .filter(|total| *total <= self.limits.max_total_input_bytes)
            .ok_or_else(|| {
                media_error(
                    "media-limit",
                    "suite media input exceeds aggregate byte limit",
                )
            })?;
        let total_pcm_frames = if state == PlayerState::Realized {
            self.total_pcm_frames
                .saturating_sub(old_pcm_frames)
                .checked_add(prepared.frames)
                .filter(|total| *total <= self.limits.max_total_pcm_frames)
                .ok_or_else(|| {
                    media_error(
                        "media-limit",
                        "suite decoded audio exceeds aggregate PCM limit",
                    )
                })?
        } else {
            self.total_pcm_frames
        };
        let clip = if state == PlayerState::Realized {
            Some(prepared.render(self.limits, &cancelled)?)
        } else {
            None
        };
        let player = self.player_mut(handle)?;
        player.encoded = EncodedMedia::ToneDevice(Some(sequence.to_vec()));
        if state == PlayerState::Realized {
            player.clip = clip;
        }
        player.position_micros = 0;
        player.completed_loops = 0;
        player.rendered_frames = 0;
        self.total_encoded_bytes = total_encoded_bytes;
        self.total_pcm_frames = total_pcm_frames;
        Ok(())
    }

    /// Queues a nonblocking single tone on the shared mixer.
    pub fn play_tone(
        &mut self,
        note: i32,
        duration_millis: i32,
        volume: i32,
        now_micros: i64,
    ) -> Result<(), EmuError> {
        if !(0..=127).contains(&note) || duration_millis <= 0 {
            return Err(media_error("illegal-argument", "invalid single tone"));
        }
        let volume = volume.clamp(0, 100);
        self.tick(now_micros)?;
        let frame_count = usize::try_from(duration_millis)
            .ok()
            .and_then(|value| value.checked_mul(OUTPUT_SAMPLE_RATE as usize))
            .map(|value| value / 1_000)
            .ok_or_else(|| media_error("media-limit", "single tone duration overflow"))?;
        if frame_count > self.limits.max_pcm_frames {
            return Err(media_error(
                "media-limit",
                "single tone exceeds duration limit",
            ));
        }
        let replaced_frames = self.one_shot.as_ref().map_or(0, |tone| tone.samples.len());
        let total_pcm_frames = self
            .total_pcm_frames
            .saturating_sub(replaced_frames)
            .checked_add(frame_count)
            .filter(|total| *total <= self.limits.max_total_pcm_frames)
            .ok_or_else(|| {
                media_error(
                    "media-limit",
                    "suite decoded audio exceeds aggregate PCM limit",
                )
            })?;
        let note = u8::try_from(note)
            .map_err(|_| media_error("illegal-argument", "invalid single tone note"))?;
        let cancelled = || self.decode_cancelled.as_ref().is_some_and(|check| check());
        let mut samples = Vec::new();
        append_synth_note(&mut samples, note, frame_count, volume, &cancelled)?;
        self.one_shot = Some(OneShot {
            samples: samples.into_boxed_slice(),
            rendered_frames: 0,
        });
        self.total_pcm_frames = total_pcm_frames;
        Ok(())
    }
}
