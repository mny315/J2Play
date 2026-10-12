use super::*;

#[test]
fn data_input_utf_follows_cldc_byte_group_decoding() {
    assert_eq!(
        decode_data_input_utf(&[b'A', 0, 0xc0, 0x80, b'Z']),
        Ok(vec![u16::from(b'A'), 0, 0, u16::from(b'Z')])
    );
    for unit in 0..=u16::MAX {
        let three = [
            0xe0 | (unit >> 12) as u8,
            0x80 | ((unit >> 6) & 0x3f) as u8,
            0x80 | (unit & 0x3f) as u8,
        ];
        assert_eq!(decode_data_input_utf(&three), Ok(vec![unit]));
        if unit < 0x800 {
            let two = [0xc0 | (unit >> 6) as u8, 0x80 | (unit & 0x3f) as u8];
            assert_eq!(decode_data_input_utf(&two), Ok(vec![unit]));
        }
    }
    for invalid in [
        &[0xc1][..],
        &[0xe0],
        &[0xe0, 0x80],
        &[0xc1, 0x7f],
        &[0xe0, 0x7f, 0x80],
        &[0xe0, 0x80, 0xff],
        &[0x80],
        &[0xf0, 0x90, 0x80, 0x80],
    ] {
        assert!(decode_data_input_utf(invalid).is_err(), "{invalid:?}");
    }
}

#[test]
fn data_input_utf_rejects_out_of_contract_length_and_byte_results() {
    assert_eq!(data_input_utf_length(0).unwrap(), 0);
    assert_eq!(data_input_utf_length(i32::from(u16::MAX)).unwrap(), 65_535);
    for value in [-1, i32::from(u16::MAX) + 1, i32::MAX] {
        assert_eq!(
            data_input_utf_length(value).unwrap_err().code(),
            "invalid-stream-read"
        );
    }

    assert_eq!(data_input_utf_byte(0).unwrap(), 0);
    assert_eq!(data_input_utf_byte(i32::from(u8::MAX)).unwrap(), u8::MAX);
    for value in [-1, i32::from(u8::MAX) + 1, i32::MAX] {
        assert_eq!(
            data_input_utf_byte(value).unwrap_err().code(),
            "invalid-stream-read"
        );
    }
}

#[test]
fn data_input_utf_native_continuation_resumes_to_a_string_result() {
    let mut read_utf = runtime_method(
        "java/io/DataInputStream",
        "readUTF",
        "(Ljava/io/DataInput;)Ljava/lang/String;",
        &[],
        0,
        1,
        Vec::new(),
        true,
    );
    read_utf.is_native = true;
    let pending_byte = runtime_method(
        "Input",
        "readUnsignedByte",
        "()I",
        &[0x10, b'A', 0xac],
        1,
        0,
        Vec::new(),
        true,
    );
    let child = Box::new(SuspendedCall {
        method: pending_byte,
        locals: Vec::new(),
        stack: Vec::new(),
        pc: 0,
        synchronized_monitor: None,
        monitor_entry: None,
        pending: None,
        native_resume: None,
        class_initialization: None,
    });
    let mut program = Program::new();
    program
        .methods
        .insert(read_utf.key.clone(), read_utf.clone());
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let input = machine
        .heap
        .managed
        .allocate_object("Input", HashMap::new())
        .unwrap();
    let CallOutcome::Suspend(continuation) = suspended_native_pending_call(
        &read_utf,
        NativeResume::DataInputReadUtf {
            input,
            length: Some(1),
            bytes: Vec::new(),
        },
        child,
    ) else {
        unreachable!()
    };

    let CallOutcome::Return(Some(Value::Reference(Some(string)))) =
        machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("readUTF continuation must return a String reference");
    };
    assert_eq!(
        machine.heap.string_values.get(&string),
        Some(&vec![u16::from(b'A')])
    );
}
