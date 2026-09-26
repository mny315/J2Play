use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{ClassFile, Member};

const FIXTURE: &str = "fixtures/StringBufferMonitor";
const BUFFER: &str = "java/lang/StringBuffer";

#[derive(Clone, Copy, Debug)]
enum Mutation {
    AppendChar,
    AppendArray,
    Delete,
}

fn monitor_fixture(mutation: Mutation) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(FIXTURE);
    let super_class = pool.class("java/lang/Thread");
    let code_name = pool.utf8("Code");
    let thread_init = pool.method("java/lang/Thread", "<init>", "()V");
    let mut constructor = vec![0x2a]; // aload_0
    emit_reference(&mut constructor, 0xb7, thread_init);
    constructor.push(0xb1);
    let run = worker_code(&mut pool);
    let probe = probe_code(&mut pool, mutation);
    let methods = [
        MethodCode {
            flags: 0x0001,
            name: "<init>",
            descriptor: "()V",
            max_stack: 1,
            max_locals: 1,
            code: constructor,
        },
        MethodCode {
            flags: 0x0001,
            name: "run",
            descriptor: "()V",
            max_stack: 2,
            max_locals: 1,
            code: run,
        },
        MethodCode {
            flags: 0x0009,
            name: "probe",
            descriptor: "()I",
            max_stack: 5,
            max_locals: 2,
            code: probe,
        },
    ]
    .into_iter()
    .map(|method| code_method(&mut pool, code_name, method))
    .collect();
    let fields = [
        ("shared", "Ljava/lang/StringBuffer;"),
        ("completed", "I"),
        ("started", "I"),
        ("observed", "I"),
    ]
    .into_iter()
    .map(|(name, descriptor)| Member {
        access_flags: 0x0009,
        name_index: pool.utf8(name),
        descriptor_index: pool.utf8(descriptor),
        attributes: Vec::new(),
    })
    .collect();
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

fn worker_code(pool: &mut Pool) -> Vec<u8> {
    let shared = pool.field(FIXTURE, "shared", "Ljava/lang/StringBuffer;");
    let started = pool.field(FIXTURE, "started", "I");
    let observed = pool.field(FIXTURE, "observed", "I");
    let completed = pool.field(FIXTURE, "completed", "I");
    let sleep = pool.method("java/lang/Thread", "sleep", "(J)V");
    let length = pool.method(BUFFER, "length", "()I");
    // A scheduler worker retains the monitor across sleep. The main thread
    // must let it observe the original buffer and release the lock before mutation.
    let mut run = Vec::new();
    emit_reference(&mut run, 0xb2, shared);
    run.extend_from_slice(&[0xc2, 0x04]); // monitorenter; iconst_1
    emit_reference(&mut run, 0xb3, started);
    run.push(0x0a); // lconst_1
    emit_reference(&mut run, 0xb8, sleep);
    emit_reference(&mut run, 0xb2, shared);
    emit_reference(&mut run, 0xb6, length);
    emit_reference(&mut run, 0xb3, observed);
    emit_reference(&mut run, 0xb2, shared);
    run.extend_from_slice(&[0xc3, 0x04]); // monitorexit; iconst_1
    emit_reference(&mut run, 0xb3, completed);
    run.push(0xb1);

    run
}

fn probe_code(pool: &mut Pool, mutation: Mutation) -> Vec<u8> {
    let buffer_class = pool.class(BUFFER);
    let this_class = pool.class(FIXTURE);
    let shared = pool.field(FIXTURE, "shared", "Ljava/lang/StringBuffer;");
    let started = pool.field(FIXTURE, "started", "I");
    let observed = pool.field(FIXTURE, "observed", "I");
    let completed = pool.field(FIXTURE, "completed", "I");
    let buffer_init = pool.method(BUFFER, "<init>", "()V");
    let fixture_init = pool.method(FIXTURE, "<init>", "()V");
    let start = pool.method("java/lang/Thread", "start", "()V");
    let sleep = pool.method("java/lang/Thread", "sleep", "(J)V");
    let join = pool.method("java/lang/Thread", "join", "()V");
    let length = pool.method(BUFFER, "length", "()I");
    let append_char = pool.method(BUFFER, "append", "(C)Ljava/lang/StringBuffer;");
    let mut probe = Vec::new();
    emit_reference(&mut probe, 0xbb, buffer_class);
    probe.push(0x59);
    emit_reference(&mut probe, 0xb7, buffer_init);
    probe.extend_from_slice(&[0x10, b'?']); // seed one character before either thread runs
    emit_reference(&mut probe, 0xb6, append_char);
    emit_reference(&mut probe, 0xb3, shared);
    emit_reference(&mut probe, 0xbb, this_class);
    probe.push(0x59);
    emit_reference(&mut probe, 0xb7, fixture_init);
    probe.extend_from_slice(&[0x4b, 0x2a]); // astore_0; aload_0
    emit_reference(&mut probe, 0xb6, start);
    probe.push(0x0a); // lconst_1: give the worker its first scheduling turn
    emit_reference(&mut probe, 0xb8, sleep);
    emit_reference(&mut probe, 0xb2, started);
    probe.extend_from_slice(&[0x04, 0x64, 0x3c]); // istore_1: started must equal 1
    emit_reference(&mut probe, 0xb2, shared);
    let (name, descriptor, arguments, final_length): (_, _, &[u8], _) = match mutation {
        // Intrinsic, interpreted nested call, and registered native respectively.
        Mutation::AppendChar => ("append", "(C)Ljava/lang/StringBuffer;", &[0x10, b'x'], 2),
        Mutation::AppendArray => (
            "append",
            "([C)Ljava/lang/StringBuffer;",
            &[0x04, 0xbc, 0x05, 0x59, 0x03, 0x10, b'x', 0x55],
            2,
        ),
        Mutation::Delete => ("delete", "(II)Ljava/lang/StringBuffer;", &[0x03, 0x04], 0),
    };
    probe.extend_from_slice(arguments);
    emit_reference(&mut probe, 0xb6, pool.method(BUFFER, name, descriptor));
    probe.extend_from_slice(&[0x57, 0x2a]); // pop; aload_0
    emit_reference(&mut probe, 0xb6, join);
    probe.push(0x1b); // iload_1
    emit_reference(&mut probe, 0xb2, observed);
    probe.extend_from_slice(&[0x04, 0x64, 0x80]); // worker must have observed the original length
    emit_reference(&mut probe, 0xb2, shared);
    emit_reference(&mut probe, 0xb6, length);
    probe.extend_from_slice(&[0x10, final_length, 0x64, 0x80]); // (length - expected) | result
    emit_reference(&mut probe, 0xb2, completed);
    probe.extend_from_slice(&[0x04, 0x64, 0x80, 0xac]); // (completed - 1) | result

    probe
}

#[test]
fn string_buffer_mutation_waits_for_its_monitor_and_resumes_after_release() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::string_buffer_mutation_waits_for_its_monitor_and_resumes_after_release"
    )) {
        return;
    }
    for mutation in [
        Mutation::AppendChar,
        Mutation::AppendArray,
        Mutation::Delete,
    ] {
        let limits = vm::Limits::default();
        let mut program = vm::Program::new();
        for class in runtime_bootstrap::production_bootstrap_classes() {
            program.add_class(&class, &limits).unwrap();
        }
        program
            .add_class(&monitor_fixture(mutation), &limits)
            .unwrap();
        cldc::register_core_natives(program.native_registry_mut()).unwrap();
        let execution = program
            .execute(FIXTURE, "probe", "()I", limits, false)
            .unwrap();
        assert_eq!(
            execution.thread_failure_count, 0,
            "{:?}",
            execution.thread_failures
        );
        assert_eq!(
            execution.value,
            Some(vm::Value::Int(0)),
            "mutation={mutation:?}"
        );
    }
}
