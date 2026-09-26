use super::*;

use std::cell::Cell;
use std::rc::Rc;

struct DriverCancellationContext(Rc<Cell<bool>>);

impl HostServices for DriverCancellationContext {
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
    fn execution_cancelled(&self) -> bool {
        self.0.get()
    }
}

#[test]
fn driver_observes_cancellation_when_idle_or_without_enough_bytecode_for_a_poll() {
    let mut program = Program::new();
    program
        .classes
        .insert("App".into(), test_class_definition(None));
    for name in ["<init>", "callback"] {
        let method = runtime_method("App", name, "()V", &[0xb1], 0, 1, vec![None], false);
        program.methods.insert(method.key.clone(), method);
    }
    for mode in 0..3 {
        let cancelled = Rc::new(Cell::new(false));
        let mut context = DriverCancellationContext(Rc::clone(&cancelled));
        let mut turns = 0;
        let result = program.execute_instance_step_driver_with_context(
            "App",
            |_| {
                turns += 1;
                if turns == 3 {
                    cancelled.set(true);
                }
                // The sentinel keeps this test bounded on the broken driver.
                if turns > 3 {
                    return Ok(DriverStep::Stop);
                }
                Ok(match mode {
                    0 => DriverStep::Idle,
                    1 => DriverStep::Tick,
                    _ => DriverStep::Call(InstanceCall {
                        target: CallTarget::Instance,
                        name: "callback".into(),
                        descriptor: "()V".into(),
                        arguments: vec![],
                    }),
                })
            },
            Limits::default(),
            false,
            &mut context,
        );
        assert_eq!(result.unwrap_err().code(), "execution-cancelled");
        assert_eq!(turns, 3);
    }
}

#[test]
fn vm_host_stack_wrapper_establishes_a_trackable_red_zone() {
    assert!(!host_stack_has_red_zone(None));
    assert!(!host_stack_has_red_zone(Some(HOST_STACK_RED_ZONE - 1)));
    assert!(host_stack_has_red_zone(Some(HOST_STACK_RED_ZONE)));

    let remaining = with_vm_host_stack(stacker::remaining_stack)
        .expect("VM host stack must have a known lower bound inside the wrapper");
    assert!(remaining >= HOST_STACK_RED_ZONE);
}

#[derive(Default)]
struct FlightContext {
    events: Vec<String>,
    frames: usize,
    realtime: bool,
}

impl HostServices for FlightContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn vm_flight_recorder_enabled(&self) -> bool {
        true
    }

    fn realtime_pacing(&self) -> bool {
        self.realtime
    }

    fn record_vm_flight(&mut self, event: String) {
        self.events.push(event);
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }

    fn present_lcdui_frame(
        &mut self,
        _width: u32,
        _height: u32,
        _pixels: &[u32],
    ) -> Result<(), EmuError> {
        self.frames = self.frames.saturating_add(1);
        Ok(())
    }
}

struct MediaPumpContext {
    now_millis: Rc<Cell<i64>>,
    pump_times_micros: Vec<i64>,
}

impl HostServices for MediaPumpContext {
    fn monotonic_millis(&self) -> i64 {
        self.now_millis.get()
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

    fn mmapi_pump(&mut self, now_micros: i64) -> Result<(), EmuError> {
        self.pump_times_micros.push(now_micros);
        Ok(())
    }
}

#[test]
fn instance_driver_pumps_media_when_host_time_advances() {
    let constructor = runtime_method("App", "<init>", "()V", &[0xb1], 0, 1, Vec::new(), false);
    let mut program = Program::new();
    program.classes.insert(
        "App".to_owned(),
        Class {
            super_name: None,
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    program.methods.insert(constructor.key.clone(), constructor);

    let now_millis = Rc::new(Cell::new(0));
    let driver_now_millis = Rc::clone(&now_millis);
    let mut times = [5, 5, 12].into_iter();
    let mut context = MediaPumpContext {
        now_millis,
        pump_times_micros: Vec::new(),
    };

    program
        .execute_instance_step_driver_with_context(
            "App",
            |_| {
                Ok(if let Some(now) = times.next() {
                    driver_now_millis.set(now);
                    DriverStep::Tick
                } else {
                    DriverStep::Stop
                })
            },
            Limits::default(),
            false,
            &mut context,
        )
        .unwrap();

    assert_eq!(context.pump_times_micros, [5_000, 12_000]);
}

#[test]
fn flight_recorder_observes_release_method_entry_and_return() {
    let entry = method(&[0x10, 7, 0xac], 1, 0);
    let mut program = Program::new();
    program.methods.insert(entry.key.clone(), entry);
    let mut context = FlightContext::default();

    let execution = program
        .execute_with_context("T", "main", "()I", Limits::default(), false, &mut context)
        .unwrap();

    assert_eq!(execution.value, Some(Value::Int(7)));
    assert_eq!(context.events.len(), 2);
    assert!(context.events[0].contains("call-enter T::main()I args=[]"));
    assert!(context.events[1].contains("call-exit T::main()I return Some(Int(7))"));
}

#[test]
fn application_shutdown_prevents_a_worker_turn_inside_teardown() {
    let constants = vec![
        None,
        Some(Constant::Methodref {
            class_index: 2,
            name_and_type_index: 3,
        }),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::NameAndType {
            name_index: 5,
            descriptor_index: 6,
        }),
        Some(Constant::Utf8("java/lang/Thread".to_owned())),
        Some(Constant::Utf8("start".to_owned())),
        Some(Constant::Utf8("()V".to_owned())),
        Some(Constant::Methodref {
            class_index: 2,
            name_and_type_index: 8,
        }),
        Some(Constant::NameAndType {
            name_index: 9,
            descriptor_index: 6,
        }),
        Some(Constant::Utf8("yield".to_owned())),
    ];
    let constructor = runtime_method(
        "App",
        "<init>",
        "()V",
        &[0x2a, 0xb6, 0, 1, 0xb1],
        1,
        1,
        constants.clone(),
        false,
    );
    let run = runtime_method("App", "run", "()V", &[0xb1], 0, 1, Vec::new(), false);
    let destroy = runtime_method(
        "App",
        "destroy",
        "()V",
        &[0xb8, 0, 7, 0xb1],
        0,
        1,
        constants.clone(),
        false,
    );
    let mut start = runtime_method(
        "java/lang/Thread",
        "start",
        "()V",
        &[],
        0,
        1,
        Vec::new(),
        false,
    );
    start.is_native = true;
    let mut thread_yield = runtime_method(
        "java/lang/Thread",
        "yield",
        "()V",
        &[],
        0,
        0,
        Vec::new(),
        true,
    );
    thread_yield.is_native = true;
    let mut program = Program::new();
    program
        .classes
        .insert("java/lang/Thread".to_owned(), thread_class());
    program.classes.insert(
        "App".to_owned(),
        Class {
            super_name: Some("java/lang/Thread".to_owned()),
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    for method in [constructor, run, destroy, start, thread_yield] {
        program.methods.insert(method.key.clone(), method);
    }
    let destroy_call = InstanceCall {
        target: CallTarget::Instance,
        name: "destroy".to_owned(),
        descriptor: "()V".to_owned(),
        arguments: Vec::new(),
    };

    let mut regular_context = FlightContext::default();
    let mut regular_steps = [DriverStep::Call(destroy_call.clone()), DriverStep::Stop].into_iter();
    program
        .execute_instance_step_driver_with_context(
            "App",
            |_| Ok(regular_steps.next().unwrap()),
            Limits::default(),
            false,
            &mut regular_context,
        )
        .unwrap();
    let destroy_entry = regular_context
        .events
        .iter()
        .position(|event| event.contains("call-enter App::destroy()V"))
        .unwrap();
    let worker_entry = regular_context
        .events
        .iter()
        .position(|event| event.contains("call-enter App::run()V"))
        .unwrap();
    let destroy_exit = regular_context
        .events
        .iter()
        .position(|event| event.contains("call-exit App::destroy()V"))
        .unwrap();
    assert!(destroy_entry < worker_entry && worker_entry < destroy_exit);

    let mut idle_context = FlightContext::default();
    let mut idle_steps = [DriverStep::Idle, DriverStep::Stop].into_iter();
    program
        .execute_instance_step_driver_with_context(
            "App",
            |_| Ok(idle_steps.next().unwrap()),
            Limits::default(),
            false,
            &mut idle_context,
        )
        .unwrap();
    assert!(
        idle_context
            .events
            .iter()
            .all(|event| !event.contains("call-enter App::run()V"))
    );

    let mut teardown_context = FlightContext::default();
    let mut teardown_steps = [
        DriverStep::CallAfterApplicationShutdown(destroy_call),
        DriverStep::Stop,
    ]
    .into_iter();
    program
        .execute_instance_step_driver_with_context(
            "App",
            |_| Ok(teardown_steps.next().unwrap()),
            Limits::default(),
            false,
            &mut teardown_context,
        )
        .unwrap();
    assert!(
        teardown_context
            .events
            .iter()
            .any(|event| event.contains("call-enter App::destroy()V"))
    );
    assert!(
        teardown_context
            .events
            .iter()
            .all(|event| !event.contains("call-enter App::run()V"))
    );
}

#[test]
fn realtime_frame_in_long_running_driver_call_yields_for_host_input() {
    let present_constants = vec![
        None,
        Some(Constant::Methodref {
            class_index: 2,
            name_and_type_index: 3,
        }),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::NameAndType {
            name_index: 5,
            descriptor_index: 6,
        }),
        Some(Constant::Utf8("javax/microedition/lcdui/Canvas".to_owned())),
        Some(Constant::Utf8("present".to_owned())),
        Some(Constant::Utf8("([I)V".to_owned())),
    ];
    let constructor = runtime_method("App", "<init>", "()V", &[0xb1], 0, 1, vec![None], false);
    // Allocate a one-pixel framebuffer, then present it forever without
    // returning from the host-driven callback.
    let start = runtime_method(
        "App",
        "start",
        "()V",
        &[0x04, 0xbc, 10, 0x4c, 0x2b, 0xb8, 0, 1, 0xa7, 0xff, 0xfc],
        1,
        2,
        present_constants.clone(),
        false,
    );
    // A callback dispatched during the host-poll turn may itself publish a
    // frame. That nested presentation must finish synchronously: the host
    // was just polled, and parking a second driver continuation would lose
    // one of the two Java stacks.
    let key = runtime_method(
        "App",
        "key",
        "()V",
        &[0x04, 0xbc, 10, 0xb8, 0, 1, 0xb1],
        1,
        1,
        present_constants,
        false,
    );
    let mut present = runtime_method(
        "javax/microedition/lcdui/Canvas",
        "present",
        "([I)V",
        &[],
        0,
        1,
        vec![None],
        true,
    );
    present.is_native = true;
    let class = |super_name: Option<&str>| Class {
        super_name: super_name.map(str::to_owned),
        interfaces: Vec::new(),
        fields: Vec::new(),
        is_public: true,
        is_abstract: false,
        is_interface: false,
        no_arg_constructor: NoArgConstructor::Public,
    };
    let mut program = Program::new();
    program.classes.insert("App".to_owned(), class(None));
    program
        .classes
        .insert("javax/microedition/lcdui/Canvas".to_owned(), class(None));
    for method in [constructor, start, key, present] {
        program.methods.insert(method.key.clone(), method);
    }

    let call = |name: &str| InstanceCall {
        target: CallTarget::Instance,
        name: name.to_owned(),
        descriptor: "()V".to_owned(),
        arguments: Vec::new(),
    };
    let mut started = false;
    let mut host_polls = 0;
    let mut context = FlightContext {
        realtime: true,
        ..FlightContext::default()
    };
    let limits = Limits {
        max_instructions: 1_000,
        lcd_width: 1,
        lcd_height: 1,
        lcd_normal_width: 1,
        lcd_normal_height: 1,
        ..Limits::default()
    };

    program
        .execute_instance_step_driver_with_context(
            "App",
            |turn| {
                Ok(match turn {
                    DriverTurn::Regular if !started => {
                        started = true;
                        DriverStep::Call(call("start"))
                    }
                    DriverTurn::Regular => DriverStep::Stop,
                    DriverTurn::HostPoll => {
                        host_polls += 1;
                        match host_polls {
                            1 => DriverStep::Tick,
                            2 => DriverStep::Call(call("key")),
                            _ => DriverStep::Stop,
                        }
                    }
                })
            },
            limits,
            false,
            &mut context,
        )
        .unwrap();

    assert!(context.frames >= 4);
    assert_eq!(host_polls, 3);
    assert!(
        context
            .events
            .iter()
            .any(|event| event.contains("call-enter App::key()V"))
    );
}

#[test]
fn lifecycle_suspend_parks_a_callback_that_never_reaches_a_host_native() {
    use std::sync::atomic::{AtomicBool, Ordering};

    struct SuspendContext {
        suspended: Arc<AtomicBool>,
    }

    impl HostServices for SuspendContext {
        fn monotonic_millis(&self) -> i64 {
            0
        }

        fn wall_clock_millis(&self) -> i64 {
            0
        }

        fn system_property(&self, _: &str) -> Option<&str> {
            None
        }

        fn execution_suspended(&self) -> bool {
            self.suspended.load(Ordering::Acquire)
        }

        fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
            Ok(None)
        }
    }

    let constructor = runtime_method("App", "<init>", "()V", &[0xb1], 0, 1, vec![None], false);
    // The backward branch never invokes a native and never returns. A
    // lifecycle signal must still surface a host-poll turn before the
    // bounded instruction limit is exhausted.
    let start = runtime_method(
        "App",
        "start",
        "()V",
        &[0x00, 0xa7, 0xff, 0xff],
        0,
        1,
        vec![None],
        false,
    );
    let mut program = Program::new();
    program.classes.insert(
        "App".to_owned(),
        Class {
            super_name: None,
            interfaces: Vec::new(),
            fields: Vec::new(),
            is_public: true,
            is_abstract: false,
            is_interface: false,
            no_arg_constructor: NoArgConstructor::Public,
        },
    );
    for method in [constructor, start] {
        program.methods.insert(method.key.clone(), method);
    }

    let suspended = Arc::new(AtomicBool::new(true));
    let mut context = SuspendContext {
        suspended: Arc::clone(&suspended),
    };
    let mut started = false;
    let mut host_polls = 0;
    let execution = program
        .execute_instance_step_driver_with_context(
            "App",
            |turn| {
                Ok(match turn {
                    DriverTurn::Regular if !started => {
                        started = true;
                        DriverStep::Call(InstanceCall {
                            target: CallTarget::Instance,
                            name: "start".to_owned(),
                            descriptor: "()V".to_owned(),
                            arguments: Vec::new(),
                        })
                    }
                    DriverTurn::HostPoll => {
                        host_polls += 1;
                        suspended.store(false, Ordering::Release);
                        DriverStep::Stop
                    }
                    DriverTurn::Regular => DriverStep::Stop,
                })
            },
            Limits {
                max_instructions: 4_096,
                ..Limits::default()
            },
            false,
            &mut context,
        )
        .unwrap();

    assert_eq!(host_polls, 1);
    assert!((1_024..2_048).contains(&execution.instructions));
}

#[test]
fn realtime_sleep_in_long_running_driver_call_yields_for_host_input() {
    let sleep_constants = vec![
        None,
        Some(Constant::Methodref {
            class_index: 2,
            name_and_type_index: 3,
        }),
        Some(Constant::Class { name_index: 4 }),
        Some(Constant::NameAndType {
            name_index: 5,
            descriptor_index: 6,
        }),
        Some(Constant::Utf8("java/lang/Thread".to_owned())),
        Some(Constant::Utf8("sleep".to_owned())),
        Some(Constant::Utf8("(J)V".to_owned())),
    ];
    let constructor = runtime_method("App", "<init>", "()V", &[0xb1], 0, 1, vec![None], false);
    let start = runtime_method(
        "App",
        "start",
        "()V",
        &[0x0a, 0xb8, 0, 1, 0xa7, 0xff, 0xfc],
        2,
        1,
        sleep_constants,
        false,
    );
    let key = runtime_method("App", "key", "()V", &[0xb1], 0, 1, vec![None], false);
    let mut sleep = runtime_method(
        "java/lang/Thread",
        "sleep",
        "(J)V",
        &[],
        0,
        2,
        vec![None],
        true,
    );
    sleep.is_native = true;
    let class = Class {
        super_name: None,
        interfaces: Vec::new(),
        fields: Vec::new(),
        is_public: true,
        is_abstract: false,
        is_interface: false,
        no_arg_constructor: NoArgConstructor::Public,
    };
    let mut program = Program::new();
    program.classes.insert("App".to_owned(), class.clone());
    program.classes.insert("java/lang/Thread".to_owned(), class);
    for method in [constructor, start, key, sleep] {
        program.methods.insert(method.key.clone(), method);
    }

    let call = |name: &str| InstanceCall {
        target: CallTarget::Instance,
        name: name.to_owned(),
        descriptor: "()V".to_owned(),
        arguments: Vec::new(),
    };
    let mut steps = [
        DriverStep::Call(call("start")),
        DriverStep::Call(call("key")),
        DriverStep::Stop,
    ]
    .into_iter();
    let mut context = FlightContext {
        realtime: true,
        ..FlightContext::default()
    };
    let limits = Limits {
        max_instructions: 1_000,
        ..Limits::default()
    };

    program
        .execute_instance_step_driver_with_context(
            "App",
            |_| Ok(steps.next().unwrap()),
            limits,
            false,
            &mut context,
        )
        .unwrap();

    assert!(
        context
            .events
            .iter()
            .any(|event| event.contains("call-enter App::key()V"))
    );
}

#[test]
fn application_shutdown_removes_workers_and_timer_dispatch() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let worker = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let timer_thread = machine
        .heap
        .managed
        .allocate_object("java/lang/Thread", HashMap::new())
        .unwrap();
    let timer = machine
        .heap
        .managed
        .allocate_object("java/util/Timer", HashMap::new())
        .unwrap();
    let task = machine
        .heap
        .managed
        .allocate_object("java/util/TimerTask", HashMap::new())
        .unwrap();
    machine
        .scheduler
        .thread_states
        .insert(worker, ThreadState::Runnable);
    machine.scheduler.runnable_threads.push_back(worker);
    machine.scheduler.timer_threads.insert(timer, timer_thread);
    machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
        timer,
        task,
        deadline: 0,
        period: 0,
        fixed_rate: false,
    });
    machine
        .scheduler
        .interrupted_threads
        .insert(worker.to_raw());
    machine
        .scheduler
        .interrupted_threads
        .insert(timer_thread.to_raw());
    machine.scheduler.interrupted_threads.insert(MAIN_THREAD_ID);

    machine.terminate_application_threads();

    assert_eq!(
        machine.scheduler.thread_states.get(&worker),
        Some(&ThreadState::Terminated)
    );
    assert!(machine.scheduler.runnable_threads.is_empty());
    assert!(machine.scheduler.scheduled_tasks.is_empty());
    assert!(machine.scheduler.timer_threads.is_empty());
    assert!(
        !machine
            .scheduler
            .interrupted_threads
            .contains(&worker.to_raw())
    );
    assert!(
        !machine
            .scheduler
            .interrupted_threads
            .contains(&timer_thread.to_raw())
    );
    assert!(
        machine
            .scheduler
            .interrupted_threads
            .contains(&MAIN_THREAD_ID)
    );
}
