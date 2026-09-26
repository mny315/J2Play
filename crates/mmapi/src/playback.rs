//! MMAPI playback operations on the suite-owned runtime.

use super::{
    AudioSink, Category, EmuError, EncodedMedia, OUTPUT_SAMPLE_RATE, PlayerEvent, PlayerEventKind,
    PlayerState, Runtime, clamp_i16, encoded_media_bytes, media_error, micros_to_frames,
    mix_one_shot, mix_player, require_open, require_realized,
};

#[allow(clippy::missing_errors_doc)]
impl<S: AudioSink> Runtime<S> {
    /// Dispatches the selector from [`natives::HostServices::mmapi_transition`].
    pub fn transition(
        &mut self,
        handle: u64,
        transition: i32,
        now_micros: i64,
    ) -> Result<(), EmuError> {
        match transition {
            0 => self.realize(handle),
            1 => self.prefetch(handle),
            2 => self.start(handle, now_micros),
            3 => self.stop(handle, now_micros),
            4 => self.deallocate(handle, now_micros),
            5 => self.close(handle, now_micros),
            _ => Err(EmuError::new(
                Category::Api,
                "native-arguments",
                "unknown MMAPI transition selector",
            )),
        }
    }

    /// Dispatches the selector from [`natives::HostServices::mmapi_time`].
    pub fn time(&mut self, handle: u64, selector: i32, now_micros: i64) -> Result<i64, EmuError> {
        match selector {
            0 => self.duration(handle),
            1 => self.media_time(handle, now_micros),
            2 => self.last_event_data(handle),
            _ => Err(EmuError::new(
                Category::Api,
                "native-arguments",
                "unknown MMAPI time selector",
            )),
        }
    }

    /// Advances all players and writes one mixed PCM block to the sink.
    ///
    /// # Errors
    /// Rejects an excessive host time jump or propagates a backend failure.
    pub fn tick(&mut self, now_micros: i64) -> Result<(), EmuError> {
        let now_micros = now_micros.max(self.last_tick_micros);
        if self.one_shot.is_none()
            && !self
                .players
                .values()
                .any(|player| player.state == PlayerState::Started)
        {
            self.last_tick_micros = now_micros;
            return Ok(());
        }
        let elapsed =
            u64::try_from(now_micros.saturating_sub(self.last_tick_micros)).unwrap_or(u64::MAX);
        let frame_numerator = u128::from(elapsed)
            .saturating_mul(u128::from(OUTPUT_SAMPLE_RATE))
            .saturating_add(u128::from(self.render_frame_remainder));
        let frame_count = usize::try_from(frame_numerator / 1_000_000).unwrap_or(usize::MAX);
        if frame_count > self.limits.max_tick_frames {
            return Err(media_error(
                "media-time-limit",
                "audio time advance exceeds render limit",
            ));
        }
        self.mix_scratch.clear();
        self.mix_scratch.resize(frame_count, 0);
        for player in self.players.values() {
            if player.state != PlayerState::Started {
                continue;
            }
            let Some(clip) = player.clip.as_ref() else {
                continue;
            };
            mix_player(player, clip, &mut self.mix_scratch);
        }
        if let Some(one_shot) = &self.one_shot {
            mix_one_shot(one_shot, &mut self.mix_scratch);
        }
        self.output_scratch.clear();
        self.output_scratch
            .extend(self.mix_scratch.iter().copied().map(clamp_i16));
        if !self.output_scratch.is_empty() {
            self.sink.write(&self.output_scratch)?;
        }
        // PCM cursors advance only after successful output. Reconstructing them
        // from rounded media times repeats or skips samples between small ticks.
        self.render_frame_remainder =
            u64::try_from(frame_numerator % 1_000_000).unwrap_or_default();
        for player in self.players.values_mut() {
            if player.state == PlayerState::Started {
                player.rendered_frames = player.rendered_frames.saturating_add(frame_count);
            }
        }
        self.finish_players(now_micros);
        if let Some(tone) = self.one_shot.take_if(|tone| {
            tone.rendered_frames = tone.rendered_frames.saturating_add(frame_count);
            tone.rendered_frames >= tone.samples.len()
        }) {
            self.total_pcm_frames = self.total_pcm_frames.saturating_sub(tone.samples.len());
        }
        self.last_tick_micros = now_micros;
        Ok(())
    }

    fn finish_players(&mut self, now_micros: i64) {
        for player in self.players.values_mut() {
            if player.state != PlayerState::Started {
                continue;
            }
            let duration = player.duration_micros();
            if duration <= 0 {
                player.position_micros = duration.max(0);
                player.state = PlayerState::Prefetched;
                player.queue_bounded(self.limits, PlayerEventKind::EndOfMedia, duration.max(0));
                continue;
            }
            let elapsed = now_micros.saturating_sub(player.started_at_micros);
            let absolute = player.start_position_micros.saturating_add(elapsed);
            let rendered_loops = player
                .clip
                .as_ref()
                .map_or(0, |clip| player.rendered_frames / clip.samples.len().max(1));
            let allowed = if player.loop_count == -1 {
                u64::MAX
            } else {
                u64::try_from(player.loop_count).unwrap_or(0)
            };
            let remaining = allowed.saturating_sub(player.completed_loops);
            let newly_completed = u64::try_from(rendered_loops)
                .unwrap_or(u64::MAX)
                .saturating_sub(player.completed_loops)
                .min(remaining);
            let total_completed = player.completed_loops.saturating_add(newly_completed);
            let emit = newly_completed
                .min(u64::try_from(self.limits.max_pending_events).unwrap_or(u64::MAX));
            for index in 0..emit {
                player.queue_bounded(self.limits, PlayerEventKind::EndOfMedia, duration);
                if total_completed < allowed || index + 1 < emit {
                    player.queue_bounded(self.limits, PlayerEventKind::Started, 0);
                }
            }
            if total_completed >= allowed {
                player.completed_loops = allowed;
                player.position_micros = duration;
                player.state = PlayerState::Prefetched;
            } else {
                player.completed_loops = total_completed;
                player.position_micros = absolute.rem_euclid(duration);
                player.start_position_micros = player.position_micros;
                player.started_at_micros = now_micros;
            }
        }
    }

    /// Returns a state after polling elapsed host time.
    pub fn state(&mut self, handle: u64, now_micros: i64) -> Result<PlayerState, EmuError> {
        self.tick(now_micros)?;
        Ok(self.player(handle)?.state)
    }

    /// Starts or resumes playback without waiting for media completion.
    pub fn start(&mut self, handle: u64, now_micros: i64) -> Result<(), EmuError> {
        self.tick(now_micros)?;
        self.prefetch(handle)?;
        if self.player(handle)?.state == PlayerState::Started {
            return Ok(());
        }
        let started_players = self
            .players
            .values()
            .filter(|player| player.state == PlayerState::Started);
        if started_players.clone().count() >= self.limits.max_started_players {
            return Err(media_error(
                "audio-resource-limit",
                "simultaneous audio player limit reached",
            ));
        }
        if self.player(handle)?.content_type == "audio/midi"
            && started_players
                .filter(|player| player.content_type == "audio/midi")
                .count()
                >= self.limits.max_started_midi_players
        {
            return Err(media_error(
                "audio-resource-limit",
                "simultaneous MIDI player limit reached",
            ));
        }
        let limits = self.limits;
        let player = self.player_mut(handle)?;
        let duration = player.duration_micros();
        let restarting = duration >= 0 && player.position_micros >= duration;
        let position_micros = if restarting {
            0
        } else {
            player.position_micros
        };
        player.queue(limits, PlayerEventKind::Started, position_micros)?;
        player.position_micros = position_micros;
        if restarting {
            player.completed_loops = 0;
            player.rendered_frames = 0;
        }
        player.start_position_micros = position_micros;
        player.started_at_micros = now_micros;
        player.state = PlayerState::Started;
        Ok(())
    }

    /// Starts one player while replacing the previous member of the same
    /// compatibility group. Ordinary MMAPI players are not part of the group.
    pub fn start_exclusive(&mut self, handle: u64, now_micros: i64) -> Result<(), EmuError> {
        // Validate and decode the replacement before silencing a working clip.
        self.prefetch(handle)?;
        if let Some(previous) = self.exclusive_player
            && previous != handle
        {
            self.stop(previous, now_micros)?;
        }
        self.start(handle, now_micros)?;
        self.exclusive_player = Some(handle);
        Ok(())
    }

    /// Stops playback in PREFETCHED while preserving position and loop progress.
    pub fn stop(&mut self, handle: u64, now_micros: i64) -> Result<(), EmuError> {
        self.tick(now_micros)?;
        let limits = self.limits;
        let player = self.player_mut(handle)?;
        require_open(player)?;
        if player.state == PlayerState::Started {
            player.queue(limits, PlayerEventKind::Stopped, player.position_micros)?;
            player.state = PlayerState::Prefetched;
        }
        if self.exclusive_player == Some(handle) {
            self.exclusive_player = None;
        }
        Ok(())
    }

    /// Releases prefetched resources and returns to REALIZED.
    pub fn deallocate(&mut self, handle: u64, now_micros: i64) -> Result<(), EmuError> {
        require_open(self.player(handle)?)?;
        let stopped = self.stop(handle, now_micros);
        let limits = self.limits;
        let player = self.player_mut(handle)?;
        // Rendering or listener backpressure cannot prevent resource release.
        if player.state == PlayerState::Started {
            player.queue_bounded(limits, PlayerEventKind::Stopped, player.position_micros);
        }
        if matches!(player.state, PlayerState::Started | PlayerState::Prefetched) {
            player.state = PlayerState::Realized;
        }
        if self.exclusive_player == Some(handle) {
            self.exclusive_player = None;
        }
        stopped
    }

    /// Closes a player permanently. Repeated close is harmless.
    pub fn close(&mut self, handle: u64, now_micros: i64) -> Result<(), EmuError> {
        if self.player(handle)?.state == PlayerState::Closed {
            return Ok(());
        }
        // Preserve output diagnostics, but release the player even if output fails.
        let advanced = self.tick(now_micros);
        let limits = self.limits;
        let (pcm_frames, encoded_bytes) = {
            let player = self.player(handle)?;
            (
                player.clip.as_ref().map_or(0, |clip| clip.samples.len()),
                encoded_media_bytes(&player.encoded),
            )
        };
        {
            let player = self.player_mut(handle)?;
            player.queue_bounded(limits, PlayerEventKind::Closed, player.position_micros);
            player.state = PlayerState::Closed;
            player.clip = None;
            player.encoded = EncodedMedia::Closed;
        }
        self.total_pcm_frames = self.total_pcm_frames.saturating_sub(pcm_frames);
        self.total_encoded_bytes = self.total_encoded_bytes.saturating_sub(encoded_bytes);
        if self.exclusive_player == Some(handle) {
            self.exclusive_player = None;
        }
        advanced
    }

    pub fn media_time(&mut self, handle: u64, now_micros: i64) -> Result<i64, EmuError> {
        self.tick(now_micros)?;
        let player = self.player(handle)?;
        require_open(player)?;
        Ok(player.position_micros)
    }

    pub fn set_media_time(
        &mut self,
        handle: u64,
        media_time_micros: i64,
        now_micros: i64,
    ) -> Result<i64, EmuError> {
        self.tick(now_micros)?;
        let player = self.player_mut(handle)?;
        require_realized(player)?;
        let duration = player.duration_micros();
        let actual = media_time_micros.max(0).min(duration.max(0));
        player.position_micros = actual;
        player.completed_loops = 0;
        // The duration is rounded to whole microseconds. Converting it back
        // can point at the last sample instead of the end of the PCM buffer.
        player.rendered_frames = if actual == duration {
            player.clip.as_ref().map_or(0, |clip| clip.samples.len())
        } else {
            micros_to_frames(actual)
        };
        if player.state == PlayerState::Started {
            player.start_position_micros = actual;
            player.started_at_micros = now_micros;
        }
        Ok(actual)
    }

    pub fn set_loop_count(&mut self, handle: u64, count: i32) -> Result<(), EmuError> {
        let player = self.player_mut(handle)?;
        require_open(player)?;
        if player.state == PlayerState::Started {
            return Err(media_error(
                "illegal-state",
                "loop count cannot change while started",
            ));
        }
        if count == 0 || count < -1 {
            return Err(media_error("illegal-argument", "invalid loop count"));
        }
        player.loop_count = count;
        player.completed_loops = 0;
        // Changing the loop limit preserves the exact cursor within the clip;
        // rounded media time can drift from it after several short loops.
        player.rendered_frames = player
            .clip
            .as_ref()
            .map_or(0, |clip| player.rendered_frames % clip.samples.len().max(1));
        Ok(())
    }

    pub fn next_event(
        &mut self,
        handle: u64,
        now_micros: i64,
    ) -> Result<Option<PlayerEvent>, EmuError> {
        if self.player(handle)?.events.is_empty() {
            self.tick(now_micros)?;
        }
        let player = self.player_mut(handle)?;
        let event = player.events.pop_front();
        if let Some(event) = event {
            player.last_event_data = event.media_time_micros;
        }
        Ok(event)
    }

    pub fn last_event_data(&self, handle: u64) -> Result<i64, EmuError> {
        Ok(self.player(handle)?.last_event_data)
    }
}
