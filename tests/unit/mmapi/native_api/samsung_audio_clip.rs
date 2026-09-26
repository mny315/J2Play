use super::*;
use natives::NativeValue;

#[derive(Default)]
struct ClipContext {
    created: Vec<(String, Vec<u8>)>,
    fail_transition: Option<i32>,
    closed: bool,
}

impl natives::HostServices for ClipContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }

    fn mmapi_create_bytes(&mut self, content_type: &str, bytes: &[u8]) -> Result<u64, EmuError> {
        self.created.push((content_type.to_owned(), bytes.to_vec()));
        Ok(7)
    }

    fn mmapi_transition(&mut self, handle: u64, transition: i32, _: i64) -> Result<(), EmuError> {
        assert_eq!(handle, 7);
        if self.fail_transition == Some(transition) {
            return Err(media_error("media-malformed", "test decode failure"));
        }
        self.closed |= transition == 5;
        Ok(())
    }
}

impl natives::VmAccess for ClipContext {
    fn read_java_byte_array_range(
        &self,
        reference: u64,
        offset: i32,
        length: i32,
    ) -> Result<Option<Vec<u8>>, EmuError> {
        assert_eq!(reference, 1);
        let (Ok(offset), Ok(length)) = (usize::try_from(offset), usize::try_from(length)) else {
            return Ok(None);
        };
        Ok(offset
            .checked_add(length)
            .and_then(|end| b"skipMThdtail".get(offset..end))
            .map(<[u8]>::to_vec))
    }
}

#[test]
fn creation_reads_only_the_requested_byte_range_and_closes_failed_players() {
    let mut registry = NativeRegistry::default();
    register_natives(&mut registry).unwrap();
    let signature = NativeSignature::new("com/samsung/util/AudioClip", "createBytes0", "(I[BII)J");
    let args = |reference, offset, length| {
        [
            NativeValue::Int(3),
            NativeValue::Reference(reference),
            NativeValue::Int(offset),
            NativeValue::Int(length),
        ]
    };
    for (offset, length, expected) in [(4, 4, b"MThd".as_slice()), (12, 0, b"")] {
        let mut context = ClipContext::default();
        assert_eq!(
            registry
                .invoke(&signature, &mut context, &args(Some(1), offset, length))
                .unwrap(),
            Some(NativeValue::Long(7))
        );
        assert_eq!(context.created, [("audio/midi".into(), expected.to_vec())]);
        assert!(!context.closed);
    }
    for (reference, offset, length, code) in [
        (Some(1), -1, 1, "array-index-out-of-bounds-exception"),
        (Some(1), 0, -1, "array-index-out-of-bounds-exception"),
        (Some(1), 11, 2, "array-index-out-of-bounds-exception"),
        (
            Some(1),
            i32::MAX,
            i32::MAX,
            "array-index-out-of-bounds-exception",
        ),
        (None, -1, -1, "null-pointer-exception"),
    ] {
        let mut context = ClipContext::default();
        assert_eq!(
            registry
                .invoke(&signature, &mut context, &args(reference, offset, length))
                .unwrap_err()
                .code(),
            code
        );
        assert!(context.created.is_empty());
    }
    for fail_transition in [0, 1] {
        let mut context = ClipContext {
            fail_transition: Some(fail_transition),
            ..ClipContext::default()
        };
        assert_eq!(
            registry
                .invoke(&signature, &mut context, &args(Some(1), 4, 4))
                .unwrap_err()
                .code(),
            "media-malformed"
        );
        assert_eq!(context.created.len(), 1);
        assert!(context.closed);
    }
}

#[test]
fn samsung_audio_clip_types_and_controls_map_to_mmapi() {
    let midi = b"MThd\0\0\0\x06\0\0\0\x01\0\x60";
    assert_eq!(samsung_content_type(3, midi, None).unwrap(), "audio/midi");
    assert_eq!(
        samsung_content_type(1, b"MMMD\0\0\0\x02\0\0", None).unwrap(),
        "audio/mmf"
    );
    assert_eq!(
        samsung_content_type(1, b"ID3\x04\0", None).unwrap(),
        "audio/mpeg"
    );
    assert_eq!(
        samsung_content_type(3, b"encoded", Some("/sound.mid")).unwrap(),
        "audio/midi"
    );
    assert_eq!(
        samsung_content_type(1, b"encoded", None).unwrap(),
        "audio/mmf"
    );
    assert_eq!(
        samsung_content_type(2, b"encoded", None).unwrap(),
        "audio/mpeg"
    );
    assert_eq!(
        samsung_content_type(3, b"encoded", None).unwrap(),
        "audio/midi"
    );

    assert_eq!(samsung_loop_count(0).unwrap(), 1);
    assert_eq!(samsung_loop_count(4).unwrap(), 4);
    assert_eq!(samsung_loop_count(255).unwrap(), 255);
    assert_eq!(
        samsung_loop_count(-1).unwrap_err().code(),
        "illegal-argument"
    );
    assert_eq!(
        samsung_loop_count(256).unwrap_err().code(),
        "illegal-argument"
    );
    assert_eq!(
        samsung_content_type(0, b"MMMD\0\0\0\x02\0\0", None)
            .unwrap_err()
            .code(),
        "illegal-argument"
    );
    assert_eq!(samsung_volume(0).unwrap(), 0);
    assert_eq!(samsung_volume(5).unwrap(), 100);
    assert_eq!(samsung_volume(6).unwrap_err().code(), "illegal-argument");
}
