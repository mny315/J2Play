use super::*;

pub(super) fn spec(code: &[u8]) -> IntegerMethodSpec {
    IntegerMethodSpec {
        code: code.to_vec(),
        static_integer_parameters: vec![],
        integer_constants: vec![],
        parameter_slots: vec![0],
        max_locals: 2,
        max_stack: 4,
    }
}

#[test]
fn rejects_uninitialized_and_inconsistent_control_flow() {
    assert!(verify(&spec(&[0x1b, 0xac])).is_none());
    assert!(verify(&spec(&[0x1a, 0x99, 0, 7, 0x04, 0xa7, 0, 3, 0xac])).is_none());
    assert!(verify(&spec(&[0xa7, 0, 1])).is_none());
    assert!(verify(&spec(&[0x1a, 0xb8, 0, 1, 0xac])).is_none());
}

#[test]
fn accepts_integer_loop_with_initialization_before_entry() {
    assert!(
        verify(&spec(&[
            0x03, 0x3c, 0x1a, 0x9e, 0, 13, 0x1b, 0x1a, 0x60, 0x3c, 0x84, 0, 0xff, 0xa7, 0xff, 0xf5,
            0x1b, 0xac
        ]))
        .is_some()
    );
}

#[test]
fn malformed_bytes_are_bounded() {
    for byte in 0..=255 {
        let _ = verify(&spec(&[byte; 32]));
    }
    assert!(verify(&spec(&vec![0; 2_049])).is_none());
}
