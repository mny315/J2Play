use super::*;

#[test]
fn native_byte_ranges_preserve_string_decoding_and_bounds() {
    let mut heap = Heap::new(64 * 1024);
    let receiver = heap
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    let mut state = NativeContextTestState::new(heap);
    let mut host = DefaultNativeContext;
    let mut context = state.context(&mut host, vec![receiver]);
    let source = context
        .allocate_java_byte_array(&[b'x', 0xc3, 0xa9, b'z'])
        .unwrap();
    context.heap.temporary_roots.push(Handle::from_raw(source));
    assert_eq!(
        context.read_java_byte_array_range(source, 1, 2).unwrap(),
        Some(vec![0xc3, 0xa9])
    );
    assert_eq!(
        context.read_java_byte_array_range(source, 4, 0).unwrap(),
        Some(vec![])
    );
    for (offset, count) in [(-1, 0), (0, -1), (5, 0), (3, 2), (i32::MAX, i32::MAX)] {
        assert_eq!(
            context
                .read_java_byte_array_range(source, offset, count)
                .unwrap(),
            None
        );
    }
    assert_eq!(
        context
            .read_java_byte_array_range(receiver.to_raw(), 0, 0)
            .unwrap_err()
            .code(),
        "type-mismatch"
    );

    let mut registry = NativeRegistry::new();
    cldc::register_core_natives(&mut registry).unwrap();
    let signature =
        NativeSignature::new("java/lang/String", "initBytes", "([BIILjava/lang/String;)Z");
    for (encoding, expected) in [
        ("UTF-8", vec![0x00e9]),
        ("ISO-8859-1", vec![0x00c3, 0x00a9]),
        ("latin1", vec![0x00c3, 0x00a9]),
        ("US-ASCII", vec![0xfffd, 0xfffd]),
        ("UTF-16BE", vec![0xc3a9]),
        ("UTF-16LE", vec![0xa9c3]),
        ("UTF-16", vec![0xc3a9]),
    ] {
        let encoding = context.intern_java_string(encoding).unwrap();
        assert_eq!(
            registry
                .invoke(
                    &signature,
                    &mut context,
                    &[
                        NativeValue::Reference(Some(receiver.to_raw())),
                        NativeValue::Reference(Some(source)),
                        NativeValue::Int(1),
                        NativeValue::Int(2),
                        NativeValue::Reference(Some(encoding)),
                    ]
                )
                .unwrap(),
            Some(NativeValue::Int(1))
        );
        assert_eq!(
            context.read_java_utf16(receiver.to_raw()).unwrap(),
            expected
        );
    }
    for (offset, count) in [(-1, 1), (0, -1), (3, 2), (i32::MAX, i32::MAX)] {
        assert_eq!(
            registry
                .invoke(
                    &signature,
                    &mut context,
                    &[
                        NativeValue::Reference(Some(receiver.to_raw())),
                        NativeValue::Reference(Some(source)),
                        NativeValue::Int(offset),
                        NativeValue::Int(count),
                        NativeValue::Reference(None),
                    ]
                )
                .unwrap_err()
                .code(),
            "string-index"
        );
    }
}
