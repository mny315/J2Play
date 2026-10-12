use super::*;

fn read_expect(code: &mut Vec<u8>, method: u16, expected: u8) {
    code.push(0x2b);
    instruction(code, 0xb6, method);
    expect_int(code, expected);
}

fn write_flush(code: &mut Vec<u8>, write: u16, flush: u16, byte: u8) {
    code.extend_from_slice(&[0x2d, 0x10, byte]);
    instruction(code, 0xb6, write);
    code.push(0x2d);
    instruction(code, 0xb6, flush);
}

#[allow(clippy::too_many_lines)]
fn visibility_fixture(separate_connection: bool, retry_read: bool) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/FileVisibilityChecks");
    let super_class = pool.class("java/lang/Object");
    let connection = pool.class(CONNECTION);
    let constructor = pool.method(CONNECTION, "<init>", "(Ljava/lang/String;I)V");
    let open = pool.method(CONNECTION, "openInputStream", "()Ljava/io/InputStream;");
    let read = pool.method("java/io/InputStream", "read", "()I");
    let available = pool.method("java/io/InputStream", "available", "()I");
    let truncate = pool.method(CONNECTION, "truncate", "(J)V");
    let write = pool.method("java/io/OutputStream", "write", "(I)V");
    let flush = pool.method("java/io/OutputStream", "flush", "()V");
    let string_index = pool.utf8("file:///owned");
    let url = pool.push(Constant::String { string_index });
    let mut code = Vec::new();
    let mut handlers = Vec::new();
    instruction(&mut code, 0xbb, connection);
    code.extend_from_slice(&[0x59, 0x12, u8::try_from(url).unwrap(), 0x06]);
    instruction(&mut code, 0xb7, constructor);
    code.extend_from_slice(&[0x4b, 0x2a]);
    instruction(&mut code, 0xb6, open);
    code.push(0x4c);
    read_expect(&mut code, read, b'a');
    if separate_connection {
        instruction(&mut code, 0xbb, connection);
        code.extend_from_slice(&[0x59, 0x12, u8::try_from(url).unwrap(), 0x06]);
        instruction(&mut code, 0xb7, constructor);
    } else {
        code.push(0x2a);
    }
    code.extend_from_slice(&[0x4d, 0x2c, 0x0a]);
    instruction(
        &mut code,
        0xb6,
        pool.method(CONNECTION, "openOutputStream", "(J)Ljava/io/OutputStream;"),
    );
    code.push(0x4e);
    write_flush(&mut code, write, flush, b'X');
    if retry_read {
        let exception = pool.class("java/io/IOException");
        rejects_io(&mut code, &mut handlers, exception, |code| {
            code.push(0x2b);
            instruction(code, 0xb6, read);
            code.push(0x57);
        });
    }
    read_expect(&mut code, read, b'X');
    code.extend_from_slice(&[0x2b, 0x10, 100]);
    instruction(
        &mut code,
        0xb6,
        pool.method("java/io/InputStream", "mark", "(I)V"),
    );
    code.extend_from_slice(&[0x2c, 0x06, 0x85]);
    instruction(&mut code, 0xb6, truncate);
    read_expect(&mut code, available, 1);
    read_expect(&mut code, read, b'c');
    read_expect(&mut code, read, 255);
    write_flush(&mut code, write, flush, b'Y');
    code.push(0x2b);
    let reset = pool.method("java/io/InputStream", "reset", "()V");
    instruction(&mut code, 0xb6, reset);
    read_expect(&mut code, read, b'Y');
    code.extend_from_slice(&[0x2c, 0x0a]);
    instruction(&mut code, 0xb6, truncate);
    read_expect(&mut code, available, 0);
    code.extend_from_slice(&[0x2b, 0x0a]);
    instruction(
        &mut code,
        0xb6,
        pool.method("java/io/InputStream", "skip", "(J)J"),
    );
    code.push(0x88);
    expect_int(&mut code, 0);
    write_flush(&mut code, write, flush, b'Z');
    read_expect(&mut code, read, 255);
    code.push(0x2b);
    instruction(&mut code, 0xb6, reset);
    read_expect(&mut code, read, 255);
    write_flush(&mut code, write, flush, b'T');
    read_expect(&mut code, read, b'T');
    code.extend_from_slice(&[0x03, 0xac]);
    owned_class(pool, this_class, super_class, code, handlers)
}

#[test]
fn open_file_input_observes_flushed_writes_and_truncation() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "open_file_input_observes_flushed_writes_and_truncation"
    )) {
        return;
    }
    for (separate_connection, initial_revision, retry_read) in [
        (false, 0, false),
        (true, 0, false),
        (true, i64::MAX.cast_unsigned() - 2, false),
        (false, u64::MAX - 2, false),
        (true, 0, true),
    ] {
        let limits = vm::Limits::default();
        let mut program = vm::Program::new();
        for class in runtime_bootstrap::production_bootstrap_classes() {
            program.add_class(&class, &limits).unwrap();
        }
        program
            .add_class(
                &visibility_fixture(separate_connection, retry_read),
                &limits,
            )
            .unwrap();
        cldc::register_core_natives(program.native_registry_mut()).unwrap();
        gcf::register_natives(program.native_registry_mut()).unwrap();
        let mut context = Context {
            url: "file:///owned".into(),
            data: b"abcdef".to_vec(),
            snapshots: vec![],
            fail_next: false,
            revision: initial_revision,
            reads: 0,
            fail_read: retry_read.then_some(2),
        };
        let execution = program
            .execute_with_context(
                "fixtures/FileVisibilityChecks",
                "run",
                "()I",
                limits,
                false,
                &mut context,
            )
            .unwrap();
        assert_eq!(
            execution.value,
            Some(vm::Value::Int(0)),
            "separate={separate_connection}"
        );
        assert_eq!(context.data, b"aZT");
        assert_eq!(
            context.reads,
            7 + usize::from(retry_read),
            "read snapshots only when contents change"
        );
    }
}
