use super::{
    Arc, Clip, EmuError, EncodedMedia, Limits, OUTPUT_SAMPLE_RATE, Player, PlayerState,
    check_decode_cancellation, decode_midi, decode_tone_sequence, decode_wav, media_error, smaf,
};
use std::io::Cursor;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

pub(super) fn require_open(player: &Player) -> Result<(), EmuError> {
    if player.state == PlayerState::Closed {
        Err(media_error("player-closed", "MMAPI player is closed"))
    } else {
        Ok(())
    }
}

pub(super) fn require_realized(player: &Player) -> Result<(), EmuError> {
    require_open(player)?;
    if player.state == PlayerState::Unrealized {
        Err(media_error("illegal-state", "MMAPI player is unrealized"))
    } else {
        Ok(())
    }
}

const CONTENT_TYPES: &[(&str, &[&str])] = &[
    (
        "audio/x-wav",
        &["audio/wav", "audio/wave", "audio/vnd.wave"],
    ),
    (
        "audio/midi",
        &["audio/mid", "audio/x-mid", "audio/x-midi", "audio/sp-midi"],
    ),
    ("audio/x-tone-seq", &[]),
    (
        "audio/mmf",
        &["application/vnd.smaf", "application/vnd.yamaha.smaf-audio"],
    ),
    (
        "audio/mpeg",
        &["audio/mp3", "audio/x-mp3", "audio/mpeg3", "audio/x-mpeg"],
    ),
    (
        "audio/mp4a-latm",
        &["audio/mp4", "audio/aac", "audio/x-aac"],
    ),
    ("audio/amr", &["audio/amr-nb"]),
];

pub(super) fn canonical_content_type(value: &str) -> Result<&'static str, EmuError> {
    let base = value.split(';').next().unwrap_or(value).trim();
    CONTENT_TYPES
        .iter()
        .find(|(canonical, aliases)| {
            base.eq_ignore_ascii_case(canonical)
                || aliases.iter().any(|alias| base.eq_ignore_ascii_case(alias))
        })
        .map(|(canonical, _)| *canonical)
        .ok_or_else(|| {
            media_error(
                "media-unsupported-format",
                format!("unsupported MMAPI content type: {base}"),
            )
        })
}

pub(super) fn decode_player(
    player: &Player,
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    match (&player.encoded, player.content_type) {
        (EncodedMedia::Bytes(bytes), "audio/x-wav") => decode_wav(bytes, limits, cancelled),
        (EncodedMedia::Bytes(bytes), "audio/midi") => decode_midi(bytes, limits, cancelled),
        (EncodedMedia::Bytes(bytes), "audio/mmf") => smaf::decode(bytes, limits, cancelled),
        (EncodedMedia::Bytes(bytes), "audio/x-tone-seq") => {
            decode_tone_sequence(bytes, limits, cancelled)
        }
        (EncodedMedia::Bytes(bytes), "audio/mpeg") => {
            decode_symphonia(Arc::clone(bytes), "mp3", limits, cancelled)
        }
        (EncodedMedia::Bytes(bytes), "audio/mp4a-latm") => {
            decode_symphonia(Arc::clone(bytes), "m4a", limits, cancelled)
        }
        (EncodedMedia::Bytes(bytes), "audio/amr") => decode_amr_nb(bytes, limits, cancelled),
        (EncodedMedia::ToneDevice(Some(sequence)), _) => {
            decode_tone_sequence(sequence, limits, cancelled)
        }
        (EncodedMedia::ToneDevice(None) | EncodedMedia::MidiDevice, _) => Ok(Clip {
            samples: Box::default(),
            duration_micros: 0,
        }),
        (EncodedMedia::Decoded, _) => Err(media_error(
            "illegal-state",
            "decoded player cannot be realized again",
        )),
        (EncodedMedia::Closed, _) => Err(media_error("player-closed", "MMAPI player is closed")),
        _ => Err(media_error(
            "media-unsupported-format",
            "unsupported player format",
        )),
    }
}

pub(super) fn decode_amr_nb(
    bytes: &[u8],
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    let max_source_samples = limits
        .max_pcm_frames
        .checked_mul(amr_nb::SAMPLE_RATE as usize)
        .ok_or_else(|| media_error("media-limit", "AMR-NB duration limit overflow"))?
        / OUTPUT_SAMPLE_RATE as usize;
    let samples =
        amr_nb::decode_storage_file_with_cancellation(bytes, max_source_samples, cancelled)
            .map_err(|error| {
                let code = match error {
                    amr_nb::DecodeError::SampleLimit => "media-limit",
                    amr_nb::DecodeError::Cancelled => "execution-cancelled",
                    _ => "media-malformed",
                };
                media_error(code, format!("AMR-NB decode failed: {error}"))
            })?;
    resample_mono(samples, amr_nb::SAMPLE_RATE, limits, cancelled)
}

pub(super) fn micros_to_frames(micros: i64) -> usize {
    let micros = u128::try_from(micros.max(0)).unwrap_or_default();
    usize::try_from(micros * u128::from(OUTPUT_SAMPLE_RATE) / 1_000_000).unwrap_or(usize::MAX)
}

#[allow(clippy::too_many_lines)]
pub(super) fn decode_symphonia(
    bytes: Arc<[u8]>,
    extension: &str,
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    check_decode_cancellation(cancelled)?;
    if bytes.is_empty() {
        return Err(media_error(
            "media-malformed",
            "empty compressed audio stream",
        ));
    }
    let source = Cursor::new(bytes);
    let stream = MediaSourceStream::new(Box::new(source), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    hint.with_extension(extension);
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| {
            media_error(
                "media-malformed",
                format!("compressed audio probe failed: {error}"),
            )
        })?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| media_error("media-malformed", "compressed audio has no audio track"))?;
    let track_id = track.id;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| media_error("media-malformed", "audio codec parameters are missing"))?;
    let mut audio_decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|error| {
            media_error(
                "media-unsupported-format",
                format!("compressed audio codec is unsupported: {error}"),
            )
        })?;

    let mut source_rate = None;
    let mut source_samples = Vec::<i16>::new();
    let mut interleaved = Vec::<i16>::new();
    loop {
        check_decode_cancellation(cancelled)?;
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => {
                return Err(media_error(
                    "media-unsupported-format",
                    "dynamic compressed-audio track changes are unsupported",
                ));
            }
            Err(error) => {
                return Err(media_error(
                    "media-malformed",
                    format!("compressed audio packet error: {error}"),
                ));
            }
        };
        if packet.track_id != track_id {
            continue;
        }
        let audio_buffer = match audio_decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(SymphoniaError::IoError(_) | SymphoniaError::DecodeError(_)) => continue,
            Err(error) => {
                return Err(media_error(
                    "media-malformed",
                    format!("compressed audio decode failed: {error}"),
                ));
            }
        };
        let spec = audio_buffer.spec();
        let rate = spec.rate();
        let channels = spec.channels().count();
        if !(4_000..=192_000).contains(&rate) || channels == 0 || channels > 8 {
            return Err(media_error(
                "media-unsupported-format",
                "compressed audio uses unsupported sample rate or channel layout",
            ));
        }
        if let Some(previous) = source_rate {
            if previous != rate {
                return Err(media_error(
                    "media-unsupported-format",
                    "compressed audio changes sample rate mid-stream",
                ));
            }
        } else {
            source_rate = Some(rate);
        }
        let source_frames = source_samples
            .len()
            .checked_add(audio_buffer.frames())
            .ok_or_else(|| media_error("media-limit", "decoded audio duration overflow"))?;
        resampled_frame_count(source_frames, rate, limits.max_pcm_frames)?;
        audio_buffer.copy_to_vec_interleaved(&mut interleaved);
        if !interleaved.len().is_multiple_of(channels) {
            return Err(media_error(
                "media-malformed",
                "decoded audio contains a partial sample frame",
            ));
        }
        append_mono_samples(&mut source_samples, &interleaved, channels);
    }
    let rate = source_rate.ok_or_else(|| {
        media_error(
            "media-malformed",
            "compressed audio produced no decodable samples",
        )
    })?;
    resample_mono(source_samples, rate, limits, cancelled)
}

fn append_mono_samples(samples: &mut Vec<i16>, interleaved: &[i16], channels: usize) {
    // The decoder validates the channel count and complete frames first.
    match channels {
        1 => samples.extend_from_slice(interleaved),
        2 => samples.extend(
            interleaved
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[left, right]| i16::midpoint(left, right)),
        ),
        _ => {
            for frame in interleaved.chunks_exact(channels) {
                let sum = frame.iter().map(|sample| i64::from(*sample)).sum::<i64>();
                let mono = sum / i64::try_from(channels).unwrap_or(1);
                samples.push(
                    i16::try_from(mono.clamp(i64::from(i16::MIN), i64::from(i16::MAX)))
                        .expect("mono sample is clamped to i16"),
                );
            }
        }
    }
}

pub(super) fn resampled_frame_count(
    source_frames: usize,
    source_rate: u32,
    limit: usize,
) -> Result<usize, EmuError> {
    if source_rate == 0 {
        return Err(media_error("media-malformed", "audio sample rate is zero"));
    }
    let output_frames = source_frames
        .checked_mul(OUTPUT_SAMPLE_RATE as usize)
        .ok_or_else(|| media_error("media-limit", "audio duration overflow"))?
        / source_rate as usize;
    if output_frames > limit {
        return Err(media_error(
            "media-limit",
            "decoded audio exceeds duration limit",
        ));
    }
    Ok(output_frames)
}

pub(super) fn resample_mono(
    mut samples: Vec<i16>,
    source_rate: u32,
    limits: Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<Clip, EmuError> {
    check_decode_cancellation(cancelled)?;
    let output_frames = resampled_frame_count(samples.len(), source_rate, limits.max_pcm_frames)?;
    if samples.is_empty() {
        return Ok(Clip {
            samples: Box::default(),
            duration_micros: 0,
        });
    }
    let duration_micros = i64::try_from(
        u128::try_from(samples.len())
            .unwrap_or(u128::MAX)
            .saturating_mul(1_000_000)
            / u128::from(source_rate),
    )
    .unwrap_or(i64::MAX);
    if source_rate == OUTPUT_SAMPLE_RATE {
        return Ok(Clip {
            samples: samples.into_boxed_slice(),
            duration_micros,
        });
    }
    let source_frames = samples.len();
    if output_frames > source_frames {
        samples
            .try_reserve(output_frames - source_frames)
            .map_err(|_| media_error("media-limit", "decoded audio allocation failed"))?;
        samples.resize(output_frames, 0);
    }
    let denominator = u64::from(OUTPUT_SAMPLE_RATE);
    let resample = |samples: &[i16], index: usize| {
        let position = u64::try_from(index)
            .unwrap_or(u64::MAX)
            .saturating_mul(u64::from(source_rate));
        let base = usize::try_from(position / denominator)
            .unwrap_or(usize::MAX)
            .min(source_frames - 1);
        let next = (base + 1).min(source_frames - 1);
        let fraction =
            i64::try_from(position % denominator).expect("sample fraction is below the mixer rate");
        let left = i64::from(samples[base]);
        let right = i64::from(samples[next]);
        let interpolated = (left * (i64::from(OUTPUT_SAMPLE_RATE) - fraction) + right * fraction)
            / i64::from(OUTPUT_SAMPLE_RATE);
        i16::try_from(interpolated).expect("interpolation stays within the source sample range")
    };
    // Rounding may leave the frame count unchanged during slight upsampling.
    // The rate still determines which source samples must stay unmodified.
    if source_rate < OUTPUT_SAMPLE_RATE {
        for index in (0..output_frames).rev() {
            if index.is_multiple_of(4096) {
                check_decode_cancellation(cancelled)?;
            }
            samples[index] = resample(&samples, index);
        }
    } else if source_rate.is_multiple_of(OUTPUT_SAMPLE_RATE) {
        // Integer downsampling lands exactly on source samples. The validated
        // output count keeps index * stride inside the original buffer.
        let stride = (source_rate / OUTPUT_SAMPLE_RATE) as usize;
        for index in 0..output_frames {
            if index.is_multiple_of(4096) {
                check_decode_cancellation(cancelled)?;
            }
            samples[index] = samples[index * stride];
        }
    } else {
        for index in 0..output_frames {
            if index.is_multiple_of(4096) {
                check_decode_cancellation(cancelled)?;
            }
            samples[index] = resample(&samples, index);
        }
    }
    samples.truncate(output_frames);
    Ok(Clip {
        samples: samples.into_boxed_slice(),
        duration_micros,
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/mmapi/media_decode.rs"]
mod tests;
