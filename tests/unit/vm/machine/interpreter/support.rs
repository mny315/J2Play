use super::*;

pub(crate) fn method(code: &[u8], max_stack: usize, max_locals: usize) -> Method {
    let mut method = runtime_method(
        "T",
        "main",
        "()I",
        code,
        max_stack,
        max_locals,
        vec![None],
        true,
    );
    method.readonly_leaf =
        crate::machine::interpreter_batch::is_readonly_leaf(&method.instructions);
    method
}

pub(crate) fn run(
    code: &[u8],
    max_stack: usize,
    max_locals: usize,
) -> Result<Option<Value>, EmuError> {
    let m = method(code, max_stack, max_locals);
    let mut p = Program::new();
    p.methods.insert(m.key.clone(), m);
    p.execute("T", "main", "()I", Limits::default(), false)
        .map(|e| e.value)
}

pub(crate) fn runtime_method(
    class: &str,
    name: &str,
    descriptor: &str,
    code: &[u8],
    max_stack: usize,
    max_locals: usize,
    constants: Vec<Option<Constant>>,
    is_static: bool,
) -> Method {
    let key = MethodKey {
        class: class.into(),
        name: name.into(),
        descriptor: descriptor.into(),
    };
    let instructions = decode(code).unwrap();
    let instruction_index = build_instruction_index(code.len(), &instructions);
    let runtime_instructions =
        build_runtime_instructions(&instructions, &instruction_index).unwrap();
    Method {
        stack_key: Arc::new(key.clone()),
        stack_key_id: None,
        key,
        constant_pool_id: None,
        code_fingerprint: fnv1a64(code),
        max_stack,
        max_locals,
        code: Arc::new(code.into()),
        instructions: Arc::new(instructions),
        runtime_instructions: Arc::new(runtime_instructions),
        instruction_index: Arc::new(instruction_index),
        constants: Arc::new(constants),
        descriptor: parse_method_descriptor(descriptor).unwrap(),
        is_static,
        is_synchronized: false,
        exception_table: Arc::new(Vec::new()),
        is_native: false,
        compiled_integer: None,
        readonly_leaf: false,
        readonly_call_tree: false,
        compatibility_candidate: None,
    }
}

pub(crate) fn thread_class() -> Class {
    Class {
        super_name: None,
        interfaces: vec!["java/lang/Runnable".to_owned()],
        fields: vec![
            Field {
                key: "java/lang/Thread.target:Ljava/lang/Runnable;".into(),
                declaring_class: "java/lang/Thread".to_owned(),
                kind: ValueKind::Reference,
                is_static: false,
                field_token: FieldToken::new(),
                instance_slot: None,
                initial: Value::Reference(None),
                constant_string: None,
            },
            Field {
                key: "java/lang/Thread.name:Ljava/lang/String;".into(),
                declaring_class: "java/lang/Thread".to_owned(),
                kind: ValueKind::Reference,
                is_static: false,
                field_token: FieldToken::new(),
                instance_slot: None,
                initial: Value::Reference(None),
                constant_string: None,
            },
            Field {
                key: "java/lang/Thread.priority:I".into(),
                declaring_class: "java/lang/Thread".to_owned(),
                kind: ValueKind::Int,
                is_static: false,
                field_token: FieldToken::new(),
                instance_slot: None,
                initial: Value::Int(0),
                constant_string: None,
            },
        ],
        is_public: true,
        is_abstract: false,
        is_interface: false,
        no_arg_constructor: NoArgConstructor::Public,
    }
}

pub(crate) fn machine_thread_name(machine: &Machine<'_, '_>, thread: Handle) -> String {
    let HeapValue::Reference(Some(name)) = machine
        .heap
        .managed
        .field(thread, "java/lang/Thread.name:Ljava/lang/String;")
        .unwrap()
    else {
        panic!("thread name must be non-null");
    };
    String::from_utf16_lossy(machine.heap.string_values.get(&name).unwrap())
}
