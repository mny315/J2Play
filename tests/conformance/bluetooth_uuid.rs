use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{Attribute, ClassFile, Constant, ExceptionHandler};

const UUID: &str = "javax/bluetooth/UUID";

#[derive(Clone, Copy, Debug)]
enum Input<'a> {
    Long(i64),
    Text(Option<&'a str>, bool),
}

fn construct(pool: &mut Pool, code: &mut Vec<u8>, input: Input<'_>) {
    construct_class(pool, code, input, UUID);
}

fn construct_class(pool: &mut Pool, code: &mut Vec<u8>, input: Input<'_>, class: &str) {
    emit_reference(code, 0xbb, pool.class(class));
    code.push(0x59);
    let descriptor = match input {
        Input::Long(value) => {
            let index = pool.push(Constant::Long(value));
            pool.entries.push(None);
            emit_reference(code, 0x14, index);
            "(J)V"
        }
        Input::Text(text, short) => {
            if let Some(text) = text {
                emit_reference(code, 0x13, pool.string(text));
            } else {
                code.push(0x01);
            }
            code.push(if short { 0x04 } else { 0x03 });
            "(Ljava/lang/String;Z)V"
        }
    };
    emit_reference(code, 0xb7, pool.method(class, "<init>", descriptor));
}

fn fixture(input: Input<'_>, expected: Result<&str, &str>) -> ClassFile {
    let mut pool = Pool::new();
    let mut code = Vec::new();
    construct(&mut pool, &mut code, input);
    let end_pc = u16::try_from(code.len()).unwrap();
    let mut handlers = Vec::new();
    match expected {
        Ok(text) => {
            emit_reference(
                &mut code,
                0xb6,
                pool.method("java/lang/Object", "toString", "()Ljava/lang/String;"),
            );
            emit_reference(&mut code, 0x13, pool.string(text));
            emit_reference(
                &mut code,
                0xb6,
                pool.method("java/lang/String", "equals", "(Ljava/lang/Object;)Z"),
            );
            code.push(0xac);
        }
        Err(class) => {
            code.extend([0x57, 0x03, 0xac, 0x57, 0x04, 0xac]);
            handlers.push(ExceptionHandler {
                start_pc: 0,
                end_pc,
                handler_pc: end_pc + 3,
                catch_type: pool.class(class),
            });
        }
    }
    probe_class(pool, code, handlers, 0)
}

fn probe_class(
    mut pool: Pool,
    code: Vec<u8>,
    handlers: Vec<ExceptionHandler>,
    max_locals: u16,
) -> ClassFile {
    let this_class = pool.class("fixtures/BluetoothUuid");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let mut run = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 0x0009,
            name: "run",
            descriptor: "()I",
            max_stack: 4,
            max_locals,
            code,
        },
    );
    let Attribute::Code(code) = &mut run.attributes[0] else {
        unreachable!();
    };
    code.exception_table = handlers;
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

fn run_fixture(fixtures: &[ClassFile]) -> vm::Execution {
    let limits = vm::Limits::default();
    let mut program = vm::Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory() {
        program.add_class(&entry.class, &limits).unwrap();
    }
    for fixture in fixtures {
        program.add_class(fixture, &limits).unwrap();
    }
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    bluetooth::register_natives(program.native_registry_mut()).unwrap();
    program
        .execute("fixtures/BluetoothUuid", "run", "()I", limits, false)
        .unwrap()
}

#[test]
fn constructors_normalize_values_and_throw_the_specified_java_exceptions() {
    const ARGUMENT: &str = "java/lang/IllegalArgumentException";
    const NUMBER: &str = "java/lang/NumberFormatException";
    const NULL: &str = "java/lang/NullPointerException";
    if crate::support::isolate(concat!(
        module_path!(),
        "::constructors_normalize_values_and_throw_the_specified_java_exceptions"
    )) {
        return;
    }
    for (input, expected) in [
        (Input::Long(0), Ok("1000800000805F9B34FB")),
        (Input::Long(0x1101), Ok("110100001000800000805F9B34FB")),
        (
            Input::Long(0xffff_ffff),
            Ok("FFFFFFFF00001000800000805F9B34FB"),
        ),
        (
            Input::Text(Some("00001101"), true),
            Ok("110100001000800000805F9B34FB"),
        ),
        (
            Input::Text(Some("0000110100001000800000805f9b34fb"), false),
            Ok("110100001000800000805F9B34FB"),
        ),
        (Input::Text(Some("0000"), false), Ok("0")),
        (Input::Text(Some("00aBcD"), false), Ok("ABCD")),
        (
            Input::Text(Some("ffffffffffffffffffffffffffffffff"), false),
            Ok("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"),
        ),
        (Input::Long(-1), Err(ARGUMENT)),
        (Input::Long(i64::MIN), Err(ARGUMENT)),
        (Input::Long(0x1_0000_0000), Err(ARGUMENT)),
        (Input::Long(i64::MAX), Err(ARGUMENT)),
        (Input::Text(None, true), Err(NULL)),
        (Input::Text(None, false), Err(NULL)),
        (Input::Text(Some(""), false), Err(ARGUMENT)),
        (Input::Text(Some("000000001"), true), Err(ARGUMENT)),
        (
            Input::Text(Some("000000000000000000000000000000001"), false),
            Err(ARGUMENT),
        ),
        (Input::Text(Some("+1"), true), Err(NUMBER)),
        (Input::Text(Some("-1"), false), Err(NUMBER)),
        (Input::Text(Some("0x10"), false), Err(NUMBER)),
        (Input::Text(Some("f-g"), false), Err(NUMBER)),
        (Input::Text(Some(" 1"), false), Err(NUMBER)),
        (Input::Text(Some("１"), false), Err(NUMBER)),
        (Input::Text(Some("\0"), false), Err(NUMBER)),
    ] {
        assert_eq!(
            run_fixture(&[fixture(input, expected)]).value,
            Some(vm::Value::Int(1)),
            "{input:?}"
        );
    }
}

fn subclass() -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/UuidChild");
    let super_class = pool.class(UUID);
    let code_name = pool.utf8("Code");
    let mut code = vec![0x2a, 0x1f];
    emit_reference(&mut code, 0xb7, pool.method(UUID, "<init>", "(J)V"));
    code.push(0xb1);
    let constructor = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 0x0001,
            name: "<init>",
            descriptor: "(J)V",
            max_stack: 3,
            max_locals: 3,
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
        methods: vec![constructor],
        attributes: Vec::new(),
    }
}

fn equality_fixture() -> ClassFile {
    let mut pool = Pool::new();
    let equals = pool.method("java/lang/Object", "equals", "(Ljava/lang/Object;)Z");
    let hash = pool.method("java/lang/Object", "hashCode", "()I");
    let text = pool.method("java/lang/Object", "toString", "()Ljava/lang/String;");
    let mut code = Vec::new();
    construct(&mut pool, &mut code, Input::Long(0x1101));
    code.push(0x4b);
    construct(
        &mut pool,
        &mut code,
        Input::Text(Some("0000110100001000800000805f9b34fb"), false),
    );
    code.push(0x4c);
    construct_class(
        &mut pool,
        &mut code,
        Input::Long(0x1101),
        "fixtures/UuidChild",
    );
    code.push(0x4d);
    construct(&mut pool, &mut code, Input::Long(0x1102));
    code.push(0x4e);
    for (left, right, expected) in [
        (0, 0, true),
        (0, 1, true),
        (1, 0, true),
        (0, 2, true),
        (2, 1, true),
        (0, 3, false),
        (3, 1, false),
    ] {
        code.extend([0x2a + left, 0x2a + right]);
        emit_reference(&mut code, 0xb6, equals);
        code.extend([0x03 + u8::from(expected), 0x9f, 0, 5, 0x03, 0xac]);
    }
    for other in [1, 2] {
        code.push(0x2a);
        emit_reference(&mut code, 0xb6, hash);
        code.push(0x2a + other);
        emit_reference(&mut code, 0xb6, hash);
        code.extend([0x9f, 0, 5, 0x03, 0xac]);
    }
    code.extend([0x2a, 0x01]); // equals(null)
    emit_reference(&mut code, 0xb6, equals);
    code.extend([0x99, 0, 5, 0x03, 0xac]);
    code.extend([0x2a, 0x2a]); // equals(the same canonical text as a String)
    emit_reference(&mut code, 0xb6, text);
    emit_reference(&mut code, 0xb6, equals);
    code.extend([0x99, 0, 5, 0x03, 0xac, 0x04, 0xac]);
    probe_class(pool, code, Vec::new(), 4)
}

#[test]
fn equality_and_hashing_use_full_values_including_subclasses() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::equality_and_hashing_use_full_values_including_subclasses"
    )) {
        return;
    }
    assert_eq!(
        run_fixture(&[subclass(), equality_fixture()]).value,
        Some(vm::Value::Int(1))
    );
}
