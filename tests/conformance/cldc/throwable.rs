use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::ClassFile;
use diagnostics::EmuError;

#[test]
fn throwable_descriptions_dispatch_to_guest_overrides() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::throwable_descriptions_dispatch_to_guest_overrides"
    )) {
        return;
    }
    for (overridden, expected) in [
        ("getMessage", "fixtures.CustomThrowable: replacement"),
        ("toString", "replacement"),
    ] {
        let limits = vm::Limits::default();
        let mut program = vm::Program::new();
        for class in runtime_bootstrap::production_bootstrap_classes() {
            program.add_class(&class, &limits).unwrap();
        }
        program.add_class(&fixture(overridden), &limits).unwrap();
        cldc::register_core_natives(program.native_registry_mut()).unwrap();
        let mut host = ConsoleHost::default();
        let result = program
            .execute_with_context(
                "fixtures/CustomThrowable",
                "run",
                "()V",
                limits,
                false,
                &mut host,
            )
            .unwrap();
        assert_eq!(result.thread_failure_count, 0);
        assert_eq!(host.0.lines().next(), Some(expected), "{overridden}");
        if overridden == "toString" {
            assert!(
                host.0.contains("fixtures.CustomThrowable.run"),
                "{}",
                host.0
            );
        }
    }
}

fn fixture(overridden: &str) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/CustomThrowable");
    let super_class = pool.class("java/lang/Throwable");
    let code_name = pool.utf8("Code");
    let parent_init = pool.method("java/lang/Throwable", "<init>", "(Ljava/lang/String;)V");
    let original = pool.string("original");
    let replacement = pool.string("replacement");
    let mut init = vec![0x2a];
    emit_reference(&mut init, 0x13, original);
    emit_reference(&mut init, 0xb7, parent_init);
    init.push(0xb1);
    let mut methods = vec![code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 1,
            name: "<init>",
            descriptor: "()V",
            max_stack: 2,
            max_locals: 1,
            code: init,
        },
    )];
    let mut code = Vec::new();
    emit_reference(&mut code, 0x13, replacement);
    code.push(0xb0);
    methods.push(code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 1,
            name: overridden,
            descriptor: "()Ljava/lang/String;",
            max_stack: 1,
            max_locals: 1,
            code,
        },
    ));
    let constructor = pool.method("fixtures/CustomThrowable", "<init>", "()V");
    let mut code = Vec::new();
    if overridden == "getMessage" {
        let out = pool.field("java/lang/System", "out", "Ljava/io/PrintStream;");
        emit_reference(&mut code, 0xb2, out);
    }
    emit_reference(&mut code, 0xbb, this_class);
    code.push(0x59);
    emit_reference(&mut code, 0xb7, constructor);
    if overridden == "getMessage" {
        let format = pool.method("java/lang/Throwable", "toString", "()Ljava/lang/String;");
        let print = pool.method("java/io/PrintStream", "println", "(Ljava/lang/String;)V");
        emit_reference(&mut code, 0xb6, format);
        emit_reference(&mut code, 0xb6, print);
    } else {
        let print = pool.method("java/lang/Throwable", "printStackTrace", "()V");
        emit_reference(&mut code, 0xb6, print);
    }
    code.push(0xb1);
    methods.push(code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 9,
            name: "run",
            descriptor: "()V",
            max_stack: 3,
            max_locals: 0,
            code,
        },
    ));
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x21,
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
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
    fn write_console_error(&mut self, text: &str) -> Result<(), EmuError> {
        self.write_console_output(text, true)
    }
    fn write_console_output(&mut self, text: &str, newline: bool) -> Result<(), EmuError> {
        assert!(self.0.len() + text.len() < 4096);
        self.0.push_str(text);
        if newline {
            self.0.push('\n');
        }
        Ok(())
    }
}
