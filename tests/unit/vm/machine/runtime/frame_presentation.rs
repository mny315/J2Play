use super::*;
use std::cell::Cell;
use std::rc::Rc;

struct FrameDelayContext {
    now: Rc<Cell<i64>>,
    frames: Rc<Cell<u32>>,
    reservations: Rc<Cell<u32>>,
    realtime: bool,
    fail_frame: Rc<Cell<bool>>,
}

impl Default for FrameDelayContext {
    fn default() -> Self {
        Self {
            now: Rc::new(Cell::new(100)),
            frames: Rc::default(),
            reservations: Rc::default(),
            realtime: true,
            fail_frame: Rc::default(),
        }
    }
}

impl HostServices for FrameDelayContext {
    fn monotonic_millis(&self) -> i64 {
        self.now.get()
    }
    fn wall_clock_millis(&self) -> i64 {
        self.now.get()
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
    fn realtime_pacing(&self) -> bool {
        self.realtime
    }
    fn lcdui_frame_delay_millis(&mut self, changed: bool) -> u64 {
        if !changed {
            return 0;
        }
        self.reservations.set(self.reservations.get() + 1);
        20
    }
    fn pace_millis(&mut self, millis: u64) -> Result<(), EmuError> {
        self.now
            .set(self.now.get() + i64::try_from(millis).unwrap());
        Ok(())
    }
    fn present_lcdui_frame(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u32],
    ) -> Result<(), EmuError> {
        assert_eq!((width, height, pixels.len()), (2, 2, 4));
        if self.fail_frame.get() {
            return Err(EmuError::new(
                diagnostics::Category::Platform,
                "test-frame",
                "test sink failure",
            ));
        }
        self.frames.set(self.frames.get() + 1);
        Ok(())
    }
}

fn presentation_method() -> Method {
    let mut native = runtime_method(
        "javax/microedition/lcdui/Canvas",
        "present",
        "([I)V",
        &[],
        0,
        1,
        vec![None],
        true,
    );
    native.is_native = true;
    native
}

fn frame_limits() -> Limits {
    Limits {
        lcd_width: 2,
        lcd_height: 2,
        lcd_normal_width: 2,
        lcd_normal_height: 2,
        ..Limits::default()
    }
}

#[test]
fn frame_deadline_parks_only_presenter_keeps_pixels_rooted_and_preserves_interrupt() {
    let native = presentation_method();
    let loader_run = runtime_method(
        "Loader",
        "run",
        "()V",
        &[0x10, 42, 0x57, 0xb1],
        1,
        1,
        vec![None],
        false,
    );
    let mut program = Program::new();
    program.methods.insert(native.key.clone(), native.clone());
    program.methods.insert(loader_run.key.clone(), loader_run);
    program
        .classes
        .insert("Loader".into(), test_class_definition(None));
    let mut context = FrameDelayContext::default();
    let now = Rc::clone(&context.now);
    let frames = Rc::clone(&context.frames);
    let reservations = Rc::clone(&context.reservations);
    let mut machine = program.machine(frame_limits(), false, &mut context);
    let presenter = machine
        .heap
        .managed
        .allocate_object("Presenter", HashMap::new())
        .unwrap();
    let loader = machine
        .heap
        .managed
        .allocate_object("Loader", HashMap::new())
        .unwrap();
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 4)
        .unwrap();
    let garbage = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 32)
        .unwrap();
    machine.scheduler.current_thread = presenter.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(presenter, ThreadState::Running);
    machine
        .scheduler
        .thread_states
        .insert(loader, ThreadState::Runnable);
    machine.scheduler.runnable_threads.push_back(loader);
    let CallOutcome::Suspend(continuation) = machine
        .call(&native, [Value::Reference(Some(pixels))], 1)
        .unwrap()
    else {
        panic!("frame deadline must park the presenting worker");
    };
    machine
        .scheduler
        .thread_continuations
        .insert(presenter, continuation);
    assert_eq!(now.get(), 100);
    assert_eq!(frames.get(), 0);
    assert_eq!(
        machine.scheduler.sleeping_threads.get(&presenter),
        Some(&120)
    );
    let roots = machine.roots(&[], &[]);
    machine.collect_heap(roots);
    assert!(machine.heap.managed.get(pixels).is_ok());
    assert!(machine.heap.managed.get(garbage).is_err());

    machine.scheduler.current_thread = MAIN_THREAD_ID;
    assert!(machine.run_one_thread(1).unwrap());
    assert_eq!(
        machine.scheduler.thread_states.get(&loader),
        Some(&ThreadState::Terminated)
    );
    assert_eq!(
        now.get(),
        100,
        "loader must finish before the frame deadline"
    );

    // An interrupt wakes the native continuation early without converting
    // frame pacing to an interruptible Java sleep or reserving a second slot.
    now.set(119);
    machine.scheduler.current_thread = presenter.to_raw();
    machine
        .scheduler
        .thread_states
        .insert(presenter, ThreadState::Running);
    machine
        .scheduler
        .interrupted_threads
        .insert(presenter.to_raw());
    let continuation = machine
        .scheduler
        .thread_continuations
        .remove(&presenter)
        .unwrap();
    let CallOutcome::Suspend(continuation) =
        machine.resume_suspended_call(continuation, 1).unwrap()
    else {
        panic!("early resume must retain the original frame deadline");
    };
    assert_eq!(frames.get(), 0);
    now.set(120);
    machine
        .scheduler
        .thread_states
        .insert(presenter, ThreadState::Running);
    assert!(matches!(
        machine.resume_suspended_call(continuation, 1).unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(frames.get(), 1);
    assert_eq!(reservations.get(), 1);
    assert!(
        machine
            .scheduler
            .interrupted_threads
            .contains(&presenter.to_raw())
    );
    assert!(!machine.scheduler.sleeping_threads.contains_key(&presenter));
    assert!(machine.scheduler.suspend_requested);
}

#[test]
fn synchronous_callbacks_and_deterministic_hosts_do_not_acquire_frame_continuations() {
    let native = presentation_method();
    let mut program = Program::new();
    program.methods.insert(native.key.clone(), native.clone());
    for (realtime, main, display, timer, paint) in [
        (true, true, false, false, false),
        (true, false, true, false, false),
        (true, false, false, true, false),
        (true, false, false, false, true),
        (false, false, false, false, false),
    ] {
        let mut context = FrameDelayContext {
            realtime,
            ..FrameDelayContext::default()
        };
        let now = Rc::clone(&context.now);
        let frames = Rc::clone(&context.frames);
        let reservations = Rc::clone(&context.reservations);
        let mut machine = program.machine(frame_limits(), false, &mut context);
        let thread = machine
            .heap
            .managed
            .allocate_object("Presenter", HashMap::new())
            .unwrap();
        let pixels = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 4)
            .unwrap();
        machine.scheduler.current_thread = if main {
            MAIN_THREAD_ID
        } else {
            thread.to_raw()
        };
        machine.scheduler.dispatching_display = display;
        machine.scheduler.dispatching_timer = timer;
        machine.scheduler.dispatching_canvas_paint = paint;
        assert!(matches!(
            machine
                .call(&native, [Value::Reference(Some(pixels))], 1)
                .unwrap(),
            CallOutcome::Return(None)
        ));
        assert!(machine.scheduler.sleeping_threads.is_empty());
        assert_eq!(now.get(), if realtime { 120 } else { 100 });
        assert_eq!(frames.get(), 1);
        assert_eq!(reservations.get(), u32::from(realtime));
    }
}

#[test]
fn invalid_frame_is_rejected_before_reserving_a_presentation_slot() {
    let native = presentation_method();
    let mut context = FrameDelayContext::default();
    let reservations = Rc::clone(&context.reservations);
    let frames = Rc::clone(&context.frames);
    let program = Program::new();
    let mut machine = program.machine(frame_limits(), false, &mut context);
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 3)
        .unwrap();
    let error = machine
        .call(&native, [Value::Reference(Some(pixels))], 1)
        .err()
        .unwrap();
    assert_eq!(error.code(), "framebuffer-size");
    assert_eq!(reservations.get(), 0);
    assert_eq!(frames.get(), 0);
}

#[test]
fn repeated_frame_skips_delay_but_preserves_host_delivery_and_detects_pixel_changes() {
    let native = presentation_method();
    let mut context = FrameDelayContext::default();
    let now = Rc::clone(&context.now);
    let reservations = Rc::clone(&context.reservations);
    let frames = Rc::clone(&context.frames);
    let program = Program::new();
    let mut machine = program.machine(frame_limits(), false, &mut context);
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 4)
        .unwrap();
    let same_pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 4)
        .unwrap();
    for frame in [pixels, pixels, same_pixels] {
        assert!(matches!(
            machine
                .call(&native, [Value::Reference(Some(frame))], 1)
                .unwrap(),
            CallOutcome::Return(None)
        ));
    }
    assert_eq!(now.get(), 120);
    assert_eq!(reservations.get(), 1);
    assert_eq!(
        frames.get(),
        3,
        "unchanged frames still refresh host composition"
    );
    machine
        .heap
        .managed
        .array_set(same_pixels, 3, HeapValue::Int(-1))
        .unwrap();
    machine
        .call(&native, [Value::Reference(Some(same_pixels))], 1)
        .unwrap();
    assert_eq!(now.get(), 140);
    assert_eq!(reservations.get(), 2);
    machine
        .call(&native, [Value::Reference(Some(pixels))], 1)
        .unwrap();
    assert_eq!(now.get(), 160);
    assert_eq!(reservations.get(), 3);
    assert_eq!(frames.get(), 5);
    assert!(
        !machine
            .java_frame_matches_presented(pixels, (1, 4))
            .unwrap(),
        "equal pixel counts with different dimensions are not the same frame"
    );
}

#[test]
fn failed_frame_does_not_become_a_successful_presentation_baseline() {
    let native = presentation_method();
    let mut context = FrameDelayContext::default();
    let reservations = Rc::clone(&context.reservations);
    let frames = Rc::clone(&context.frames);
    let fail = Rc::clone(&context.fail_frame);
    let program = Program::new();
    let mut machine = program.machine(frame_limits(), false, &mut context);
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 4)
        .unwrap();
    machine
        .call(&native, [Value::Reference(Some(pixels))], 1)
        .unwrap();
    fail.set(true);
    machine
        .heap
        .managed
        .array_set(pixels, 0, HeapValue::Int(7))
        .unwrap();
    assert_eq!(
        machine
            .call(&native, [Value::Reference(Some(pixels))], 1)
            .err()
            .unwrap()
            .code(),
        "test-frame"
    );
    fail.set(false);
    machine
        .call(&native, [Value::Reference(Some(pixels))], 1)
        .unwrap();
    assert_eq!(reservations.get(), 3);
    assert_eq!(frames.get(), 2);
}

#[test]
fn realtime_frame_presentation_yields_only_resumable_workers_to_the_host() {
    let native = presentation_method();
    let mut program = Program::new();
    program.methods.insert(native.key.clone(), native.clone());

    for (realtime, dispatching_timer, should_suspend) in [
        (false, false, false),
        (true, false, true),
        (true, true, false),
    ] {
        let mut context = PacingContext {
            now_millis: 100,
            paced_millis: 0,
            realtime,
            instructions_per_second: None,
            yield_25_millis: false,
            random_seed: None,
        };
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let worker = machine
            .heap
            .managed
            .allocate_object("worker", HashMap::new())
            .unwrap();
        let pixels = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 240 * 320)
            .unwrap();
        machine.scheduler.current_thread = worker.to_raw();
        // A TimerTask is allowed to cancel its own Timer. cancel_timer()
        // then removes timer_threads[current Timer], but the callback is
        // still synchronously active until run_due_timer_tasks() returns.
        machine.scheduler.dispatching_timer = dispatching_timer;
        let outcome = machine
            .call(&native, vec![Value::Reference(Some(pixels))], 1)
            .unwrap();
        assert!(matches!(outcome, CallOutcome::Return(None)));
        assert_eq!(machine.scheduler.suspend_requested, should_suspend);
    }
}

#[test]
fn realtime_frame_inside_display_turn_defers_suspend_until_dispatch_returns() {
    let native = presentation_method();
    let mut program = Program::new();
    program.methods.insert(native.key.clone(), native.clone());
    let mut context = PacingContext {
        now_millis: 100,
        paced_millis: 0,
        realtime: true,
        instructions_per_second: None,
        yield_25_millis: false,
        random_seed: None,
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 240 * 320)
        .unwrap();
    machine.scheduler.host_driver_active = true;
    machine.scheduler.dispatching_display = true;

    let outcome = machine
        .call(&native, vec![Value::Reference(Some(pixels))], 1)
        .unwrap();

    assert!(matches!(outcome, CallOutcome::Return(None)));
    assert!(!machine.scheduler.suspend_requested);
    assert!(machine.scheduler.driver_host_poll_yield);

    let after_dispatch = runtime_method(
        "App",
        "afterDispatch",
        "()V",
        &[0xb1],
        0,
        0,
        vec![None],
        true,
    );
    machine.scheduler.dispatching_display = false;
    let outcome = machine.call(&after_dispatch, Vec::new(), 1).unwrap();
    assert!(matches!(outcome, CallOutcome::Suspend(_)));
}
