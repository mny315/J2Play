use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{ClassFile, Constant};
use diagnostics::EmuError;

const FIXTURE: &str = "fixtures/FileListing";
const CONNECTION: &str = "javax/microedition/io/file/FileConnectionImpl";

fn fixture(filter: &str, include_hidden: bool) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(FIXTURE);
    let super_class = pool.class("java/lang/Object");
    let connection = pool.class(CONNECTION);
    let constructor = pool.method(CONNECTION, "<init>", "(Ljava/lang/String;I)V");
    let list = pool.method(
        CONNECTION,
        "list",
        "(Ljava/lang/String;Z)Ljava/util/Enumeration;",
    );
    let has_more = pool.method("java/util/Enumeration", "hasMoreElements", "()Z");
    let Some(Constant::Methodref {
        class_index,
        name_and_type_index,
    }) = pool.entries[usize::from(has_more)].take()
    else {
        unreachable!()
    };
    pool.entries[usize::from(has_more)] = Some(Constant::InterfaceMethodref {
        class_index,
        name_and_type_index,
    });
    let url = pool.string("file:///owned/");
    let filter = pool.string(filter);
    let mut code = Vec::new();
    emit_reference(&mut code, 0xbb, connection);
    code.push(0x59);
    emit_reference(&mut code, 0x13, url);
    code.push(0x04); // Connector.READ
    emit_reference(&mut code, 0xb7, constructor);
    emit_reference(&mut code, 0x13, filter);
    code.push(if include_hidden { 0x04 } else { 0x03 });
    emit_reference(&mut code, 0xb6, list);
    emit_reference(&mut code, 0xb9, has_more);
    code.extend_from_slice(&[1, 0, 0xac]);
    let code_name = pool.utf8("Code");
    let run = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 9,
            name: "run",
            descriptor: "()Z",
            max_stack: 4,
            max_locals: 0,
            code,
        },
    );
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x21,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![run],
        attributes: Vec::new(),
    }
}

struct Context<'a> {
    name: &'a str,
}

impl natives::HostServices for Context<'_> {
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
    fn gcf_file_list(&mut self, url: &str) -> Result<Vec<String>, EmuError> {
        assert_eq!(url, "file:///owned/");
        Ok(vec![self.name.to_owned()])
    }
}

fn matches(name: &str, filter: &str, include_hidden: bool) -> bool {
    let limits = vm::Limits {
        max_instructions: 100_000,
        ..vm::Limits::default()
    };
    let mut program = vm::Program::new();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    program
        .add_class(&fixture(filter, include_hidden), &limits)
        .unwrap();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    gcf::register_natives(program.native_registry_mut()).unwrap();
    let result = program
        .execute_with_context(FIXTURE, "run", "()Z", limits, false, &mut Context { name })
        .unwrap_or_else(|error| panic!("name={name:?}, filter={filter:?}: {error}"));
    match result.value {
        Some(vm::Value::Int(value @ (0 | 1))) => value != 0,
        value => panic!("unexpected listing result: {value:?}"),
    }
}

#[test]
fn file_listing_filters_match_names_and_hidden_entries() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::file_listing_filters_match_names_and_hidden_entries"
    )) {
        return;
    }
    for (name, filter, expected) in [
        ("note.txt", "note.txt", true),
        ("note.txt", "note", false),
        ("note.txt", "", false),
        ("note.txt", "*", true),
        ("note.txt", "**", true),
        ("note.txt", "*.txt", true),
        ("note.txt", "*.bin", false),
        ("note.txt", "note.*", true),
        ("note.txt", "n*t*t", true),
        ("note.txt", "n*t*e", false),
        ("note.txt", "n?te.???", true),
        ("note.txt", "note.txt?", false),
        ("note.txt", "note.txt**", true),
        ("aaaaab", "*aab", true),
        ("abcabd", "*ab*bd", true),
        ("abcabd", "*ab*be", false),
        ("каталог/", "кат*", true),
        ("данные.txt", "д*.txt", true),
        ("данные.txt", "Д*.txt", false),
        ("a😀.txt", "a??.txt", true),
        ("a😀.txt", "a?.txt", false),
    ] {
        assert_eq!(
            matches(name, filter, false),
            expected,
            "{name:?}, {filter:?}"
        );
    }
    assert!(!matches(".hidden", "*", false));
    assert!(matches(".hidden", "*", true));
}

#[test]
fn file_listing_repeated_stars_finish_within_execution_budget() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::file_listing_repeated_stars_finish_within_execution_budget"
    )) {
        return;
    }
    let name = "a".repeat(32);
    assert!(!matches(&name, "*a*a*a*a*a*a*a*a*b", false));
    assert!(matches(&name, "*a*a*a*a*a*a*a*a*", false));
    assert!(!matches(&name, &format!("{}b", "*".repeat(256)), false));
}
