use super::*;
use crate::machine::tests::interpreter::NativeContextTestState;

#[test]
fn native_arrays_preserve_all_byte_values_integer_limits_and_empty_shapes() {
    let mut state = NativeContextTestState::new(Heap::new(16_384));
    let mut host = DefaultNativeContext;
    let bytes: Vec<u8> = (0..=255).collect();
    let ints = [i32::MIN, -1, 0, 1, i32::MAX];
    let longs = [i64::MIN, -1, 0, 1, i64::MAX];
    for empty in [false, true] {
        let bytes = if empty { &[][..] } else { &bytes };
        let ints = if empty { &[][..] } else { &ints };
        let longs = if empty { &[][..] } else { &longs };
        let mut context = state.context(&mut host, vec![]);
        let handles = [
            context.allocate_java_byte_array(bytes).unwrap(),
            context.allocate_java_int_array(ints).unwrap(),
            context.allocate_java_long_array(longs).unwrap(),
        ];
        let expected = [
            (
                ArrayKind::Byte,
                bytes
                    .iter()
                    .map(|value| HeapValue::Int(i32::from(value.cast_signed())))
                    .collect::<Vec<_>>(),
            ),
            (
                ArrayKind::Int,
                ints.iter().copied().map(HeapValue::Int).collect(),
            ),
            (
                ArrayKind::Long,
                longs.iter().copied().map(HeapValue::Long).collect(),
            ),
        ];
        for (handle, (expected_kind, expected_values)) in handles.into_iter().zip(expected) {
            let Allocation::Array { kind, elements } =
                state.heap.managed.get(Handle::from_raw(handle)).unwrap()
            else {
                panic!("native allocation must return an array");
            };
            assert_eq!(*kind, expected_kind);
            assert_eq!(*elements, expected_values);
        }
    }
}

fn allocate_pattern(
    context: &mut MachineNativeContext<'_>,
    kind: &ArrayKind,
    length: usize,
) -> Result<u64, EmuError> {
    match kind {
        ArrayKind::Byte => context.allocate_java_byte_array(&vec![0xe0; length]),
        ArrayKind::Int => context.allocate_java_int_array(&vec![i32::MIN; length]),
        ArrayKind::Long => context.allocate_java_long_array(&vec![i64::MIN; length]),
        _ => unreachable!("test uses only native array constructors"),
    }
}

#[test]
fn native_array_gc_retries_keep_arguments_and_do_not_retain_previous_results() {
    for kind in [ArrayKind::Byte, ArrayKind::Int, ArrayKind::Long] {
        let mut state = NativeContextTestState::new(Heap::new(128));
        let mut host = DefaultNativeContext;
        let root = state
            .heap
            .managed
            .allocate_object("fixture/Argument", HashMap::new())
            .unwrap();
        let mut previous = None;
        for _ in 0..3 {
            let remaining = 128 - state.heap.managed.bytes();
            let garbage = state
                .heap
                .managed
                .allocate_array(ArrayKind::Byte, (remaining - 24) as i32)
                .unwrap();
            assert_eq!(state.heap.managed.bytes(), 128);
            let handle = Handle::from_raw(
                allocate_pattern(&mut state.context(&mut host, vec![root]), &kind, 8).unwrap(),
            );
            assert!(state.heap.managed.get(root).is_ok());
            assert!(state.heap.managed.get(garbage).is_err());
            if let Some(previous) = previous {
                assert!(state.heap.managed.get(previous).is_err());
            }
            assert_eq!(state.heap.managed.len(), 2);
            assert_eq!(state.heap.managed.array_length(handle).unwrap(), 8);
            assert_eq!(state.heap.managed.array_kind(handle).unwrap(), &kind);
            let expected = match kind {
                ArrayKind::Byte => HeapValue::Int(-32),
                ArrayKind::Int => HeapValue::Int(i32::MIN),
                ArrayKind::Long => HeapValue::Long(i64::MIN),
                _ => unreachable!(),
            };
            for index in 0..8 {
                assert_eq!(
                    state.heap.managed.array_get(handle, index).unwrap(),
                    expected
                );
            }
            assert_eq!(
                allocate_pattern(
                    &mut state.context(&mut host, vec![root, handle]),
                    &kind,
                    256
                )
                .unwrap_err()
                .code(),
                "managed-heap-limit"
            );
            assert!(state.heap.managed.get(root).is_ok());
            assert!(state.heap.managed.get(handle).is_ok());
            assert_eq!(state.heap.managed.len(), 2);
            assert!(state.heap.temporary_roots.is_empty());
            previous = Some(handle);
        }
    }
}
