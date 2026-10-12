use crate::machine::{
    CallOutcome, DefaultNativeContext, HashMap, Program, SuspendedCall, ThreadState, Value,
    tests::{runtime_method, test_class_definition},
};

#[path = "checkpoint/payloads.rs"]
mod payloads;

#[path = "checkpoint/micro3d.rs"]
mod micro3d;

#[path = "checkpoint/m3g.rs"]
mod m3g_state;

#[path = "checkpoint/cold_restart.rs"]
mod cold_restart;

#[path = "checkpoint/scheduler.rs"]
mod scheduler;

fn program() -> Program {
    let mut program = Program::new();
    program
        .classes
        .insert("Checkpoint".into(), test_class_definition(None));
    let method = runtime_method(
        "Checkpoint",
        "next",
        "(I)I",
        &[0x1a, 0x04, 0x60, 0xac],
        2,
        1,
        vec![],
        true,
    );
    program.methods.insert(method.key.clone(), method);
    program
}

#[test]
fn checkpoint_thread_limit_counts_live_threads_not_retained_terminated_objects() {
    let program = program();
    let limits = super::super::Limits {
        max_threads: 1,
        ..super::super::Limits::default()
    };
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(limits, false, &mut context);
    let instance = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    let retired: Vec<_> = (0..4)
        .map(|_| {
            let thread = machine
                .heap
                .managed
                .allocate_object("Checkpoint", HashMap::new())
                .unwrap();
            machine.scheduler.finish_thread(thread);
            thread
        })
        .collect();
    machine
        .scheduler
        .thread_states
        .insert(instance, ThreadState::Runnable);
    machine.scheduler.runnable_threads.push_back(instance);
    let bytes = machine.encode_checkpoint(instance).unwrap();
    machine.scheduler.thread_states.clear();
    machine.restore_checkpoint("Checkpoint", &bytes).unwrap();
    for thread in &retired {
        assert_eq!(
            machine.scheduler.thread_states[thread],
            ThreadState::Terminated
        );
    }
    assert_eq!(
        machine.scheduler.thread_states[&instance],
        ThreadState::Runnable
    );
    machine
        .scheduler
        .thread_states
        .insert(retired[0], ThreadState::Runnable);
    let excessive = machine.encode_checkpoint(instance).unwrap();
    machine.device.random_seed_sequence = 77;
    assert_eq!(
        machine
            .restore_checkpoint("Checkpoint", &excessive)
            .unwrap_err()
            .code(),
        "checkpoint-scheduler"
    );
    assert_eq!(machine.device.random_seed_sequence, 77);
}

#[test]
fn checkpoint_rejects_stale_terminated_thread_handles() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(super::super::Limits::default(), false, &mut context);
    let instance = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    machine
        .scheduler
        .thread_states
        .insert(heap::Handle::from_raw(u64::MAX), ThreadState::Terminated);
    let bytes = machine.encode_checkpoint(instance).unwrap();
    assert_eq!(
        machine
            .restore_checkpoint("Checkpoint", &bytes)
            .unwrap_err()
            .code(),
        "checkpoint-scheduler"
    );
}

#[test]
fn checkpoint_enforces_the_live_timer_queue_capacity_before_replacing_state() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(super::super::Limits::default(), false, &mut context);
    let instance = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    let task = crate::machine::ScheduledJavaTask {
        timer: instance,
        task: instance,
        deadline: 1_000,
        period: 0,
        fixed_rate: false,
    };
    machine.scheduler.scheduled_tasks = vec![task; 1_024];
    let valid = machine.encode_checkpoint(instance).unwrap();
    machine.scheduler.scheduled_tasks.clear();
    machine.restore_checkpoint("Checkpoint", &valid).unwrap();
    assert_eq!(machine.scheduler.scheduled_tasks.len(), 1_024);

    machine.scheduler.scheduled_tasks.push(task);
    let excessive = machine.encode_checkpoint(instance).unwrap();
    machine.scheduler.scheduled_tasks.clear();
    machine.device.random_seed_sequence = 77;
    assert_eq!(
        machine
            .restore_checkpoint("Checkpoint", &excessive)
            .unwrap_err()
            .code(),
        "checkpoint-scheduler"
    );
    assert!(machine.scheduler.scheduled_tasks.is_empty());
    assert_eq!(machine.device.random_seed_sequence, 77);
}

#[test]
fn restores_heap_pixels_and_a_parked_java_call_against_a_new_program() {
    let original = program();
    let mut context = DefaultNativeContext;
    let mut machine = original.machine(super::super::Limits::default(), false, &mut context);
    let instance = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    let array = machine
        .heap
        .managed
        .allocate_array(heap::ArrayKind::Int, 3)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(array, 1, Value::Int(123))
        .unwrap();
    machine.heap.temporary_roots.push(array);
    let method = original.methods.values().next().unwrap().clone();
    machine
        .scheduler
        .thread_states
        .insert(instance, ThreadState::Sleeping);
    machine.scheduler.sleeping_threads.insert(instance, 1500);
    machine.scheduler.thread_continuations.insert(
        instance,
        Box::new(SuspendedCall {
            method,
            locals: vec![Some(Value::Int(41))],
            stack: vec![],
            pc: 0,
            synchronized_monitor: None,
            monitor_entry: None,
            pending: None,
            native_resume: None,
            class_initialization: None,
        }),
    );
    machine.m3g.graphics.renderer.clear(Some(0xff12_3456), true);
    machine.device.random_seed_sequence = 73;
    machine.scheduler.virtual_monotonic_millis = 1000;
    let bytes = machine.encode_checkpoint(instance).unwrap();

    let reloaded = program();
    let mut context = DefaultNativeContext;
    let mut restored = reloaded.machine(super::super::Limits::default(), false, &mut context);
    assert_eq!(
        restored.restore_checkpoint("Checkpoint", &bytes).unwrap(),
        instance
    );
    assert_eq!(
        restored.heap.managed.array_get(array, 1).unwrap(),
        Value::Int(123)
    );
    assert_eq!(restored.m3g.graphics.renderer.pixels()[0], 0xff12_3456);
    assert_eq!(restored.device.random_seed_sequence, 73);
    assert_eq!(restored.scheduler.sleeping_threads[&instance], 1500);
    assert_eq!(restored.scheduler.virtual_monotonic_millis, 1000);
    let continuation = restored
        .scheduler
        .thread_continuations
        .remove(&instance)
        .unwrap();
    assert!(matches!(
        restored.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(Some(Value::Int(42)))
    ));
}

#[test]
fn rejects_a_checkpoint_for_a_different_midlet_and_truncated_state() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(super::super::Limits::default(), false, &mut context);
    let instance = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    let bytes = machine.encode_checkpoint(instance).unwrap();
    assert!(machine.restore_checkpoint("Different", &bytes).is_err());
    assert!(
        machine
            .restore_checkpoint("Checkpoint", &bytes[..bytes.len() - 1])
            .is_err()
    );
}

#[test]
fn restore_rejects_invalid_background_and_profile_exceeding_sprite_crops() {
    let program = program();
    for (sprite, width, valid) in [
        (false, -1, false),
        (false, 8, true),
        (true, -7, true),
        (true, 8, false),
        (true, i32::MIN, false),
    ] {
        let mut context = DefaultNativeContext;
        let limits = super::super::Limits {
            m3g_max_sprite_crop_dimension: 7,
            ..super::super::Limits::default()
        };
        let mut machine = program.machine(limits, false, &mut context);
        let instance = machine
            .heap
            .managed
            .allocate_object("Checkpoint", HashMap::new())
            .unwrap();
        let image = machine
            .m3g
            .runtime
            .create(
                None,
                m3g::ObjectKind::Image2D(
                    m3g::Image2DState::mutable(m3g::ImageFormat::Rgba, 2, 2).unwrap(),
                ),
            )
            .unwrap();
        let kind = if sprite {
            m3g::ObjectKind::Sprite3D(m3g::SpriteState {
                node: m3g::NodeState::default(),
                scaled: false,
                image,
                appearance: None,
                crop: [0, 0, 2, 2],
            })
        } else {
            m3g::ObjectKind::Background(m3g::BackgroundState::default())
        };
        let target = machine.m3g.runtime.create(None, kind).unwrap();
        match machine.m3g.runtime.kind_mut(target).unwrap() {
            m3g::ObjectKind::Sprite3D(state) => state.crop = [i32::MIN, i32::MAX, width, 2],
            m3g::ObjectKind::Background(state) => state.crop = [i32::MIN, i32::MAX, width, 2],
            _ => unreachable!(),
        }
        machine.device.random_seed_sequence = 11;
        let bytes = machine.encode_checkpoint(instance).unwrap();
        machine.device.random_seed_sequence = 22;
        let result = machine.restore_checkpoint("Checkpoint", &bytes);
        if valid {
            assert_eq!(result.unwrap(), instance);
            assert_eq!(machine.device.random_seed_sequence, 11);
        } else {
            assert_eq!(result.unwrap_err().code(), "invalid-crop");
            assert_eq!(machine.device.random_seed_sequence, 22);
        }
    }
}

#[test]
fn rejects_previous_bootstrap_state_before_restoring_any_vm_state() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(super::super::Limits::default(), false, &mut context);
    let instance = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    machine.device.random_seed_sequence = 73;
    let bytes = machine.encode_checkpoint(instance).unwrap();
    // The first field is the postcard-encoded format, before any VM data.
    let current_format = save_state::encode(&super::FORMAT).unwrap();
    assert!(bytes.starts_with(&current_format));
    machine.device.random_seed_sequence = 99;
    for previous_format in 1..super::FORMAT {
        let mut previous = bytes.clone();
        previous.splice(
            ..current_format.len(),
            save_state::encode(&previous_format).unwrap(),
        );
        assert_eq!(
            machine
                .restore_checkpoint("Checkpoint", &previous)
                .unwrap_err()
                .code(),
            "checkpoint-version"
        );
        assert_eq!(machine.device.random_seed_sequence, 99);
    }
}

#[derive(Default)]
struct CheckpointHost(Vec<u8>);

impl natives::HostServices for CheckpointHost {
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
    fn save_checkpoint(
        &mut self,
        bytes: &[u8],
        _: &[u8],
        _: (i64, i64),
    ) -> Result<(), diagnostics::EmuError> {
        self.0 = bytes.to_vec();
        Ok(())
    }
}

#[test]
fn checkpoint_driver_skips_construction_and_class_initialization_on_resume() {
    let mut program = program();
    for name in ["<init>", "<clinit>"] {
        let mut code = vec![0x00; 17];
        code.push(0xb1);
        let method = runtime_method(
            "Checkpoint",
            name,
            "()V",
            &code,
            0,
            1,
            vec![],
            name == "<clinit>",
        );
        program.methods.insert(method.key.clone(), method);
    }
    let limits = super::super::Limits::default();
    let mut host = CheckpointHost::default();
    let mut steps = std::collections::VecDeque::from([
        crate::DriverStep::SaveCheckpoint {
            driver_state: vec![],
        },
        crate::DriverStep::Stop,
    ]);
    program
        .execute_instance_checkpoint_driver_with_context(
            "Checkpoint",
            |_| Ok(steps.pop_front().unwrap()),
            limits.clone(),
            false,
            &mut host,
            None,
        )
        .unwrap();
    let initial: super::Checkpoint = save_state::decode(&host.0).unwrap();
    assert!(initial.instructions >= 36);
    let bytes = host.0.clone();
    let mut steps = std::collections::VecDeque::from([
        crate::DriverStep::SaveCheckpoint {
            driver_state: vec![],
        },
        crate::DriverStep::Stop,
    ]);
    program
        .execute_instance_checkpoint_driver_with_context(
            "Checkpoint",
            |_| Ok(steps.pop_front().unwrap()),
            limits,
            false,
            &mut host,
            Some(bytes),
        )
        .unwrap();
    let resumed: super::Checkpoint = save_state::decode(&host.0).unwrap();
    assert_eq!(
        resumed.instructions, initial.instructions,
        "Neither constructor nor class initializer should run twice"
    );
    assert_eq!(resumed.instance, initial.instance);
}
