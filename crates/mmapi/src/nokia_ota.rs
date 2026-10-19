//! Nokia Smart Messaging 3.0.0, section 3.6: basic and temporary OTA songs.
//! Format reference: <https://www.scribd.com/document/655577912/sms3-0-0>
//! This bit stream is distinct from the MIDP `ToneControl` sequence format.

use super::{
    Clip, EmuError, Limits, OUTPUT_SAMPLE_RATE, append_synth_note, check_decode_cancellation,
    media_error,
};

const TEMPOS: [u16; 32] = [
    25, 28, 31, 35, 40, 45, 50, 56, 63, 70, 80, 90, 100, 112, 125, 140, 160, 180, 200, 225, 250,
    285, 320, 355, 400, 450, 500, 565, 635, 715, 800, 900,
];

struct Bits<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Bits<'_> {
    fn read(&mut self, width: usize) -> Result<u8, EmuError> {
        let mut value = 0;
        for _ in 0..width {
            let byte = self
                .bytes
                .get(self.position / 8)
                .ok_or_else(|| malformed("truncated bit stream"))?;
            value = (value << 1) | ((byte >> (7 - self.position % 8)) & 1);
            self.position += 1;
        }
        Ok(value)
    }

    fn align(&mut self) -> Result<(), EmuError> {
        let padding = (8 - self.position % 8) % 8;
        if self.read(padding)? != 0 {
            return Err(malformed("nonzero command padding"));
        }
        Ok(())
    }
}

struct Note {
    pitch: u8,
    volume: i32,
    sounding_frames: usize,
    end_frame: usize,
}

struct Song {
    notes: Vec<Note>,
    nanos: u64,
    frames: usize,
    instructions: usize,
    scale: u8,
    style: u8,
    tempo: u16,
    volume: u8,
}

impl Song {
    fn new() -> Self {
        Self {
            notes: Vec::new(),
            nanos: 0,
            frames: 0,
            instructions: 0,
            scale: 1,
            style: 0,
            tempo: 63,
            volume: 7,
        }
    }

    fn instruction(&mut self, bits: &mut Bits<'_>, limits: Limits) -> Result<(), EmuError> {
        if self.instructions >= limits.max_tone_events {
            return Err(media_error(
                "media-limit",
                "Nokia OTA instruction limit reached",
            ));
        }
        self.instructions += 1;
        match bits.read(3)? {
            1 => self.note(bits, limits)?,
            2 => self.scale = bits.read(2)?,
            3 => {
                self.style = bits.read(2)?;
                if self.style == 3 {
                    return Err(malformed("reserved style"));
                }
            }
            4 => self.tempo = TEMPOS[usize::from(bits.read(5)?)],
            5 => self.volume = bits.read(4)?,
            _ => return Err(malformed("invalid pattern instruction")),
        }
        Ok(())
    }

    fn note(&mut self, bits: &mut Bits<'_>, limits: Limits) -> Result<(), EmuError> {
        let pitch = bits.read(4)?;
        let duration = bits.read(3)?;
        let modifier = bits.read(2)?;
        if pitch > 12 || duration > 5 {
            return Err(malformed("reserved note value or duration"));
        }
        let (numerator, denominator) = match modifier {
            1 => (3, 2),
            2 => (7, 4),
            3 => (2, 3),
            _ => (1, 1),
        };
        let nanos = 240_000_000_000_u64 * numerator
            / (u64::from(self.tempo) * (1 << duration) * denominator);
        self.nanos = self
            .nanos
            .checked_add(nanos)
            .ok_or_else(|| media_error("media-limit", "Nokia OTA duration overflow"))?;
        let end_frame = usize::try_from(
            u128::from(self.nanos) * u128::from(OUTPUT_SAMPLE_RATE) / 1_000_000_000,
        )
        .ok()
        .filter(|frames| *frames <= limits.max_pcm_frames)
        .ok_or_else(|| media_error("media-limit", "Nokia OTA PCM limit reached"))?;
        let frames = end_frame - self.frames;
        let sounding_frames = if pitch == 0 || self.volume == 0 {
            0
        } else {
            // The specification leaves articulation lengths to the renderer.
            match self.style {
                0 => frames - frames / 8,
                2 => frames / 2,
                _ => frames,
            }
        };
        self.notes.push(Note {
            pitch: 60 + self.scale * 12 + pitch.saturating_sub(1),
            volume: i32::from(self.volume) * 100 / 15,
            sounding_frames,
            end_frame,
        });
        self.frames = end_frame;
        Ok(())
    }

    fn patterns(
        &mut self,
        bits: &mut Bits<'_>,
        limits: Limits,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EmuError> {
        let count = bits.read(8)?;
        if count == 0 {
            return Err(malformed("song has no patterns"));
        }
        let mut patterns = [None; 4];
        for _ in 0..count {
            check_decode_cancellation(cancelled)?;
            if bits.read(3)? != 0 {
                return Err(malformed("missing pattern header"));
            }
            let id = usize::from(bits.read(2)?);
            let repeats = bits.read(4)?;
            if repeats == 15 {
                return Err(media_error(
                    "media-unsupported-format",
                    "infinite Nokia OTA pattern loops are unsupported; use Sound.play(0)",
                ));
            }
            let length = bits.read(8)?;
            let after_header = bits.position;
            let (start, length) = if length == 0 {
                patterns[id].ok_or_else(|| malformed("undefined pattern reference"))?
            } else {
                patterns[id] = Some((bits.position, length));
                (bits.position, length)
            };
            let mut end = after_header;
            for _ in 0..=repeats {
                bits.position = start;
                for _ in 0..length {
                    check_decode_cancellation(cancelled)?;
                    self.instruction(bits, limits)?;
                }
                if start == after_header {
                    end = bits.position;
                }
            }
            bits.position = end;
        }
        Ok(())
    }
}

pub(super) fn decode(
    bytes: &[u8],
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    check_decode_cancellation(cancelled)?;
    if bytes.len() > limits.max_input_bytes {
        return Err(media_error("media-limit", "Nokia OTA input limit reached"));
    }
    let mut bits = Bits { bytes, position: 0 };
    let mut song = Song::new();
    let mut programmed = false;
    let mut unicode = false;
    let mut sounded = false;
    loop {
        check_decode_cancellation(cancelled)?;
        let parts = bits.read(8)?;
        if parts == 0 {
            break;
        }
        for _ in 0..parts {
            check_decode_cancellation(cancelled)?;
            match bits.read(7)? {
                0x25 if !programmed => programmed = true,
                0x22 if programmed && !sounded => unicode = true,
                0x05 if programmed && !sounded => {
                    if bits.read(7)? != 0x22 {
                        return Err(malformed("invalid cancel command"));
                    }
                    unicode = false;
                }
                0x1d if programmed && !sounded => {
                    match bits.read(3)? {
                        1 => {
                            let length = bits.read(4)?;
                            for _ in 0..length {
                                bits.read(8)?;
                                if unicode {
                                    bits.read(8)?;
                                }
                            }
                        }
                        2 => {}
                        _ => {
                            return Err(media_error(
                                "media-unsupported-format",
                                "unsupported Nokia OTA song type",
                            ));
                        }
                    }
                    song.patterns(&mut bits, limits, cancelled)?;
                    sounded = true;
                }
                _ => return Err(malformed("invalid command order or command type")),
            }
            bits.align()?;
        }
    }
    if !sounded || song.notes.is_empty() {
        return Err(malformed("song has no notes"));
    }
    for tail in bytes[bits.position / 8..].chunks(4096) {
        check_decode_cancellation(cancelled)?;
        if tail.iter().any(|byte| *byte != 0) {
            return Err(malformed("data after command end"));
        }
    }
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(song.frames)
        .map_err(|_| media_error("media-limit", "Nokia OTA PCM allocation failed"))?;
    for note in song.notes {
        check_decode_cancellation(cancelled)?;
        append_synth_note(
            &mut samples,
            note.pitch,
            note.sounding_frames,
            note.volume,
            cancelled,
        )?;
        samples.resize(note.end_frame, 0);
    }
    Ok(Clip {
        samples: samples.into_boxed_slice(),
        duration_micros: i64::try_from(song.nanos / 1000)
            .map_err(|_| media_error("media-limit", "Nokia OTA duration overflow"))?,
    })
}

fn malformed(detail: &str) -> EmuError {
    media_error(
        "media-malformed",
        format!("invalid Nokia OTA ringtone: {detail}"),
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/mmapi/nokia_ota.rs"]
mod tests;
