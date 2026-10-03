use super::{
    Clip, EmuError, HashMap, Limits, OUTPUT_SAMPLE_RATE, append_synth_note,
    check_decode_cancellation, media_error,
};

struct ToneBlock {
    events: Vec<ToneEvent>,
    depth: usize,
}

pub(super) struct PreparedToneSequence {
    events: Vec<ToneEvent>,
    tempo: u8,
    resolution: u8,
    pub(super) frames: usize,
}

impl PreparedToneSequence {
    fn new(
        events: Vec<ToneEvent>,
        tempo: u8,
        resolution: u8,
        limits: Limits,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EmuError> {
        let mut total_duration_micros = 0;
        let mut frames = 0;
        for event in &events {
            check_decode_cancellation(cancelled)?;
            if let ToneEvent::Tone { duration, .. } = *event {
                frames = tone_target_frames(
                    &mut total_duration_micros,
                    duration,
                    u64::from(tempo) * 4,
                    resolution,
                    limits.max_pcm_frames,
                )?;
            }
        }
        Ok(Self {
            events,
            tempo,
            resolution,
            frames,
        })
    }
    pub(super) fn render(
        &self,
        limits: Limits,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Clip, EmuError> {
        check_decode_cancellation(cancelled)?;
        if self.frames > limits.max_pcm_frames {
            return Err(media_error("media-limit", "tone PCM limit reached"));
        }
        let tempo = u64::from(self.tempo) * 4;
        let mut volume = 100;
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(self.frames)
            .map_err(|_| media_error("media-limit", "tone PCM allocation failed"))?;
        let mut total_duration_micros = 0_u64;
        for event in &self.events {
            check_decode_cancellation(cancelled)?;
            match *event {
                ToneEvent::Volume(value) => volume = i32::from(value),
                ToneEvent::Tone { note, duration } => {
                    let target_frames = tone_target_frames(
                        &mut total_duration_micros,
                        duration,
                        tempo,
                        self.resolution,
                        limits.max_pcm_frames,
                    )?;
                    let frames = target_frames.saturating_sub(samples.len());
                    if note == -1 {
                        samples.resize(samples.len() + frames, 0);
                    } else {
                        append_synth_note(
                            &mut samples,
                            note.cast_unsigned(),
                            frames,
                            volume,
                            cancelled,
                        )?;
                    }
                }
            }
        }
        Ok(Clip {
            samples: samples.into_boxed_slice(),
            duration_micros: i64::try_from(total_duration_micros).unwrap_or(i64::MAX),
        })
    }
}

pub(super) fn decode_tone_sequence(
    bytes: &[u8],
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    prepare_tone_sequence(bytes, limits, cancelled)?.render(limits, cancelled)
}

pub(super) fn prepare_tone_sequence(
    bytes: &[u8],
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedToneSequence, EmuError> {
    if bytes.len() < 4 || bytes[0].cast_signed() != -2 || bytes[1] != 1 {
        return Err(media_error(
            "tone-sequence",
            "tone sequence must start with VERSION 1",
        ));
    }
    let mut tempo = 30_u8;
    let mut resolution = 64_u8;
    let mut seen_tempo = false;
    let mut seen_resolution = false;
    let mut seen_block = false;
    let mut offset = 2;
    let mut blocks = HashMap::<u8, ToneBlock>::new();
    while offset < bytes.len() {
        check_decode_cancellation(cancelled)?;
        match bytes[offset].cast_signed() {
            -3 => {
                if seen_tempo || seen_block {
                    return Err(media_error("tone-sequence", "duplicate or late TEMPO"));
                }
                let value = *bytes
                    .get(offset + 1)
                    .ok_or_else(|| media_error("tone-sequence", "truncated TEMPO"))?;
                if !(5..=127).contains(&value) {
                    return Err(media_error("tone-sequence", "invalid TEMPO"));
                }
                tempo = value;
                seen_tempo = true;
                offset += 2;
            }
            -4 => {
                if seen_resolution || seen_block {
                    return Err(media_error("tone-sequence", "duplicate or late RESOLUTION"));
                }
                let value = *bytes
                    .get(offset + 1)
                    .ok_or_else(|| media_error("tone-sequence", "truncated RESOLUTION"))?;
                if !(1..=127).contains(&value) {
                    return Err(media_error("tone-sequence", "invalid RESOLUTION"));
                }
                resolution = value;
                seen_resolution = true;
                offset += 2;
            }
            -5 => {
                seen_block = true;
                let number = *bytes
                    .get(offset + 1)
                    .ok_or_else(|| media_error("tone-sequence", "truncated BLOCK_START"))?;
                if number > 127 || blocks.contains_key(&number) {
                    return Err(media_error("tone-sequence", "invalid duplicate tone block"));
                }
                let start = offset + 2;
                offset = start;
                loop {
                    check_decode_cancellation(cancelled)?;
                    let command = (*bytes
                        .get(offset)
                        .ok_or_else(|| media_error("tone-sequence", "unterminated tone block"))?)
                    .cast_signed();
                    if command == -6 {
                        let end_number = *bytes
                            .get(offset + 1)
                            .ok_or_else(|| media_error("tone-sequence", "truncated BLOCK_END"))?;
                        if end_number != number {
                            return Err(media_error("tone-sequence", "tone block number mismatch"));
                        }
                        // Definitions may only reference earlier blocks. Keep
                        // their already validated expansion so empty nested
                        // blocks cannot amplify work without producing events.
                        let block = expand_tone_events(
                            &bytes[start..offset],
                            &blocks,
                            limits.max_tone_events,
                            cancelled,
                        )?;
                        blocks.insert(number, block);
                        offset += 2;
                        break;
                    }
                    offset = skip_tone_event(bytes, offset)?;
                }
            }
            _ => break,
        }
    }
    if offset >= bytes.len() {
        return Err(media_error("tone-sequence", "tone sequence has no events"));
    }
    let block = expand_tone_events(&bytes[offset..], &blocks, limits.max_tone_events, cancelled)?;
    PreparedToneSequence::new(block.events, tempo, resolution, limits, cancelled)
}

#[derive(Clone, Copy, Debug)]
enum ToneEvent {
    Tone { note: i8, duration: u8 },
    Volume(u8),
}

fn skip_tone_event(bytes: &[u8], offset: usize) -> Result<usize, EmuError> {
    let command = (*bytes
        .get(offset)
        .ok_or_else(|| media_error("tone-sequence", "truncated event"))?)
    .cast_signed();
    let length = match command {
        -7 | -8 => 2,
        -9 => 4,
        -6..=-2 => {
            return Err(media_error(
                "tone-sequence",
                "invalid command inside tone block",
            ));
        }
        _ => 2,
    };
    let end = offset
        .checked_add(length)
        .ok_or_else(|| media_error("tone-sequence", "tone event overflow"))?;
    if end > bytes.len() {
        return Err(media_error("tone-sequence", "truncated tone event"));
    }
    Ok(end)
}

fn expand_tone_events(
    bytes: &[u8],
    blocks: &HashMap<u8, ToneBlock>,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<ToneBlock, EmuError> {
    let mut output = Vec::new();
    let mut depth = 0;
    let mut offset = 0;
    while offset < bytes.len() {
        check_decode_cancellation(cancelled)?;
        let command = bytes[offset].cast_signed();
        match command {
            -7 => {
                let block = *bytes
                    .get(offset + 1)
                    .ok_or_else(|| media_error("tone-sequence", "truncated PLAY_BLOCK"))?;
                let block = blocks
                    .get(&block)
                    .ok_or_else(|| media_error("tone-sequence", "undefined tone block"))?;
                depth = depth.max(block.depth + 1);
                if depth > 16 {
                    return Err(media_error("tone-sequence", "tone block recursion limit"));
                }
                require_event_capacity(output.len(), block.events.len(), limit)?;
                output.extend_from_slice(&block.events);
                offset += 2;
            }
            -8 => {
                let volume = *bytes
                    .get(offset + 1)
                    .ok_or_else(|| media_error("tone-sequence", "truncated SET_VOLUME"))?;
                if volume > 100 {
                    return Err(media_error("tone-sequence", "invalid tone volume"));
                }
                require_event_capacity(output.len(), 1, limit)?;
                output.push(ToneEvent::Volume(volume));
                offset += 2;
            }
            -9 => {
                let fields = bytes
                    .get(offset + 1..offset + 4)
                    .ok_or_else(|| media_error("tone-sequence", "truncated REPEAT"))?;
                if !(2..=127).contains(&fields[0]) {
                    return Err(media_error("tone-sequence", "invalid repeat multiplier"));
                }
                validate_tone(fields[1].cast_signed(), fields[2])?;
                require_event_capacity(output.len(), usize::from(fields[0]), limit)?;
                for _ in 0..fields[0] {
                    output.push(ToneEvent::Tone {
                        note: fields[1].cast_signed(),
                        duration: fields[2],
                    });
                }
                offset += 4;
            }
            -1..=127 => {
                let duration = *bytes
                    .get(offset + 1)
                    .ok_or_else(|| media_error("tone-sequence", "truncated tone pair"))?;
                validate_tone(command, duration)?;
                require_event_capacity(output.len(), 1, limit)?;
                output.push(ToneEvent::Tone {
                    note: command,
                    duration,
                });
                offset += 2;
            }
            _ => return Err(media_error("tone-sequence", "unexpected tone command")),
        }
    }
    Ok(ToneBlock {
        events: output,
        depth,
    })
}

fn require_event_capacity(current: usize, additional: usize, limit: usize) -> Result<(), EmuError> {
    if additional > limit.saturating_sub(current) {
        Err(media_error("media-limit", "tone event limit reached"))
    } else {
        Ok(())
    }
}

fn validate_tone(note: i8, duration: u8) -> Result<(), EmuError> {
    if note < -1 || duration == 0 || duration > 127 {
        Err(media_error("tone-sequence", "invalid tone pair"))
    } else {
        Ok(())
    }
}

fn tone_target_frames(
    total_duration_micros: &mut u64,
    duration: u8,
    tempo: u64,
    resolution: u8,
    limit: usize,
) -> Result<usize, EmuError> {
    let duration_micros = u64::from(duration)
        .saturating_mul(60)
        .saturating_mul(1_000_000)
        .saturating_mul(4)
        / u64::from(resolution)
        / tempo;
    *total_duration_micros = total_duration_micros.saturating_add(duration_micros);
    let frames = usize::try_from(
        total_duration_micros.saturating_mul(u64::from(OUTPUT_SAMPLE_RATE)) / 1_000_000,
    )
    .unwrap_or(usize::MAX);
    if frames > limit {
        return Err(media_error("media-limit", "tone PCM limit reached"));
    }
    Ok(frames)
}
