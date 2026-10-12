use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{Attribute, ClassFile, CodeAttribute, Constant, Member};

#[test]
fn print_stream_formats_floating_point_and_character_arrays() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::print_stream_formats_floating_point_and_character_arrays"
    )) {
        return;
    }
    for (descriptor, expected) in [("F", "1.01.0\n"), ("D", "1.01.0\n"), ("[C", "Ж🙂Ж🙂\n")] {
        let text = run_formatting(descriptor, None)
            .unwrap_or_else(|error| panic!("{descriptor}: {error}"));
        assert_eq!(text, expected, "{descriptor}");
    }
}

#[test]
fn println_dispatches_to_overridden_print_and_newline_methods() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::println_dispatches_to_overridden_print_and_newline_methods"
    )) {
        return;
    }
    assert_eq!(
        run_formatting("F", Some(overriding_stream())).unwrap(),
        "ppn"
    );
}

#[test]
fn printing_a_null_character_array_throws_null_pointer() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::printing_a_null_character_array_throws_null_pointer"
    )) {
        return;
    }
    let error = run_formatting("null", None).unwrap_err();
    assert_eq!(error.code(), "uncaught-exception");
    assert!(
        error.message().contains("java/lang/NullPointerException"),
        "{error}"
    );
}

fn run_formatting(
    descriptor: &str,
    subclass: Option<ClassFile>,
) -> Result<String, diagnostics::EmuError> {
    let mut program = vm::Program::new();
    let limits = vm::Limits::default();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    let mut fixture = format_fixture(descriptor);
    if let Some(class) = subclass {
        for constant in fixture.constant_pool.iter_mut().flatten() {
            if let Constant::Utf8(name) = constant
                && name == "java/io/PrintStream"
            {
                "fixtures/OverridingStream".clone_into(name);
            }
        }
        program.add_class(&class, &limits).unwrap();
    }
    program.add_class(&fixture, &limits).unwrap();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    let mut host = ConsoleHost::default();
    let execution = program.execute_with_context(
        "fixtures/PrintStreamFormatting",
        "run",
        "()V",
        limits,
        false,
        &mut host,
    )?;
    assert_eq!(execution.thread_failure_count, 0, "{descriptor}");
    Ok(host.0)
}

fn overriding_stream() -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/OverridingStream");
    let super_class = pool.class("java/io/PrintStream");
    let code_name = pool.utf8("Code");
    let init = pool.method("java/io/PrintStream", "<init>", "()V");
    let print = pool.method("java/io/PrintStream", "print", "(Ljava/lang/String;)V");
    let mut constructor = vec![0x2a];
    emit_reference(&mut constructor, 0xb7, init);
    constructor.push(0xb1);
    let mut methods = vec![code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 1,
            name: "<init>",
            descriptor: "()V",
            max_stack: 1,
            max_locals: 1,
            code: constructor,
        },
    )];
    for (name, descriptor, text) in [("print", "(F)V", "p"), ("println", "()V", "n")] {
        let text = pool.string(text);
        let mut code = vec![0x2a];
        emit_reference(&mut code, 0x13, text);
        emit_reference(&mut code, 0xb7, print);
        code.push(0xb1);
        methods.push(code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 1,
                name,
                descriptor,
                max_stack: 2,
                max_locals: 2,
                code,
            },
        ));
    }
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

#[derive(Default)]
struct ConsoleHost(String);

impl natives::HostServices for ConsoleHost {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, diagnostics::EmuError> {
        Ok(None)
    }

    fn write_console_output(
        &mut self,
        text: &str,
        newline: bool,
    ) -> Result<(), diagnostics::EmuError> {
        assert!(self.0.len() + text.len() < 1024);
        self.0.push_str(text);
        if newline {
            self.0.push('\n');
        }
        Ok(())
    }
}

fn format_fixture(descriptor: &str) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/PrintStreamFormatting");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let stream = pool.class("java/io/PrintStream");
    let init = pool.method("java/io/PrintStream", "<init>", "()V");
    let mut code = Vec::new();
    emit_reference(&mut code, 0xbb, stream);
    code.push(0x59);
    emit_reference(&mut code, 0xb7, init);
    code.push(0x4b);
    for name in ["print", "println"] {
        code.push(0x2a);
        match descriptor {
            "null" => code.push(0x01), // aconst_null
            "F" => code.push(0x0c),    // fconst_1
            "D" => code.push(0x0f),    // dconst_1
            "[C" => {
                let text = pool.string("Ж🙂");
                let chars = pool.method("java/lang/String", "toCharArray", "()[C");
                emit_reference(&mut code, 0x13, text);
                emit_reference(&mut code, 0xb6, chars);
            }
            _ => unreachable!(),
        }
        let argument = if descriptor == "null" {
            "[C"
        } else {
            descriptor
        };
        let method = pool.method("java/io/PrintStream", name, &format!("({argument})V"));
        emit_reference(&mut code, 0xb6, method);
    }
    code.push(0xb1);
    let method = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 0x0009,
            name: "run",
            descriptor: "()V",
            max_stack: 3,
            max_locals: 1,
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
        methods: vec![method],
        attributes: Vec::new(),
    }
}

fn flush_fixture() -> ClassFile {
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: vec![
            None,
            Some(Constant::Utf8("fixtures/PrintStreamFlush".to_owned())),
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::Utf8("java/lang/Object".to_owned())),
            Some(Constant::Class { name_index: 3 }),
            Some(Constant::Utf8("run".to_owned())),
            Some(Constant::Utf8("()I".to_owned())),
            Some(Constant::Utf8("Code".to_owned())),
            Some(Constant::Utf8("java/io/PrintStream".to_owned())),
            Some(Constant::Class { name_index: 8 }),
            Some(Constant::Utf8("<init>".to_owned())),
            Some(Constant::Utf8("()V".to_owned())),
            Some(Constant::NameAndType {
                name_index: 10,
                descriptor_index: 11,
            }),
            Some(Constant::Methodref {
                class_index: 9,
                name_and_type_index: 12,
            }),
            Some(Constant::Utf8("flush".to_owned())),
            Some(Constant::NameAndType {
                name_index: 14,
                descriptor_index: 11,
            }),
            Some(Constant::Methodref {
                class_index: 9,
                name_and_type_index: 15,
            }),
        ],
        access_flags: 0x0021,
        this_class: 2,
        super_class: 4,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![Member {
            access_flags: 0x0009,
            name_index: 5,
            descriptor_index: 6,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: 7,
                max_stack: 2,
                max_locals: 0,
                code: vec![
                    0xbb, 0x00, 0x09, // new PrintStream
                    0x59, // dup
                    0xb7, 0x00, 0x0d, // invokespecial PrintStream.<init>()V
                    0xb6, 0x00, 0x10, // invokevirtual PrintStream.flush()V
                    0x10, 42,   // bipush 42
                    0xac, // ireturn
                ],
                exception_table: Vec::new(),
                attributes: Vec::new(),
            })],
        }],
        attributes: Vec::new(),
    }
}

#[test]
fn invokevirtual_resolves_print_stream_flush_through_output_stream() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "invokevirtual_resolves_print_stream_flush_through_output_stream"
    )) {
        return;
    }
    let mut program = vm::Program::new();
    let limits = vm::Limits::default();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    assert!(program.is_assignable_to("java/io/PrintStream", "java/io/OutputStream"));
    program.add_class(&flush_fixture(), &limits).unwrap();

    let execution = program
        .execute("fixtures/PrintStreamFlush", "run", "()I", limits, false)
        .unwrap();
    assert_eq!(execution.value, Some(vm::Value::Int(42)));
}
