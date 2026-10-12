use super::*;

#[test]
pub(crate) fn bytecode_character_scan_preserves_newline_mapping() {
    let mut constants = vec![None];
    let font_class = push_class(&mut constants, "BitmapFont");
    let string_class = push_class(&mut constants, "java/lang/String");
    let table_name = push_name_and_type(&mut constants, "characters", "Ljava/lang/String;");
    let table = push_constant(
        &mut constants,
        Constant::Fieldref {
            class_index: font_class,
            name_and_type_index: table_name,
        },
    );
    let length_name = push_name_and_type(&mut constants, "length", "()I");
    let length = push_constant(
        &mut constants,
        Constant::Methodref {
            class_index: string_class,
            name_and_type_index: length_name,
        },
    );
    let char_at_name = push_name_and_type(&mut constants, "charAt", "(I)C");
    let char_at = push_constant(
        &mut constants,
        Constant::Methodref {
            class_index: string_class,
            name_and_type_index: char_at_name,
        },
    );

    let mut code = vec![0x03, 0x3c, 0x03, 0x3d, 0x1c];
    reference(&mut code, 0xb2, table);
    reference(&mut code, 0xb6, length);
    code.extend_from_slice(&[0xa2, 0x00, 0x28]);
    reference(&mut code, 0xb2, table);
    code.push(0x1c);
    reference(&mut code, 0xb6, char_at);
    code.extend_from_slice(&[
        0x10, 0x0a, 0xa0, 0x00, 0x09, 0x84, 0x01, 0x01, 0xa7, 0x00, 0x10,
    ]);
    reference(&mut code, 0xb2, table);
    code.push(0x1c);
    reference(&mut code, 0xb6, char_at);
    code.extend_from_slice(&[
        0x1a, 0xa0, 0x00, 0x05, 0x1b, 0xac, 0x84, 0x02, 0x01, 0xa7, 0xff, 0xd4, 0x02, 0xac,
    ]);

    let mut method = runtime_method("BitmapFont", "map", "(C)I", &code, 2, 3, constants, true);
    let program = bytecode_program(&mut [&mut method]);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    machine.initialize_class("BitmapFont", 1).unwrap();
    let table_string = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .heap
        .string_values
        .insert(table_string, "AB\nCD\nΩ".encode_utf16().collect());
    machine.classes.static_fields.insert(
        "BitmapFont.characters:Ljava/lang/String;".into(),
        Value::Reference(Some(table_string)),
    );

    for (character, expected) in [
        ('A' as i32, 0),
        ('D' as i32, 1),
        ('Ω' as i32, 2),
        ('\n' as i32, -1),
        ('Z' as i32, -1),
        (0x1_0041, -1),
    ] {
        let outcome = machine.call(&method, [Value::Int(character)], 1).unwrap();
        let CallOutcome::Return(Some(Value::Int(actual))) = outcome else {
            panic!("line-ranked character lookup did not return an int");
        };
        assert_eq!(actual, expected);
    }
    assert!(machine.execution.instructions > 0);
}
