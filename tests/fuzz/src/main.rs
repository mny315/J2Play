use std::io::Read;

mod http_date;

const MAX_FUZZ_INPUT: usize = 1024 * 1024;

fn main() -> Result<(), std::io::Error> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (mode, iterations) = options(&arguments)?;
    let mut input = Vec::new();
    std::io::stdin()
        .take(u64::try_from(MAX_FUZZ_INPUT).expect("fuzz input limit fits u64"))
        .read_to_end(&mut input)?;
    let mut state = 0x1841_1a5e_u64;
    let mut sample = if input.is_empty() {
        if mode == "m3g-file" {
            valid_m3g_file()
        } else if mode == "m3g-section" {
            object(9, &valid_group_data())
        } else if mode == "m3g-object" {
            let mut seed = vec![9];
            seed.extend(valid_group_data());
            seed
        } else if mode == "smaf" {
            valid_smaf_file()
        } else if mode == "midi" {
            valid_midi_file()
        } else if mode == "wav" {
            valid_wav_file()
        } else if mode == "amr" {
            valid_amr_file()
        } else if mode == "tone" {
            vec![
                0xfe, 1, 0xfd, 100, 0xfb, 0, 69, 1, 0xfa, 0, 0xf9, 0, 0xf7, 2, 72, 1,
            ]
        } else if mode == "character-encoding" {
            b"\0A\xc3\xa9\xef\xbb\xbf\xf0\x9f\x98\x80\xff".to_vec()
        } else if mode == "http-date" {
            let mut seed = 1_749_990_645_000_i64.to_le_bytes().to_vec();
            seed.extend_from_slice(b"Sunday, 15-Jun-75 12:30:46 GMT");
            seed
        } else if mode.starts_with("micro3d-") {
            valid_micro3d_resource(mode)
        } else {
            input
        }
    } else {
        input
    };
    let seed = sample.clone();
    for iteration in 0..iterations {
        exercise(mode, &sample);
        if iteration + 1 < iterations {
            // Revisit the structured seed instead of eventually spending
            // every iteration rejecting a permanently destroyed header.
            if (iteration + 1).is_multiple_of(64) {
                sample.clone_from(&seed);
            }
            mutate(&mut sample, &mut state);
        }
    }
    Ok(())
}

fn options(arguments: &[String]) -> Result<(&str, usize), std::io::Error> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "expected a known parser mode and a positive iteration count",
        )
    };
    let mode = arguments.first().map_or("all", String::as_str);
    if arguments.len() > 2
        || !matches!(
            mode,
            "all"
                | "classfile"
                | "image"
                | "jar"
                | "m3g-file"
                | "m3g-section"
                | "m3g-object"
                | "micro3d-figure"
                | "micro3d-texture"
                | "micro3d-action"
                | "smaf"
                | "midi"
                | "wav"
                | "amr"
                | "tone"
                | "character-encoding"
                | "http-date"
        )
    {
        return Err(invalid());
    }
    let iterations = arguments
        .get(1)
        .map_or(Ok(1), |value| value.parse::<usize>())
        .map_err(|_| invalid())?;
    if iterations == 0 {
        return Err(invalid());
    }
    Ok((mode, iterations))
}

fn valid_micro3d_resource(mode: &str) -> Vec<u8> {
    match mode {
        "micro3d-figure" => {
            let mut bytes = b"MB\x05\0\x02\0\x03\x01\x01\0".to_vec();
            // One origin vertex, no polygons, normals or bones.
            bytes.extend_from_slice(&[0; 20]);
            bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");
            bytes
        }
        "micro3d-action" => {
            let mut bytes = b"MT\x05\0\x02\0\x01\0".to_vec();
            bytes.extend_from_slice(&[0; 20]);
            // Compact rotation channels with omitted identity channels, then
            // a constant translation and roll in the second action.
            bytes.extend_from_slice(&[6, 0, 5, 2, 0]);
            for value in [0_i16, 0, 0, 4096, 6, 4096, 0, 0] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            bytes.extend_from_slice(&[0, 0, 17, 0, 3]);
            for value in [10_i16, -20, 30] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
            bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");
            bytes
        }
        "micro3d-texture" => {
            let mut bytes = vec![0; 66];
            bytes[..2].copy_from_slice(b"BM");
            bytes[2..6].copy_from_slice(&66_u32.to_le_bytes());
            bytes[10..14].copy_from_slice(&62_u32.to_le_bytes());
            bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
            bytes[18..22].copy_from_slice(&1_i32.to_le_bytes());
            bytes[22..26].copy_from_slice(&1_i32.to_le_bytes());
            bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
            bytes[28..30].copy_from_slice(&8_u16.to_le_bytes());
            bytes[34..38].copy_from_slice(&4_u32.to_le_bytes());
            bytes[46..50].copy_from_slice(&2_u32.to_le_bytes());
            bytes[54..58].copy_from_slice(&[255, 0, 0, 0]);
            bytes[58..62].copy_from_slice(&[255, 255, 255, 0]);
            bytes
        }
        _ => vec![0; 32],
    }
}

#[cfg(test)]
#[path = "../../unit/parser-fuzz/mod.rs"]
mod tests;

fn exercise(mode: &str, input: &[u8]) {
    match mode {
        "classfile" => {
            let _ = classfile::parse_prefix(input);
            let _ = classfile::parse(input);
            let mut remaining_bytes = 64 * 1024;
            let _ = classfile::parse_with_budget(input, &mut remaining_bytes);
        }
        "image" => exercise_image(input),
        "jar" => exercise_jar(input),
        "m3g-file" => {
            let _ = m3g::M3gFile::parse(input, m3g::LoaderLimits::default());
            let _ = m3g::M3gFile::parse_prefix(input, m3g::LoaderLimits::default());
        }
        "m3g-section" => {
            let _ = m3g::parse_section_object_stream(input, m3g::LoaderLimits::default());
        }
        "m3g-object" => exercise_m3g_object(input),
        "micro3d-figure" => {
            let _ = micro3d::FigureData::parse(input, micro3d::LoaderLimits::default());
        }
        "micro3d-texture" => {
            let _ = micro3d::TextureData::parse(input, true, micro3d::LoaderLimits::default());
        }
        "micro3d-action" => {
            let _ = micro3d::ActionTableData::parse(input, micro3d::LoaderLimits::default());
        }
        "smaf" => exercise_audio("audio/mmf", input),
        "midi" => exercise_audio("audio/midi", input),
        "wav" => exercise_audio("audio/wav", input),
        "amr" => exercise_audio("audio/amr", input),
        "tone" => exercise_audio("audio/x-tone-seq", input),
        "character-encoding" => exercise_character_encoding(input),
        "http-date" => http_date::exercise(input),
        _ => {
            let _ = classfile::parse(input);
            let _ = bytecode::decode(input);
            exercise_character_encoding(input);
            http_date::exercise(input);
            exercise_image(input);
            exercise_jar(input);
            let _ = jar::parse_jad(input);
            let _ = m3g::M3gFile::parse(input, m3g::LoaderLimits::default());
            let _ = m3g::M3gFile::parse_prefix(input, m3g::LoaderLimits::default());
            let _ = m3g::parse_section_object_stream(input, m3g::LoaderLimits::default());
            let _ = micro3d::FigureData::parse(input, micro3d::LoaderLimits::default());
            let _ = micro3d::TextureData::parse(input, true, micro3d::LoaderLimits::default());
            let _ = micro3d::ActionTableData::parse(input, micro3d::LoaderLimits::default());
            for content_type in [
                "audio/mmf",
                "audio/midi",
                "audio/wav",
                "audio/amr",
                "audio/x-tone-seq",
            ] {
                exercise_audio(content_type, input);
            }
        }
    }
}

fn valid_amr_file() -> Vec<u8> {
    let mut bytes = b"#!AMR\n".to_vec();
    // All speech rates, comfort noise and NO_DATA, with both quality flags.
    for (kind, length) in [
        (0, 13),
        (1, 14),
        (2, 16),
        (3, 18),
        (4, 20),
        (5, 21),
        (6, 27),
        (7, 32),
        (8, 6),
        (15, 1),
    ] {
        for quality in [0, 4] {
            bytes.push((kind << 3) | quality);
            bytes.resize(bytes.len() + length - 1, 0);
        }
    }
    bytes
}

fn exercise_character_encoding(input: &[u8]) {
    use natives::CharacterEncoding;

    if let Ok(name) = std::str::from_utf8(input) {
        let _ = CharacterEncoding::for_name(name);
    }
    for encoding in [
        CharacterEncoding::Utf8,
        CharacterEncoding::Latin1,
        CharacterEncoding::Ascii,
        CharacterEncoding::Utf16Be,
        CharacterEncoding::Utf16Le,
        CharacterEncoding::Utf16,
    ] {
        let units = encoding.decode(input);
        assert!(units.len() <= input.len());
        let encoded = encoding.encode(&units);
        assert!(encoded.len() <= units.len() * 3 + 2);
        // ASCII cannot encode the replacement character used for invalid input.
        if encoding != CharacterEncoding::Ascii {
            assert_eq!(encoding.decode(&encoded), units);
        }
    }
}

fn exercise_image(input: &[u8]) {
    let _ = graphics::Image::from_encoded(input);
    let _ = graphics::Image::from_midp_encoded(input);
}

fn exercise_m3g_object(input: &[u8]) {
    let Some((&kind, data)) = input.split_first() else {
        return;
    };
    // Rebuild framing/checksums so mutations reach typed object validation.
    let bytes = m3g_file_for_objects(&object(kind, data));
    let limits = m3g::LoaderLimits {
        file_bytes: MAX_FUZZ_INPUT + 128,
        decompressed_bytes: MAX_FUZZ_INPUT + 128,
        object_bytes: MAX_FUZZ_INPUT,
        sections: 4,
        objects: 128,
    };
    if let Ok(file) = m3g::M3gFile::parse(&bytes, limits) {
        let mut runtime = m3g::Runtime::new(m3g::ArenaLimits {
            objects: 128,
            bytes: MAX_FUZZ_INPUT * 4,
        });
        let references = vec![None; file.objects.len()];
        let external = vec![None; file.objects.len()];
        if let Ok(instantiated) = m3g::instantiate_file(&file, &mut runtime, &references, &external)
        {
            for handle in instantiated.handles.into_iter().flatten() {
                if let Ok(m3g::ObjectKind::KeyframeSequence(sequence)) = runtime.kind(handle) {
                    for time in [-1.0, 0.0, 5.5, f32::MAX] {
                        let _ = sequence.sample(time);
                    }
                }
            }
        }
    }
}

fn exercise_jar(input: &[u8]) {
    let _ = jar::inspect_bytes(input);
    let _ = jar::read_class_entries_bytes(input);
    if let Ok(archive) = jar::ResourceArchive::from_bytes(input) {
        let _ = archive.read_with_limit("META-INF/MANIFEST.MF", 1024 * 1024);
        let _ = archive.read_with_limit("probe.bin", 4096);
    }
}

fn exercise_audio(content_type: &str, input: &[u8]) {
    let limits = mmapi::Limits {
        max_players: 1,
        max_started_players: 1,
        max_started_midi_players: 1,
        max_handle_history: 2,
        max_input_bytes: MAX_FUZZ_INPUT,
        max_total_input_bytes: MAX_FUZZ_INPUT,
        max_pcm_frames: mmapi::OUTPUT_SAMPLE_RATE as usize * 2,
        max_total_pcm_frames: mmapi::OUTPUT_SAMPLE_RATE as usize * 2,
        max_midi_events: 4_096,
        max_midi_voice_frames: mmapi::OUTPUT_SAMPLE_RATE as usize * 8,
        max_tone_events: 1_024,
        max_pending_events: 64,
        max_tick_frames: mmapi::OUTPUT_SAMPLE_RATE as usize * 2,
    };
    let mut runtime = mmapi::Runtime::new(mmapi::NullAudioSink::default(), limits);
    if let Ok(handle) = runtime.create_from_bytes(content_type, input) {
        let _ = runtime.realize(handle);
    }
    if content_type == "audio/x-tone-seq" {
        let mut runtime = mmapi::Runtime::new(mmapi::NullAudioSink::default(), limits);
        let handle = runtime
            .create_tone_player()
            .expect("one empty tone player fits limits");
        if runtime.set_tone_sequence(handle, input).is_ok() {
            let _ = runtime.realize(handle);
        }
    }
}

fn mutate(sample: &mut Vec<u8>, state: &mut u64) {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    if sample.is_empty() {
        sample.push(state.to_le_bytes()[0]);
        return;
    }
    let sample_len = u64::try_from(sample.len()).expect("bounded fuzz input length fits u64");
    let index = usize::try_from(*state % sample_len).expect("index is below sample length");
    sample[index] ^= state.to_le_bytes()[3] | 1;
    match (*state >> 32) % 16 {
        0 if sample.len() < MAX_FUZZ_INPUT => sample.push(state.to_le_bytes()[5]),
        1 if sample.len() > 1 => {
            sample.remove(index);
        }
        _ => {}
    }
}

fn valid_m3g_file() -> Vec<u8> {
    m3g_file_for_objects(&object(9, &valid_group_data()))
}

fn m3g_file_for_objects(objects: &[u8]) -> Vec<u8> {
    const IDENTIFIER: [u8; 12] = [
        0xab, 0x4a, 0x53, 0x52, 0x31, 0x38, 0x34, 0xbb, 0x0d, 0x0a, 0x1a, 0x0a,
    ];
    let mut header = vec![1, 0, 0];
    header.extend_from_slice(&0_u32.to_le_bytes());
    header.extend_from_slice(&0_u32.to_le_bytes());
    header.extend_from_slice(b"j2play-fuzz\0");
    let content = section(objects);
    let initial_header = section(&object(0, &header));
    let size = IDENTIFIER.len() + initial_header.len() + content.len();
    header[3..7].copy_from_slice(
        &u32::try_from(size)
            .expect("seed M3G file size fits u32")
            .to_le_bytes(),
    );
    let mut file = IDENTIFIER.to_vec();
    file.extend_from_slice(&section(&object(0, &header)));
    file.extend_from_slice(&content);
    file
}

fn valid_smaf_file() -> Vec<u8> {
    let sequence = smaf_chunk(*b"Mtsq", &[0, 0x29, 10]);
    let mut track = vec![0, 0, 2, 2, 0xaa, 0xaa];
    track.extend(sequence);
    let mut root = smaf_chunk(*b"MTR\0", &track);
    root.extend_from_slice(&[0, 0]);

    let mut file = b"MMMD".to_vec();
    file.extend_from_slice(
        &u32::try_from(root.len())
            .expect("seed SMAF size fits u32")
            .to_be_bytes(),
    );
    file.extend(root);
    file
}

fn valid_midi_file() -> Vec<u8> {
    let track = [0_u8, 0x90, 69, 100, 96, 0x80, 69, 0, 0, 0xff, 0x2f, 0];
    let mut file = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk".to_vec();
    file.extend_from_slice(
        &u32::try_from(track.len())
            .expect("seed MIDI track size fits u32")
            .to_be_bytes(),
    );
    file.extend_from_slice(&track);
    file
}

fn valid_wav_file() -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&44_u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&22_050_u32.to_le_bytes());
    bytes.extend_from_slice(&44_100_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&8_u32.to_le_bytes());
    for sample in [-1000_i16, 0, 1000, 0] {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn smaf_chunk(id: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut chunk = id.to_vec();
    chunk.extend_from_slice(
        &u32::try_from(body.len())
            .expect("seed SMAF chunk size fits u32")
            .to_be_bytes(),
    );
    chunk.extend_from_slice(body);
    chunk
}

fn valid_group_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&[0, 0, 1, 1, 255]);
    data.extend_from_slice(&u32::MAX.to_le_bytes());
    data.push(0);
    data.extend_from_slice(&0_u32.to_le_bytes());
    data
}

fn object(kind: u8, data: &[u8]) -> Vec<u8> {
    let mut result = vec![kind];
    result.extend_from_slice(
        &u32::try_from(data.len())
            .expect("seed M3G object size fits u32")
            .to_le_bytes(),
    );
    result.extend_from_slice(data);
    result
}

fn section(payload: &[u8]) -> Vec<u8> {
    let mut result = vec![0];
    let payload_len = u32::try_from(payload.len()).expect("seed M3G section size fits u32");
    result.extend_from_slice(&(13_u32 + payload_len).to_le_bytes());
    result.extend_from_slice(&payload_len.to_le_bytes());
    result.extend_from_slice(payload);
    let checksum = adler32(&result);
    result.extend_from_slice(&checksum.to_le_bytes());
    result
}

fn adler32(bytes: &[u8]) -> u32 {
    const MODULUS: u32 = 65_521;
    let mut first = 1_u32;
    let mut second = 0_u32;
    for chunk in bytes.chunks(5_552) {
        for byte in chunk {
            first += u32::from(*byte);
            second += first;
        }
        first %= MODULUS;
        second %= MODULUS;
    }
    second << 16 | first
}
