use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{ClassFile, Member};
use std::cell::Cell;

const CLASS: &str = "fixtures/MonitorProgress";

fn jump(code: &mut Vec<u8>, opcode: u8) -> usize {
    let offset = code.len();
    code.extend([opcode, 0, 0]);
    offset
}

fn target(code: &mut [u8], branch: usize, destination: usize) {
    let distance =
        i16::try_from(i32::try_from(destination).unwrap() - i32::try_from(branch).unwrap())
            .unwrap();
    code[branch + 1..branch + 3].copy_from_slice(&distance.to_be_bytes());
}

fn worker(pool: &mut Pool) -> Vec<u8> {
    let monitor = pool.field(CLASS, "monitor", "Ljava/lang/Object;");
    let turn = pool.field(CLASS, "turn", "I");
    let observed = pool.field(CLASS, "observed", "I");
    let mut code = vec![0x2a];
    emit_reference(&mut code, 0xb4, pool.field(CLASS, "role", "I"));
    let waiter = jump(&mut code, 0x9a);
    code.extend([0x03, 0x3c]); // int turn = 0
    let next = code.len();
    emit_reference(&mut code, 0xb2, monitor);
    code.extend([0xc2, 0x1b]);
    emit_reference(&mut code, 0xb3, turn);
    // Keep the monitor across a sleep, then release and immediately reacquire
    // it on the next iteration. A queued consumer must get a scheduling turn.
    code.push(0x0a);
    emit_reference(
        &mut code,
        0xb8,
        pool.method("java/lang/Thread", "sleep", "(J)V"),
    );
    emit_reference(&mut code, 0xb2, monitor);
    code.extend([0xc3, 0x84, 0x01, 0x01, 0x1b, 0x10, 32]);
    let again = jump(&mut code, 0xa1);
    target(&mut code, again, next);
    code.push(0xb1);
    let consumer = code.len();
    target(&mut code, waiter, consumer);
    emit_reference(&mut code, 0xb2, monitor);
    code.push(0xc2);
    emit_reference(&mut code, 0xb2, turn);
    emit_reference(&mut code, 0xb3, observed);
    emit_reference(&mut code, 0xb2, monitor);
    code.extend([0xc3, 0xb1]);
    code
}

fn probe(pool: &mut Pool) -> Vec<u8> {
    let mut code = Vec::new();
    emit_reference(&mut code, 0xbb, pool.class("java/lang/Object"));
    code.push(0x59);
    emit_reference(
        &mut code,
        0xb7,
        pool.method("java/lang/Object", "<init>", "()V"),
    );
    emit_reference(
        &mut code,
        0xb3,
        pool.field(CLASS, "monitor", "Ljava/lang/Object;"),
    );
    for (role, store) in [(0x03, 0x4b), (0x04, 0x4c)] {
        emit_reference(&mut code, 0xbb, pool.class(CLASS));
        code.extend([0x59, role]);
        emit_reference(&mut code, 0xb7, pool.method(CLASS, "<init>", "(I)V"));
        code.push(store);
    }
    code.push(0x2a);
    emit_reference(
        &mut code,
        0xb6,
        pool.method("java/lang/Thread", "start", "()V"),
    );
    code.push(0x0a);
    emit_reference(
        &mut code,
        0xb8,
        pool.method("java/lang/Thread", "sleep", "(J)V"),
    );
    code.push(0x2b);
    emit_reference(
        &mut code,
        0xb6,
        pool.method("java/lang/Thread", "start", "()V"),
    );
    for thread in [0x2b, 0x2a] {
        code.push(thread);
        emit_reference(
            &mut code,
            0xb6,
            pool.method("java/lang/Thread", "join", "()V"),
        );
    }
    emit_reference(&mut code, 0xb2, pool.field(CLASS, "observed", "I"));
    code.push(0xac);
    code
}

fn fixture() -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(CLASS);
    let super_class = pool.class("java/lang/Thread");
    let code_name = pool.utf8("Code");
    let mut init = vec![0x2a];
    emit_reference(
        &mut init,
        0xb7,
        pool.method("java/lang/Thread", "<init>", "()V"),
    );
    init.extend([0x2a, 0x1b]);
    emit_reference(&mut init, 0xb5, pool.field(CLASS, "role", "I"));
    init.push(0xb1);
    let run = worker(&mut pool);
    let probe = probe(&mut pool);
    let methods = [
        MethodCode {
            flags: 1,
            name: "<init>",
            descriptor: "(I)V",
            max_stack: 2,
            max_locals: 2,
            code: init,
        },
        MethodCode {
            flags: 1,
            name: "run",
            descriptor: "()V",
            max_stack: 2,
            max_locals: 2,
            code: run,
        },
        MethodCode {
            flags: 9,
            name: "probe",
            descriptor: "()I",
            max_stack: 3,
            max_locals: 2,
            code: probe,
        },
    ]
    .into_iter()
    .map(|method| code_method(&mut pool, code_name, method))
    .collect();
    let fields = [
        (2, "role", "I"),
        (9, "monitor", "Ljava/lang/Object;"),
        (9, "turn", "I"),
        (9, "observed", "I"),
    ]
    .into_iter()
    .map(|(access_flags, name, descriptor)| Member {
        access_flags,
        name_index: pool.utf8(name),
        descriptor_index: pool.utf8(descriptor),
        attributes: Vec::new(),
    })
    .collect();
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x21,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

#[derive(Default)]
struct Clock(Cell<i64>);

impl natives::HostServices for Clock {
    fn monotonic_millis(&self) -> i64 {
        self.0.get()
    }
    fn wall_clock_millis(&self) -> i64 {
        self.0.get()
    }
    fn realtime_pacing(&self) -> bool {
        true
    }
    fn pace_millis(&mut self, millis: u64) -> Result<(), diagnostics::EmuError> {
        self.0.set(self.0.get() + i64::try_from(millis).unwrap());
        Ok(())
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, diagnostics::EmuError> {
        Ok(None)
    }
}

#[test]
fn sleeping_monitor_owner_does_not_starve_a_waiting_worker() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::sleeping_monitor_owner_does_not_starve_a_waiting_worker"
    )) {
        return;
    }
    let limits = vm::Limits {
        max_instructions: 100_000,
        ..Default::default()
    };
    let mut program = vm::Program::new();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    program.add_class(&fixture(), &limits).unwrap();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    let execution = program
        .execute_with_context(CLASS, "probe", "()I", limits, false, &mut Clock::default())
        .unwrap();
    assert_eq!(
        execution.thread_failure_count, 0,
        "{:?}",
        execution.thread_failures
    );
    assert!(
        matches!(execution.value, Some(vm::Value::Int(0..=1))),
        "consumer only acquired the monitor at {:?}",
        execution.value
    );
}
