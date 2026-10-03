use super::*;
use std::sync::Mutex;

const ENUMERATE: &str = "(Ljavax/microedition/rms/RecordFilter;Ljavax/microedition/rms/RecordComparator;Z)Ljavax/microedition/rms/RecordEnumeration;";

struct StoreState {
    records: Vec<(i32, Vec<u8>)>,
    version: i32,
    reads: usize,
}

fn program(records: Vec<(i32, Vec<u8>)>) -> (Program, Arc<Mutex<StoreState>>) {
    let state = Arc::new(Mutex::new(StoreState {
        records,
        version: 1,
        reads: 0,
    }));
    let mut program = program_with_core_natives();
    let shared = Arc::clone(&state);
    program
        .native_registry_mut()
        .register(
            NativeSignature::new(STORE, "ids0", "(J)[I"),
            move |context, args| {
                assert_eq!(args, &[NativeValue::Long(7)]);
                let ids = shared
                    .lock()
                    .unwrap()
                    .records
                    .iter()
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>();
                context
                    .allocate_java_int_array(&ids)
                    .map(|handle| Some(NativeValue::Reference(Some(handle))))
            },
        )
        .unwrap();
    let shared = Arc::clone(&state);
    program
        .native_registry_mut()
        .register(
            NativeSignature::new(STORE, "metadataInt0", "(JI)I"),
            move |_, args| {
                assert_eq!(args, &[NativeValue::Long(7), NativeValue::Int(0)]);
                Ok(Some(NativeValue::Int(shared.lock().unwrap().version)))
            },
        )
        .unwrap();
    let shared = Arc::clone(&state);
    program
        .native_registry_mut()
        .register(
            NativeSignature::new(STORE, "get0", "(JI)[B"),
            move |context, args| {
                let [NativeValue::Long(7), NativeValue::Int(id)] = args else {
                    panic!("unexpected record read");
                };
                let data = {
                    let mut state = shared.lock().unwrap();
                    state.reads += 1;
                    state
                        .records
                        .iter()
                        .find(|(key, _)| key == id)
                        .unwrap()
                        .1
                        .clone()
                };
                if data.is_empty() {
                    Ok(Some(NativeValue::Reference(None)))
                } else {
                    context
                        .allocate_java_byte_array(&data)
                        .map(|handle| Some(NativeValue::Reference(Some(handle))))
                }
            },
        )
        .unwrap();
    for (class, interface, name, descriptor, code, locals) in [
        (
            "test/OddRecord",
            "javax/microedition/rms/RecordFilter",
            "matches",
            "([B)Z",
            &[0x2b, 0x03, 0x33, 0x04, 0x7e, 0xac][..],
            2,
        ),
        (
            "test/RecordOrder",
            "javax/microedition/rms/RecordComparator",
            "compare",
            "([B[B)I",
            &[
                0x2b, 0x03, 0x33, 0x2c, 0x03, 0x33, 0xa4, 0, 5, 0x04, 0xac, 0x2b, 0x03, 0x33, 0x2c,
                0x03, 0x33, 0xa2, 0, 5, 0x02, 0xac, 0x03, 0xac,
            ][..],
            3,
        ),
    ] {
        let mut definition = test_class_definition(Some("java/lang/Object"));
        definition.interfaces.push(interface.into());
        program.classes.insert(class.into(), definition);
        let method = runtime_method(class, name, descriptor, code, 3, locals, vec![None], false);
        program.methods.insert(method.key.clone(), method);
    }
    (program, state)
}

fn store(machine: &mut Machine<'_, '_>) -> Handle {
    let store = machine.allocate_native_instance(STORE, &[]).unwrap();
    for (field, value) in [
        (
            "javax/microedition/rms/RecordStore.openCount:I",
            HeapValue::Int(1),
        ),
        (
            "javax/microedition/rms/RecordStore.handle:J",
            HeapValue::Long(7),
        ),
    ] {
        machine.heap.managed.set_field(store, field, value).unwrap();
    }
    store
}

fn invoke(
    machine: &mut Machine<'_, '_>,
    receiver: Handle,
    name: &str,
    descriptor: &str,
    extra: &[Value],
) -> Value {
    let class = machine.object_class(receiver).unwrap();
    let key = machine.resolve_virtual(&class, name, descriptor).unwrap();
    let method = machine.program.methods[&key].clone();
    let mut args = vec![Value::Reference(Some(receiver))];
    args.extend_from_slice(extra);
    let mut outcome = machine.call(&method, &args, 1).unwrap();
    let mut yields = 0;
    while let CallOutcome::Suspend(continuation) = outcome {
        assert!(yields < 20_000);
        let mut roots = machine.roots(&[], &args);
        continuation.roots(&mut roots);
        machine.collect_heap(roots);
        machine.scheduler.quantum_remaining = 1;
        outcome = machine.resume_suspended_call(continuation, 1).unwrap();
        yields += 1;
    }
    match outcome {
        CallOutcome::Return(value) => value.unwrap_or(Value::Int(0)),
        CallOutcome::Throw(exception) => {
            panic!("{name} threw {}", machine.object_class(exception).unwrap())
        }
        CallOutcome::Suspend(_) => unreachable!(),
    }
}

fn enumerate(
    machine: &mut Machine<'_, '_>,
    store: Handle,
    filter: bool,
    comparator: bool,
    update: bool,
) -> Handle {
    let filter = filter.then(|| {
        machine
            .allocate_native_instance("test/OddRecord", &[Value::Reference(Some(store))])
            .unwrap()
    });
    let comparator = comparator.then(|| {
        machine
            .allocate_native_instance(
                "test/RecordOrder",
                &[Value::Reference(Some(store)), Value::Reference(filter)],
            )
            .unwrap()
    });
    let Value::Reference(Some(enumeration)) = invoke(
        machine,
        store,
        "enumerateRecords",
        ENUMERATE,
        &[
            Value::Reference(filter),
            Value::Reference(comparator),
            Value::Int(i32::from(update)),
        ],
    ) else {
        panic!("missing enumeration");
    };
    enumeration
}

#[test]
fn rms_enumeration_traverses_large_records_without_copying_their_data() {
    let (program, state) = program(vec![(1, vec![7; LARGE_RECORD_BYTES]), (3, vec![])]);
    for tracing in [false, true] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_heap_bytes: 16_384,
                ..Limits::default()
            },
            tracing,
            &mut context,
        );
        let store = store(&mut machine);
        let enumeration = enumerate(&mut machine, store, false, false, true);
        assert_eq!(
            invoke(&mut machine, enumeration, "numRecords", "()I", &[]),
            Value::Int(2)
        );
        for id in [1, 3] {
            assert_eq!(
                invoke(&mut machine, enumeration, "nextRecordId", "()I", &[]),
                Value::Int(id)
            );
        }
        invoke(&mut machine, enumeration, "reset", "()V", &[]);
        assert_eq!(
            invoke(&mut machine, enumeration, "previousRecordId", "()I", &[]),
            Value::Int(3)
        );
        {
            let mut state = state.lock().unwrap();
            state.records.push((5, vec![9; LARGE_RECORD_BYTES]));
            state.version += 1;
        }
        assert_eq!(
            invoke(&mut machine, enumeration, "numRecords", "()I", &[]),
            Value::Int(3)
        );
        assert_eq!(state.lock().unwrap().reads, 0);
        state.lock().unwrap().records.pop();
    }
}

#[test]
fn rms_enumeration_preserves_filter_order_and_stable_sort_across_gc_and_yields() {
    let records = vec![
        (2, vec![2]),
        (4, vec![3]),
        (7, vec![3]),
        (8, vec![0]),
        (9, vec![1]),
    ];
    let (program, _) = program(records);
    for (filter, comparator, expected) in [
        (false, false, vec![2, 4, 7, 8, 9]),
        (true, false, vec![4, 7, 9]),
        (false, true, vec![8, 9, 2, 4, 7]),
        (true, true, vec![9, 4, 7]),
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let store = store(&mut machine);
        let worker = machine
            .allocate_native_instance("java/lang/Thread", &[Value::Reference(Some(store))])
            .unwrap();
        machine.scheduler.current_thread = worker.to_raw();
        machine
            .scheduler
            .thread_states
            .insert(worker, ThreadState::Running);
        machine.scheduler.quantum_remaining = 1;
        let enumeration = enumerate(&mut machine, store, filter, comparator, false);
        assert_eq!(
            invoke(&mut machine, enumeration, "numRecords", "()I", &[]),
            Value::Int(expected.len() as i32)
        );
        for id in expected {
            assert_eq!(
                invoke(&mut machine, enumeration, "nextRecordId", "()I", &[]),
                Value::Int(id)
            );
        }
        assert_eq!(
            invoke(&mut machine, enumeration, "hasNextElement", "()Z", &[]),
            Value::Int(0)
        );
    }
}

#[test]
#[ignore = "manual release throughput measurement for RMS enumeration rebuilds"]
fn rms_enumeration_rebuild_throughput() {
    use std::time::{Duration, Instant};
    for count in [1, 64, 512] {
        for bytes in [16, 4_096] {
            let (program, _) = program((1..=count).map(|id| (id, vec![7; bytes])).collect());
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(
                Limits {
                    max_instructions: u64::MAX,
                    ..Limits::default()
                },
                false,
                &mut context,
            );
            let store = store(&mut machine);
            let enumeration = enumerate(&mut machine, store, false, false, false);
            let mut elapsed = Duration::ZERO;
            for _ in 0..32 {
                let start = Instant::now();
                invoke(&mut machine, enumeration, "rebuild", "()V", &[]);
                elapsed += start.elapsed();
                assert_eq!(
                    invoke(&mut machine, enumeration, "numRecords", "()I", &[]),
                    Value::Int(count)
                );
                machine.collect_heap(vec![store, enumeration]);
            }
            eprintln!(
                "rms-enumeration count={count} bytes={bytes} elapsed_ns={}",
                elapsed.as_nanos()
            );
        }
    }
}
