use super::*;

#[test]
fn default_byte_range_checks_bounds_and_preserves_read_errors() {
    struct Bytes;
    impl VmAccess for Bytes {
        fn read_java_byte_array(&self, reference: u64) -> Result<Vec<u8>, EmuError> {
            if reference != 1 {
                return Err(native_error("invalid-reference", "missing byte array"));
            }
            Ok(vec![10, 20, 30])
        }
    }
    assert_eq!(
        Bytes.read_java_byte_array_range(1, 1, 2).unwrap(),
        Some(vec![20, 30])
    );
    assert_eq!(
        Bytes.read_java_byte_array_range(1, 3, 0).unwrap(),
        Some(vec![])
    );
    for (offset, length) in [(-1, 1), (0, -1), (4, 0), (2, 2), (i32::MAX, i32::MAX)] {
        assert_eq!(
            Bytes.read_java_byte_array_range(1, offset, length).unwrap(),
            None
        );
    }
    assert_eq!(
        Bytes
            .read_java_byte_array_range(0, -1, 0)
            .unwrap_err()
            .code(),
        "invalid-reference"
    );
}

#[test]
fn default_char_range_checks_bounds_and_preserves_read_errors() {
    struct Chars;
    impl VmAccess for Chars {
        fn read_java_char_array(&self, reference: u64) -> Result<Vec<u16>, EmuError> {
            if reference != 1 {
                return Err(native_error("invalid-reference", "missing char array"));
            }
            Ok(vec![0, 0xd800, 0xffff])
        }
    }
    assert_eq!(
        Chars.read_java_char_array_range(1, 1, 2).unwrap(),
        Some(vec![0xd800, 0xffff])
    );
    assert_eq!(
        Chars.read_java_char_array_range(1, 3, 0).unwrap(),
        Some(vec![])
    );
    for (offset, length) in [(-1, 1), (0, -1), (4, 0), (2, 2), (i32::MAX, i32::MAX)] {
        assert_eq!(
            Chars.read_java_char_array_range(1, offset, length).unwrap(),
            None
        );
    }
    assert_eq!(
        Chars
            .read_java_char_array_range(0, -1, 0)
            .unwrap_err()
            .code(),
        "invalid-reference"
    );
}
