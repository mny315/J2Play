use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{Attribute, ClassFile, ExceptionHandler, Member};

#[test]
fn data_output_close_flushes_the_wrapper_before_closing_the_underlying_stream() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::data_output_close_flushes_the_wrapper_before_closing_the_underlying_stream"
    )) {
        return;
    }
    let output = "fixtures/RecordingOutput";
    let wrapper = "fixtures/FlushingDataOutput";
    let mut pool = Pool::new();
    let this_class = pool.class(output);
    let super_class = pool.class("java/io/OutputStream");
    let code_name = pool.utf8("Code");
    let trace = pool.field(output, "trace", "I");
    let field = Member {
        access_flags: 0x0009,
        name_index: pool.utf8("trace"),
        descriptor_index: pool.utf8("I"),
        attributes: Vec::new(),
    };
    let init = pool.method("java/io/OutputStream", "<init>", "()V");
    let mut constructor = vec![0x2a];
    emit_reference(&mut constructor, 0xb7, init);
    constructor.push(0xb1);
    let mut methods = Vec::new();
    for (name, descriptor, code) in [
        ("<init>", "()V", constructor),
        ("write", "(I)V", vec![0xb1]),
        ("flush", "()V", record_event(trace, 1)),
        ("close", "()V", record_event(trace, 2)),
    ] {
        methods.push(code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0001,
                name,
                descriptor,
                max_stack: 2,
                max_locals: 2,
                code,
            },
        ));
    }
    let wrapper_class = pool.class(wrapper);
    let wrapper_init = pool.method(wrapper, "<init>", "(Ljava/io/OutputStream;)V");
    let output_init = pool.method(output, "<init>", "()V");
    let close = pool.method(wrapper, "close", "()V");
    let mut probe = Vec::new();
    emit_reference(&mut probe, 0xbb, wrapper_class);
    probe.push(0x59);
    emit_reference(&mut probe, 0xbb, this_class);
    probe.push(0x59);
    emit_reference(&mut probe, 0xb7, output_init);
    emit_reference(&mut probe, 0xb7, wrapper_init);
    emit_reference(&mut probe, 0xb6, close);
    emit_reference(&mut probe, 0xb2, trace);
    probe.push(0xac);
    methods.push(code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 0x0009,
            name: "probe",
            descriptor: "()I",
            max_stack: 4,
            max_locals: 0,
            code: probe,
        },
    ));
    let output_class = fixture_class(pool, this_class, super_class, vec![field], methods);
    let wrapper_class = flushing_data_output_fixture(output, wrapper);
    assert_eq!(run_probe(&[output_class, wrapper_class], output), 312);
}

fn flushing_data_output_fixture(output: &str, wrapper: &str) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(wrapper);
    let super_class = pool.class("java/io/DataOutputStream");
    let code_name = pool.utf8("Code");
    let init = pool.method(
        "java/io/DataOutputStream",
        "<init>",
        "(Ljava/io/OutputStream;)V",
    );
    let flush = pool.method("java/io/DataOutputStream", "flush", "()V");
    let trace = pool.field(output, "trace", "I");
    let mut constructor = vec![0x2a, 0x2b];
    emit_reference(&mut constructor, 0xb7, init);
    constructor.push(0xb1);
    let mut flushing = record_event(trace, 3);
    flushing.pop();
    flushing.push(0x2a);
    emit_reference(&mut flushing, 0xb7, flush);
    flushing.push(0xb1);
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0001,
                name: "<init>",
                descriptor: "(Ljava/io/OutputStream;)V",
                max_stack: 2,
                max_locals: 2,
                code: constructor,
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0001,
                name: "flush",
                descriptor: "()V",
                max_stack: 2,
                max_locals: 1,
                code: flushing,
            },
        ),
    ];
    fixture_class(pool, this_class, super_class, Vec::new(), methods)
}

#[test]
fn write_utf_checks_the_encoded_byte_limit_before_writing() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::write_utf_checks_the_encoded_byte_limit_before_writing"
    )) {
        return;
    }
    for (unit, length, expected) in [
        ('A', 65_535, 65_538),
        ('A', 65_536, -1),
        ('\0', 32_767, 65_537),
        ('\0', 32_768, -1),
        ('\u{0800}', 21_845, 65_538),
        ('\u{0800}', 21_846, -1),
    ] {
        let fixture = utf_output_fixture(unit, length);
        assert_eq!(
            run_probe(&[fixture], "fixtures/UtfOutput"),
            expected,
            "unit={unit:?}, length={length}"
        );
    }
}

fn utf_output_fixture(unit: char, length: i32) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/UtfOutput");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let bytes_class = pool.class("java/io/ByteArrayOutputStream");
    let bytes_init = pool.method("java/io/ByteArrayOutputStream", "<init>", "()V");
    let size = pool.method("java/io/ByteArrayOutputStream", "size", "()I");
    let data_class = pool.class("java/io/DataOutputStream");
    let data_init = pool.method(
        "java/io/DataOutputStream",
        "<init>",
        "(Ljava/io/OutputStream;)V",
    );
    let write_byte = pool.method("java/io/DataOutputStream", "writeByte", "(I)V");
    let write_utf = pool.method(
        "java/io/DataOutputStream",
        "writeUTF",
        "(Ljava/lang/String;)V",
    );
    let string_class = pool.class("java/lang/String");
    let string_init = pool.method("java/lang/String", "<init>", "([C)V");
    let replace = pool.method("java/lang/String", "replace", "(CC)Ljava/lang/String;");
    let length = pool.push(classfile::Constant::Integer(length));
    let unit_index = pool.push(classfile::Constant::Integer(unit as i32));
    let catch_type = pool.class("java/io/UTFDataFormatException");
    let mut code = Vec::new();
    emit_reference(&mut code, 0xbb, bytes_class);
    code.push(0x59);
    emit_reference(&mut code, 0xb7, bytes_init);
    code.push(0x4b);
    emit_reference(&mut code, 0xbb, data_class);
    code.extend_from_slice(&[0x59, 0x2a]);
    emit_reference(&mut code, 0xb7, data_init);
    code.extend_from_slice(&[0x4c, 0x2b, 0x10, 42]);
    emit_reference(&mut code, 0xb6, write_byte);
    let start_pc = u16::try_from(code.len()).unwrap();
    code.push(0x2b);
    // Build long strings at runtime: CONSTANT_Utf8 itself has a 65535-byte limit.
    emit_reference(&mut code, 0xbb, string_class);
    code.push(0x59);
    emit_reference(&mut code, 0x13, length);
    code.extend_from_slice(&[0xbc, 5]);
    emit_reference(&mut code, 0xb7, string_init);
    if unit != '\0' {
        code.push(0x03);
        emit_reference(&mut code, 0x13, unit_index);
        emit_reference(&mut code, 0xb6, replace);
    }
    emit_reference(&mut code, 0xb6, write_utf);
    let end_pc = u16::try_from(code.len()).unwrap();
    code.push(0x2a);
    emit_reference(&mut code, 0xb6, size);
    code.push(0xac);
    let handler_pc = u16::try_from(code.len()).unwrap();
    code.extend_from_slice(&[0x57, 0x2a]);
    emit_reference(&mut code, 0xb6, size);
    code.extend_from_slice(&[0x74, 0xac]); // failure returns the negative buffer size
    let mut probe = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 0x0009,
            name: "probe",
            descriptor: "()I",
            max_stack: 4,
            max_locals: 2,
            code,
        },
    );
    let Attribute::Code(code) = &mut probe.attributes[0] else {
        unreachable!()
    };
    code.exception_table.push(ExceptionHandler {
        start_pc,
        end_pc,
        handler_pc,
        catch_type,
    });
    fixture_class(pool, this_class, super_class, Vec::new(), vec![probe])
}

fn record_event(trace: u16, event: u8) -> Vec<u8> {
    let mut code = Vec::new();
    emit_reference(&mut code, 0xb2, trace);
    code.extend_from_slice(&[0x10, 10, 0x68, 0x10, event, 0x60]);
    emit_reference(&mut code, 0xb3, trace);
    code.push(0xb1);
    code
}

fn fixture_class(
    pool: Pool,
    this_class: u16,
    super_class: u16,
    fields: Vec<Member>,
    methods: Vec<Member>,
) -> ClassFile {
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

fn run_probe(classes: &[ClassFile], entry: &str) -> i32 {
    let limits = vm::Limits {
        max_instructions: 20_000_000,
        ..vm::Limits::default()
    };
    let mut program = vm::Program::new();
    for class in runtime_bootstrap::production_bootstrap_classes()
        .iter()
        .chain(classes)
    {
        program.add_class(class, &limits).unwrap();
    }
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    let execution = program
        .execute(entry, "probe", "()I", limits, false)
        .unwrap();
    let Some(vm::Value::Int(value)) = execution.value else {
        panic!("probe must return an integer")
    };
    value
}
