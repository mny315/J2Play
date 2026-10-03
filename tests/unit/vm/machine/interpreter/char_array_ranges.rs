use super::*;

#[test]
fn native_char_ranges_preserve_exact_units_and_string_constructor_bounds() {
    let mut heap = Heap::new(16 * 1024);
    let receiver = heap
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    let source = heap.allocate_array(ArrayKind::Char, 1024).unwrap();
    let units = [0, 0xd800, 0xdc00, 0xdc00, 0xffff];
    for (index, unit) in units.iter().enumerate() {
        heap.array_set(source, index as i32 + 512, HeapValue::Int(*unit))
            .unwrap();
    }
    let garbage = heap
        .allocate_array(
            ArrayKind::Byte,
            i32::try_from(16 * 1024 - heap.bytes() - 24).unwrap(),
        )
        .unwrap();
    let mut state = NativeContextTestState::new(heap);
    let mut host = DefaultNativeContext;
    let mut context = state.context(&mut host, vec![receiver, source]);
    let mut registry = NativeRegistry::new();
    cldc::register_core_natives(&mut registry).unwrap();
    let signature = NativeSignature::new("java/lang/String", "initChars", "([CII)V");
    let invoke = |context: &mut MachineNativeContext<'_>, offset, count| {
        registry.invoke(
            &signature,
            context,
            &[
                NativeValue::Reference(Some(receiver.to_raw())),
                NativeValue::Reference(Some(source.to_raw())),
                NativeValue::Int(offset),
                NativeValue::Int(count),
            ],
        )
    };
    assert_eq!(invoke(&mut context, 512, 5).unwrap(), None);
    assert!(context.heap.managed.get(garbage).is_err());
    assert_eq!(
        context.read_java_utf16(receiver.to_raw()).unwrap(),
        [0, 0xd800, 0xdc00, 0xdc00, 0xffff]
    );
    assert_eq!(
        context.read_java_string(receiver.to_raw()).unwrap(),
        "\0\u{10000}\u{fffd}\u{ffff}"
    );
    for (offset, count) in [(-1, 0), (0, -1), (1025, 0), (1023, 2), (i32::MAX, i32::MAX)] {
        assert_eq!(
            invoke(&mut context, offset, count).unwrap_err().code(),
            "string-index"
        );
        assert_eq!(context.read_java_utf16(receiver.to_raw()).unwrap().len(), 5);
    }
    assert_eq!(
        context
            .read_java_char_array_range(receiver.to_raw(), -1, 0)
            .unwrap_err()
            .code(),
        "type-mismatch"
    );
    assert!(context.read_java_string(source.to_raw()).is_err());
    context
        .heap
        .managed
        .array_set(source, 512, HeapValue::Int(42))
        .unwrap();
    assert_eq!(context.read_java_utf16(receiver.to_raw()).unwrap()[0], 0);
    assert_eq!(invoke(&mut context, 1024, 0).unwrap(), None);
    assert_eq!(context.read_java_string(receiver.to_raw()).unwrap(), "");
}
