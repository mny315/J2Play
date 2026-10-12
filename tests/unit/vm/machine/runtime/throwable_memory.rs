use super::*;

fn capture(machine: &mut Machine<'_, '_>, handle: Handle, roots: &[Value]) -> Result<(), EmuError> {
    MachineNativeContext {
        host: machine.native_context,
        heap: &mut machine.heap,
        m3g: &mut machine.m3g,
        micro3d: &mut machine.micro3d,
        jsr239: &machine.jsr239,
        classes: &mut machine.classes,
        program: machine.program,
        execution: &machine.execution,
        scheduler: &mut machine.scheduler,
        arguments: roots,
    }
    .capture_throwable_trace(handle.to_raw())
}

#[test]
fn throwable_trace_storage_counts_toward_the_managed_heap_limit() {
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 1024,
            ..Limits::default()
        },
        false,
        &mut host,
    );
    let method = runtime_method("Fixture", "work", "()V", &[0xb1], 0, 0, vec![], true);
    machine.execution.call_stack = (0..32).map(|pc| method.active_stack_frame(pc)).collect();
    let first = machine
        .allocate_object("java/lang/Throwable", HashMap::new(), &[], &[])
        .unwrap();
    machine.heap.temporary_roots.push(first);
    let before = machine.heap.managed.bytes();
    capture(&mut machine, first, &[]).unwrap();
    assert_eq!(
        machine.heap.managed.bytes() - before,
        32 * std::mem::size_of::<JavaStackFrame>()
    );
    let second = machine
        .allocate_object("java/lang/Throwable", HashMap::new(), &[], &[])
        .unwrap();
    let before_failure = machine.heap.managed.bytes();
    assert_eq!(
        capture(&mut machine, second, &[]).unwrap_err().code(),
        MANAGED_HEAP_LIMIT_CODE
    );
    assert_eq!(machine.heap.managed.bytes(), before_failure);
    assert!(!machine.heap.throwable_traces.contains_key(&second));
    assert_eq!(machine.heap.throwable_traces[&first].len(), 32);
}

#[test]
fn replacing_a_native_trace_is_atomic_and_releases_its_previous_charge() {
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 128,
            ..Limits::default()
        },
        false,
        &mut host,
    );
    let method = runtime_method("Fixture", "work", "()V", &[0xb1], 0, 0, vec![], true);
    let handle = machine
        .allocate_object("java/lang/Throwable", HashMap::new(), &[], &[])
        .unwrap();
    let root = machine
        .allocate_array(ArrayKind::Byte, 48, &[], &[Value::Reference(Some(handle))])
        .unwrap();
    let roots = [Value::Reference(Some(root))];
    let before = machine.heap.managed.bytes();
    machine
        .execution
        .call_stack
        .push(method.active_stack_frame(1));
    capture(&mut machine, handle, &roots).unwrap();
    let old_trace = machine.heap.throwable_traces[&handle].clone();
    let with_trace = machine.heap.managed.bytes();
    assert_eq!(with_trace - before, std::mem::size_of::<JavaStackFrame>());
    machine.execution.call_stack = (0..8).map(|pc| method.active_stack_frame(pc)).collect();
    assert_eq!(
        capture(&mut machine, handle, &roots).unwrap_err().code(),
        MANAGED_HEAP_LIMIT_CODE
    );
    assert_eq!(machine.heap.throwable_traces[&handle], old_trace);
    assert_eq!(machine.heap.managed.bytes(), with_trace);
    assert!(machine.heap.managed.get(root).is_ok());
    machine.execution.call_stack.clear();
    capture(&mut machine, handle, &roots).unwrap();
    assert!(machine.heap.throwable_traces[&handle].is_empty());
    assert_eq!(machine.heap.managed.bytes(), before);
    machine.collect_heap(vec![]);
    assert_eq!(machine.heap.managed.bytes(), 0);
    assert!(machine.heap.throwable_traces.is_empty());
}

#[test]
fn vm_trace_collection_keeps_the_throwable_and_current_frame_roots() {
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 128,
            ..Limits::default()
        },
        false,
        &mut host,
    );
    let method = runtime_method("Fixture", "work", "()V", &[0xb1], 0, 0, vec![], true);
    machine.execution.call_stack = (0..4).map(|pc| method.active_stack_frame(pc)).collect();
    let throwable = machine
        .allocate_object("java/lang/Throwable", HashMap::new(), &[], &[])
        .unwrap();
    let local = machine
        .allocate_object("Fixture", HashMap::new(), &[], &[])
        .unwrap();
    let operand = machine
        .allocate_object("Fixture", HashMap::new(), &[], &[])
        .unwrap();
    let base = machine.heap.managed.bytes();
    let dead = machine
        .allocate_array(ArrayKind::Byte, 56, &[], &[])
        .unwrap();
    let locals = [Some(Value::Reference(Some(local)))];
    let stack = [Value::Reference(Some(operand))];
    machine
        .record_exception_frame(throwable, &method, 0, &locals, &stack)
        .unwrap();
    assert!(machine.heap.managed.get(dead).is_err());
    for handle in [throwable, local, operand] {
        assert!(machine.heap.managed.get(handle).is_ok());
    }
    assert_eq!(
        machine.heap.managed.bytes(),
        base + 4 * std::mem::size_of::<JavaStackFrame>()
    );
    machine
        .record_exception_frame(throwable, &method, 0, &locals, &stack)
        .unwrap();
    assert_eq!(
        machine.heap.managed.bytes(),
        base + 4 * std::mem::size_of::<JavaStackFrame>()
    );
    machine.collect_heap(vec![]);
    assert_eq!(machine.heap.managed.bytes(), 0);
    assert!(machine.heap.throwable_traces.is_empty());
}

#[test]
fn invalid_throwables_cannot_replace_other_native_payload_accounting() {
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let method = runtime_method("Fixture", "work", "()V", &[0xb1], 0, 0, vec![], true);
    let string = machine
        .allocate_dynamic_string("retained text", &[], &[])
        .unwrap();
    let array = machine.allocate_array(ArrayKind::Int, 8, &[], &[]).unwrap();
    let bytes = machine.heap.managed.bytes();
    for handle in [string, array] {
        assert_eq!(
            capture(&mut machine, handle, &[]).unwrap_err().code(),
            "type-mismatch"
        );
        assert_eq!(
            machine
                .record_exception_frame(handle, &method, 0, &[], &[])
                .unwrap_err()
                .code(),
            "type-mismatch"
        );
        assert_eq!(machine.heap.managed.bytes(), bytes);
        assert!(!machine.heap.throwable_traces.contains_key(&handle));
    }
    assert_eq!(
        machine.heap.string_values[&string],
        "retained text".encode_utf16().collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "manual release throughput measurement"]
fn throwable_capture_throughput() {
    use std::{hint::black_box, time::Instant};
    let program = Program::new();
    let method = runtime_method("Fixture", "work", "()V", &[0xb1], 0, 0, vec![], true);
    for depth in [1, 32, 128] {
        for native in [false, true] {
            let mut host = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut host);
            let handle = machine
                .allocate_object("java/lang/Throwable", HashMap::new(), &[], &[])
                .unwrap();
            machine.execution.call_stack =
                (0..depth).map(|pc| method.active_stack_frame(pc)).collect();
            let started = Instant::now();
            let mut checksum = 0;
            for _ in 0..50_000 {
                if native {
                    capture(&mut machine, black_box(handle), &[]).unwrap();
                } else {
                    machine.heap.throwable_traces.remove(&handle);
                    machine
                        .record_exception_frame(black_box(handle), &method, 0, &[], &[])
                        .unwrap();
                }
                checksum += black_box(machine.heap.throwable_traces[&handle].len());
            }
            eprintln!(
                "depth={depth} native={native}: {:?}; checksum={checksum}",
                started.elapsed()
            );
        }
    }
}
