use super::*;
use crate::machine::slot_count;

const I: Value = Value::Int(1);
const F: Value = Value::Float(2.0);
const R: Value = Value::Reference(None);
const J: Value = Value::Long(3);
const D: Value = Value::Double(4.0);

const FORMS: &[(u8, &[Value], &[Value])] = &[
    (0x57, &[I], &[]),
    (0x58, &[I, F], &[]),
    (0x58, &[J], &[]),
    (0x59, &[R], &[R, R]),
    (0x5a, &[I, F], &[F, I, F]),
    (0x5b, &[I, F, R], &[R, I, F, R]),
    (0x5b, &[J, R], &[R, J, R]),
    (0x5c, &[I, F], &[I, F, I, F]),
    (0x5c, &[J], &[J, J]),
    (0x5d, &[I, F, R], &[F, R, I, F, R]),
    (0x5d, &[R, J], &[J, R, J]),
    (0x5e, &[I, F, R, I], &[R, I, I, F, R, I]),
    (0x5e, &[J, I, F], &[I, F, J, I, F]),
    (0x5e, &[I, F, D], &[D, I, F, D]),
    (0x5e, &[J, D], &[D, J, D]),
    (0x5f, &[I, R], &[R, I]),
];

#[test]
fn stack_permutations_preserve_value_order_categories_and_prefix() {
    for &(opcode, input, expected) in FORMS {
        for prefix in [&[][..], &[D, R][..]] {
            let mut stack = [prefix, input].concat();
            let expected = [prefix, expected].concat();
            let slots = slot_count(&stack);
            let expected_slots = slot_count(&expected);
            assert_eq!(
                rearrange(opcode, &mut stack, slots, expected_slots),
                Ok(expected_slots),
                "opcode {opcode:02x}, input {input:?}",
            );
            assert_eq!(stack, expected);
        }
    }
}

#[test]
fn duplication_rejects_overflow_before_mutation() {
    for &(opcode, input, expected) in FORMS {
        let slots = slot_count(input);
        let expected_slots = slot_count(expected);
        if expected_slots <= slots {
            continue;
        }
        let mut stack = input.to_vec();
        assert_eq!(
            rearrange(opcode, &mut stack, slots, expected_slots - 1),
            Err(Error::Overflow),
        );
        assert_eq!(stack, input);
    }
}

#[test]
fn invalid_stack_forms_do_not_split_wide_values_or_consume_operands() {
    for &(opcode, input, expected) in &[
        (0x57, &[J][..], Error::TypeMismatch),
        (0x58, &[J, I][..], Error::TypeMismatch),
        (0x59, &[D][..], Error::TypeMismatch),
        (0x5a, &[I, J][..], Error::TypeMismatch),
        (0x5a, &[J, I][..], Error::TypeMismatch),
        (0x5b, &[J, I, F][..], Error::TypeMismatch),
        (0x5c, &[J, I][..], Error::TypeMismatch),
        (0x5d, &[J, D][..], Error::TypeMismatch),
        (0x5e, &[J, I, D][..], Error::TypeMismatch),
        (0x5f, &[J, D][..], Error::TypeMismatch),
        (0x58, &[I][..], Error::Underflow),
        (0x5a, &[J][..], Error::Underflow),
        (0x5b, &[I, F][..], Error::Underflow),
        (0x5c, &[I][..], Error::Underflow),
        (0x5d, &[I, F][..], Error::Underflow),
        (0x5e, &[I, D][..], Error::Underflow),
    ] {
        let mut stack = input.to_vec();
        assert_eq!(
            rearrange(opcode, &mut stack, slot_count(input), usize::MAX),
            Err(expected),
            "opcode {opcode:02x}, input {input:?}",
        );
        assert_eq!(stack, input);
    }
    for opcode in 0x57..=0x5f {
        assert_eq!(
            rearrange(opcode, &mut Vec::new(), 0, usize::MAX),
            Err(Error::Underflow),
        );
    }
}
