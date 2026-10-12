use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{ClassFile, Member};
use natives::{NativeSignature, NativeValue};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI64, Ordering},
};

#[derive(Clone, Default)]
struct Clock {
    now: Arc<AtomicI64>,
    firings: Arc<Mutex<Vec<i64>>>,
}

impl natives::HostServices for Clock {
    fn monotonic_millis(&self) -> i64 {
        self.now.load(Ordering::Relaxed)
    }
    fn wall_clock_millis(&self) -> i64 {
        self.monotonic_millis()
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, diagnostics::EmuError> {
        Ok(None)
    }
    fn realtime_pacing(&self) -> bool {
        true
    }
    fn pace_millis(&mut self, millis: u64) -> Result<(), diagnostics::EmuError> {
        self.now
            .fetch_add(i64::try_from(millis).unwrap(), Ordering::Relaxed);
        Ok(())
    }
}

fn timer_class(app: bool) -> ClassFile {
    let name = if app {
        "fixtures/TimerApp"
    } else {
        "fixtures/TimedTask"
    };
    let parent = if app {
        "java/lang/Object"
    } else {
        "java/util/TimerTask"
    };
    let mut pool = Pool::new();
    let this_class = pool.class(name);
    let super_class = pool.class(parent);
    let code_name = pool.utf8("Code");
    let mut code = vec![0x2a];
    emit_reference(&mut code, 0xb7, pool.method(parent, "<init>", "()V"));
    if app {
        for class in ["java/util/Timer", "fixtures/TimedTask"] {
            emit_reference(&mut code, 0xbb, pool.class(class));
            code.push(0x59);
            emit_reference(&mut code, 0xb7, pool.method(class, "<init>", "()V"));
        }
        emit_reference(
            &mut code,
            0xb8,
            pool.method(name, "arm", "(Ljava/util/Timer;Ljava/util/TimerTask;)V"),
        );
    }
    code.push(0xb1);
    let constructor = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 1,
            name: "<init>",
            descriptor: "()V",
            max_stack: 3,
            max_locals: 1,
            code,
        },
    );
    let native = Member {
        access_flags: if app { 0x0109 } else { 0x0101 },
        name_index: pool.utf8(if app { "arm" } else { "run" }),
        descriptor_index: pool.utf8(if app {
            "(Ljava/util/Timer;Ljava/util/TimerTask;)V"
        } else {
            "()V"
        }),
        attributes: Vec::new(),
    };
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![constructor, native],
        attributes: Vec::new(),
    }
}

fn timer_program(armed: bool) -> (vm::Program, Clock) {
    let profiles = launch::builtin_device_profiles().unwrap();
    let profile = profiles
        .iter()
        .find(|p| p.profile_id() == "nokia-featurephone")
        .unwrap();
    let resolved = profile
        .resolve(device_profile::ProfileOverrides::default())
        .unwrap();
    let limits = vm::Limits::default();
    let mut program = vm::Program::new();
    runtime::install_rust_bootstrap(
        &mut program,
        &limits,
        &resolved,
        &std::collections::HashSet::<String>::new(),
        false,
    )
    .unwrap();
    runtime::register_profile_natives(&mut program, profile).unwrap();
    for app in [false, true] {
        program.add_class(&timer_class(app), &limits).unwrap();
    }
    program
        .native_registry_mut()
        .register(
            NativeSignature::new(
                "fixtures/TimerApp",
                "arm",
                "(Ljava/util/Timer;Ljava/util/TimerTask;)V",
            ),
            move |context, args| {
                let [
                    NativeValue::Reference(Some(timer)),
                    NativeValue::Reference(Some(task)),
                ] = args
                else {
                    panic!("expected Timer and TimerTask");
                };
                if armed {
                    context.schedule_timer_task(*timer, *task, 37, 37, false)?;
                }
                Ok(None)
            },
        )
        .unwrap();
    let clock = Clock::default();
    let callback = clock.clone();
    program
        .native_registry_mut()
        .register(
            NativeSignature::new("fixtures/TimedTask", "run", "()V"),
            move |_, _| {
                // A synchronous callback consumes time but still leaves most of
                // its period available to other work.
                let start = callback.now.fetch_add(9, Ordering::Relaxed);
                callback.firings.lock().unwrap().push(start);
                Ok(None)
            },
        )
        .unwrap();
    (program, clock)
}

#[test]
fn driver_ticks_service_timers_without_display_or_worker_bytecode() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::driver_ticks_service_timers_without_display_or_worker_bytecode"
    )) {
        return;
    }
    let (program, mut host) = timer_program(true);
    let clock = host.clone();
    let mut turns = [
        (36, vm::DriverStep::Tick, 0),
        (37, vm::DriverStep::Tick, 0),
        (100, vm::DriverStep::Idle, 1),
        (100, vm::DriverStep::Tick, 1),
        (200, vm::DriverStep::Stop, 2),
    ]
    .into_iter();
    program
        .execute_instance_step_driver_with_context(
            "fixtures/TimerApp",
            |_| {
                let (now, step, expected_firings) = turns.next().expect("bounded driver");
                assert_eq!(clock.firings.lock().unwrap().len(), expected_firings);
                clock.now.store(now, Ordering::Relaxed);
                Ok(step)
            },
            vm::Limits::default(),
            false,
            &mut host,
        )
        .unwrap();
    assert_eq!(*clock.firings.lock().unwrap(), [37, 100]);
}

#[test]
#[ignore = "manual release timing comparison"]
fn idle_timer_driver_release_timing() {
    for armed in [false, true] {
        let (program, mut host) = timer_program(armed);
        let mut ticks = 100_000;
        let started = std::time::Instant::now();
        program
            .execute_instance_step_driver_with_context(
                "fixtures/TimerApp",
                |_| {
                    if ticks == 0 {
                        return Ok(vm::DriverStep::Stop);
                    }
                    ticks -= 1;
                    Ok(vm::DriverStep::Tick)
                },
                vm::Limits::default(),
                false,
                &mut host,
            )
            .unwrap();
        eprintln!(
            "idle driver armed={armed} ticks=100000 elapsed_us={} callbacks={}",
            started.elapsed().as_micros(),
            host.firings.lock().unwrap().len()
        );
    }
}
