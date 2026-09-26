use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Finish {
    Close,
    Rename,
    Delete,
    ConnectionClose,
}

#[allow(clippy::too_many_lines)]
fn input_fixture(finish: Finish) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/FileInputChecks");
    let super_class = pool.class("java/lang/Object");
    let connection = pool.class(CONNECTION);
    let constructor = pool.method(CONNECTION, "<init>", "(Ljava/lang/String;I)V");
    let open = pool.method(CONNECTION, "openInputStream", "()Ljava/io/InputStream;");
    let open_data = pool.method(
        CONNECTION,
        "openDataInputStream",
        "()Ljava/io/DataInputStream;",
    );
    let read = pool.method("java/io/InputStream", "read", "()I");
    let available = pool.method("java/io/InputStream", "available", "()I");
    let close = pool.method("java/io/InputStream", "close", "()V");
    let exception = pool.class("java/io/IOException");
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
    for duplicate in [open, open_data] {
        rejects_io(&mut code, &mut handlers, exception, |code| {
            code.push(0x2a);
            instruction(code, 0xb6, duplicate);
            code.push(0x57);
        });
    }
    code.push(0x2b);
    instruction(&mut code, 0xb6, read);
    expect_int(&mut code, b'a');
    code.push(0x2b);
    instruction(&mut code, 0xb6, available);
    expect_int(&mut code, 5);
    match finish {
        Finish::Close => {
            code.push(0x2b);
            instruction(&mut code, 0xb6, close);
        }
        Finish::Rename => {
            let string_index = pool.utf8("moved");
            let name = pool.push(Constant::String { string_index });
            code.extend_from_slice(&[0x2a, 0x12, u8::try_from(name).unwrap()]);
            instruction(
                &mut code,
                0xb6,
                pool.method(CONNECTION, "rename", "(Ljava/lang/String;)V"),
            );
        }
        Finish::Delete => {
            code.push(0x2a);
            instruction(&mut code, 0xb6, pool.method(CONNECTION, "delete", "()V"));
        }
        Finish::ConnectionClose => {
            code.push(0x2a);
            instruction(&mut code, 0xb6, pool.method(CONNECTION, "close", "()V"));
            let open_output =
                pool.method(CONNECTION, "openOutputStream", "()Ljava/io/OutputStream;");
            let open_output_data = pool.method(
                CONNECTION,
                "openDataOutputStream",
                "()Ljava/io/DataOutputStream;",
            );
            for closed_open in [open, open_data, open_output, open_output_data] {
                rejects_io(&mut code, &mut handlers, exception, |code| {
                    code.push(0x2a);
                    instruction(code, 0xb6, closed_open);
                    code.push(0x57);
                });
            }
            let closed_exception =
                pool.class("javax/microedition/io/file/ConnectionClosedException");
            for (name, descriptor, arguments, pop) in [
                ("fileSize", "()J", &[][..], Some(0x58)),
                ("truncate", "(J)V", &[0x09][..], None),
                ("exists", "()Z", &[][..], Some(0x57)),
            ] {
                let call = pool.method(CONNECTION, name, descriptor);
                rejects_io(&mut code, &mut handlers, closed_exception, |code| {
                    code.push(0x2a);
                    code.extend_from_slice(arguments);
                    instruction(code, 0xb6, call);
                    code.extend(pop);
                });
            }
            // Closing the connection leaves already-open streams usable.
            code.push(0x2b);
            instruction(&mut code, 0xb6, read);
            expect_int(&mut code, b'b');
            code.push(0x2b);
            instruction(&mut code, 0xb6, close);
        }
    }
    for (name, descriptor, arguments, result) in [
        ("read", "()I", &[][..], Some(0x57)),
        (
            "read",
            "([BII)I",
            &[0x07, 0xbc, 0x08, 0x03, 0x07][..],
            Some(0x57),
        ),
        ("available", "()I", &[][..], Some(0x57)),
        ("skip", "(J)J", &[0x0a][..], Some(0x58)),
        ("reset", "()V", &[][..], None),
    ] {
        let call = pool.method("java/io/InputStream", name, descriptor);
        rejects_io(&mut code, &mut handlers, exception, |code| {
            code.push(0x2b);
            code.extend_from_slice(arguments);
            instruction(code, 0xb6, call);
            code.extend(result);
        });
    }
    code.push(0x2b);
    instruction(&mut code, 0xb6, close);
    if matches!(finish, Finish::Close | Finish::Rename) {
        code.push(0x2a);
        instruction(&mut code, 0xb6, open_data);
        code.push(0x4d);
        // Re-closing an old stream must not release its replacement's slot.
        code.push(0x2b);
        instruction(&mut code, 0xb6, close);
        rejects_io(&mut code, &mut handlers, exception, |code| {
            code.push(0x2a);
            instruction(code, 0xb6, open);
            code.push(0x57);
        });
        code.push(0x2c);
        instruction(
            &mut code,
            0xb6,
            pool.method("java/io/DataInputStream", "readUnsignedByte", "()I"),
        );
        expect_int(&mut code, b'a');
        code.push(0x2c);
        instruction(&mut code, 0xb6, close);
        code.push(0x2a);
        instruction(&mut code, 0xb6, open);
        instruction(&mut code, 0xb6, close);
    }
    code.extend_from_slice(&[0x03, 0xac]);
    owned_class(pool, this_class, super_class, code, handlers)
}

#[test]
fn file_input_lifetime_is_shared_with_wrappers_and_file_mutations() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "file_input_lifetime_is_shared_with_wrappers_and_file_mutations"
    )) {
        return;
    }
    for finish in [
        Finish::Close,
        Finish::Rename,
        Finish::Delete,
        Finish::ConnectionClose,
    ] {
        let limits = vm::Limits::default();
        let mut program = vm::Program::new();
        for class in runtime_bootstrap::production_bootstrap_classes() {
            program.add_class(&class, &limits).unwrap();
        }
        program.add_class(&input_fixture(finish), &limits).unwrap();
        cldc::register_core_natives(program.native_registry_mut()).unwrap();
        gcf::register_natives(program.native_registry_mut()).unwrap();
        let mut context = Context {
            url: "file:///owned".into(),
            data: b"abcdef".to_vec(),
            snapshots: vec![],
            fail_next: false,
            revision: 0,
            reads: 0,
            fail_read: None,
        };
        let execution = program
            .execute_with_context(
                "fixtures/FileInputChecks",
                "run",
                "()I",
                limits,
                false,
                &mut context,
            )
            .unwrap();
        assert_eq!(execution.value, Some(vm::Value::Int(0)), "{finish:?}");
    }
}
