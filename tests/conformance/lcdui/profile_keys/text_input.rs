use super::{ClassFile, MethodCode, Pool, code_method, emit_reference, profile_key_program};

fn editor_fixture(
    initial: &str,
    keys: &[i32],
    expected: &str,
    maximum: u8,
    constraints: i32,
    paint: bool,
) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("javax/microedition/lcdui/TextEditorFixture");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let field = pool.class("javax/microedition/lcdui/TextField");
    let constructor = pool.method(
        "javax/microedition/lcdui/TextField",
        "<init>",
        "(Ljava/lang/String;Ljava/lang/String;II)V",
    );
    let key = pool.method("javax/microedition/lcdui/TextField", "__key", "(I)V");
    let get_text = pool.method(
        "javax/microedition/lcdui/TextField",
        "getString",
        "()Ljava/lang/String;",
    );
    let get_caret = pool.method(
        "javax/microedition/lcdui/TextField",
        "getCaretPosition",
        "()I",
    );
    let equals = pool.method("java/lang/String", "equals", "(Ljava/lang/Object;)Z");
    let initial = pool.string(initial);
    let expected = pool.string(expected);
    let constraints = pool.push(classfile::Constant::Integer(constraints));
    let mut code = Vec::new();
    emit_reference(&mut code, 0xbb, field);
    code.extend_from_slice(&[0x59, 0x01]); // dup; null label
    emit_reference(&mut code, 0x13, initial);
    code.extend_from_slice(&[0x10, maximum]);
    emit_reference(&mut code, 0x13, constraints);
    emit_reference(&mut code, 0xb7, constructor);
    code.push(0x4b); // astore_0
    for value in keys {
        code.push(0x2a);
        let key_value = pool.push(classfile::Constant::Integer(*value));
        emit_reference(&mut code, 0x13, key_value);
        emit_reference(&mut code, 0xb6, key);
    }
    code.push(0x2a);
    emit_reference(&mut code, 0xb6, get_text);
    emit_reference(&mut code, 0x13, expected);
    emit_reference(&mut code, 0xb6, equals);
    code.extend_from_slice(&[0x9a, 0x00, 0x05, 0x02, 0xac]);
    if paint {
        append_paint_checksum(&mut pool, &mut code);
    } else {
        code.push(0x2a);
        emit_reference(&mut code, 0xb6, get_caret);
        code.push(0xac);
    }
    let run = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 0x0009,
            name: "run",
            descriptor: "()I",
            max_stack: 8,
            max_locals: 5,
            code,
        },
    );
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![run],
        attributes: Vec::new(),
    }
}

fn append_paint_checksum(pool: &mut Pool, code: &mut Vec<u8>) {
    let create = pool.method(
        "javax/microedition/lcdui/Image",
        "createImage",
        "(II)Ljavax/microedition/lcdui/Image;",
    );
    let graphics = pool.method(
        "javax/microedition/lcdui/Image",
        "getGraphics",
        "()Ljavax/microedition/lcdui/Graphics;",
    );
    let paint = pool.method(
        "javax/microedition/lcdui/TextField",
        "__paint",
        "(Ljavax/microedition/lcdui/Graphics;IIIZ)V",
    );
    let pixels = pool.method("javax/microedition/lcdui/Image", "getRGB", "([IIIIIII)V");
    code.extend_from_slice(&[0x10, 96, 0x10, 32]);
    emit_reference(code, 0xb8, create);
    code.extend_from_slice(&[0x4c, 0x2a, 0x2b]);
    emit_reference(code, 0xb6, graphics);
    code.extend_from_slice(&[0x03, 0x03, 0x10, 80, 0x04]);
    emit_reference(code, 0xb6, paint);
    code.extend_from_slice(&[
        0x11, 0x0c, 0x00, 0xbc, 10, 0x4d, 0x2b, 0x2c, 0x03, 0x10, 96, 0x03, 0x03, 0x10, 96, 0x10,
        32,
    ]);
    emit_reference(code, 0xb6, pixels);
    code.extend_from_slice(&[0x03, 0x3e, 0x03, 0x36, 4]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x15, 4, 0x11, 0x0c, 0x00]);
    let exit = code.len();
    code.extend_from_slice(&[
        0xa2, 0, 0, 0x1d, 0x10, 31, 0x68, 0x2c, 0x15, 4, 0x2e, 0x60, 0x3e, 0x84, 4, 1,
    ]);
    let back = code.len();
    code.extend_from_slice(&[0xa7, 0, 0]);
    let end = code.len();
    code.extend_from_slice(&[0x1d, 0xac]);
    code[exit + 1..exit + 3].copy_from_slice(&i16::try_from(end - exit).unwrap().to_be_bytes());
    code[back + 1..back + 3]
        .copy_from_slice(&(-i16::try_from(back - loop_start).unwrap()).to_be_bytes());
}

fn editor_result(fixture: &ClassFile) -> vm::Value {
    let limits = vm::Limits::default();
    let mut program = profile_key_program(&limits);
    program.add_class(fixture, &limits).unwrap();
    program
        .execute(
            "javax/microedition/lcdui/TextEditorFixture",
            "run",
            "()I",
            limits,
            false,
        )
        .unwrap()
        .value
        .unwrap()
}

#[test]
fn composition_paints_inside_the_editor_without_committing_and_passwords_stay_masked() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "composition_paints_inside_the_editor_without_committing_and_passwords_stay_masked"
    )) {
        return;
    }
    for constraints in [0, 0x10000] {
        let mut keys = vec![0x10000];
        keys.extend("я🙂".encode_utf16().map(|unit| 0x20000 | i32::from(unit)));
        keys.extend([0x30003, 0x40003]);
        let preview = editor_result(&editor_fixture("A", &keys, "A", 64, constraints, true));
        let committed = editor_result(&editor_fixture("Aя🙂", &[], "Aя🙂", 64, constraints, true));
        assert_ne!(preview, vm::Value::Int(-1));
        assert_eq!(preview, committed);
        keys.extend([0x30000, 0x40003]);
        let selected = editor_result(&editor_fixture("A", &keys, "A", 64, constraints, true));
        assert_ne!(selected, preview);
        keys.push(0x10000);
        assert_eq!(
            editor_result(&editor_fixture("A", &keys, "A", 64, constraints, true)),
            editor_result(&editor_fixture("A", &[], "A", 64, constraints, true))
        );
    }
}

#[test]
fn text_edits_use_the_caret_and_preserve_unicode_boundaries_and_constraints() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "text_edits_use_the_caret_and_preserve_unicode_boundaries_and_constraints"
    )) {
        return;
    }
    for (initial, keys, expected, caret, maximum, constraints) in [
        ("A", vec![98, 99, -3, 8, 100], "Adc", 2, 16, 0),
        ("ABC", vec![-3, 8, 100, 101], "AdC", 2, 3, 0),
        ("A🙂B", vec![-3, 8], "AB", 1, 16, 0),
        ("A🙂B", vec![-3, -3, 127], "AB", 1, 16, 0),
        ("🙂", vec![-3, 8, -4, 127], "🙂", 2, 16, 0),
        ("A", vec![-3, -3, 8, 98, -4, -4, 127], "bA", 2, 16, 0),
        ("A", vec![-3, 8, 127, 98], "A", 1, 16, 0x20000),
        ("12", vec![-3, 97, 51], "132", 2, 16, 2),
        ("Я", vec![0x044e, -3, 8], "ю", 0, 16, 0),
        ("A", vec![0x10000, 0x2044f, 0x30000, 0x40001], "A", 1, 16, 0),
        ("A", vec![0x10000, 0x20062, 0x40001, 0x10000], "A", 1, 16, 0),
        (
            "A",
            vec![0x10000, 0x20062, 0x40001, 0x10000, 98],
            "Ab",
            2,
            16,
            0,
        ),
        ("A🙂B", vec![-3, 0x50002], "AB", 1, 16, 0),
        ("A🙂B", vec![-3, -3, 0x60002], "AB", 1, 16, 0),
        ("Abc", vec![0x7fffe, 100], "Adbc", 2, 16, 0),
    ] {
        assert_eq!(
            editor_result(&editor_fixture(
                initial,
                &keys,
                expected,
                maximum,
                constraints,
                false
            )),
            vm::Value::Int(caret),
            "{initial:?}, {keys:?} => {expected:?}"
        );
    }
}
