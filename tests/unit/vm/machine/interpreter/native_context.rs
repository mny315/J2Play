use super::*;

#[test]
fn mmapi_pressure_collects_unreachable_wrappers_and_retries_once() {
    let mut host = BoundedMmapiContext::with_live_handle(1);
    let mut heap = Heap::new(1_024);
    let stale = heap
        .allocate_object(
            "com/nokia/mid/sound/Sound",
            HashMap::from([(
                "com/nokia/mid/sound/Sound.handle:J".to_owned(),
                HeapValue::Long(1),
            )]),
        )
        .unwrap();
    let mut state = NativeContextTestState::new(heap);
    let mut context = state.context(&mut host, Vec::new());

    assert_eq!(context.mmapi_create_bytes("audio/mpeg", b"x").unwrap(), 2);
    assert!(context.heap.managed.get(stale).is_err());

    let live = context
        .heap
        .managed
        .allocate_object(
            "com/nokia/mid/sound/Sound",
            HashMap::from([(
                "com/nokia/mid/sound/Sound.handle:J".to_owned(),
                HeapValue::Long(2),
            )]),
        )
        .unwrap();
    context.heap.temporary_roots.push(live);
    assert_eq!(
        context
            .mmapi_create_bytes("audio/mpeg", b"y")
            .unwrap_err()
            .code(),
        "player-limit"
    );
    assert!(context.heap.managed.get(live).is_ok());

    assert_eq!(host.allocation_calls, 4);
    assert_eq!(host.retain_calls, [Vec::<u64>::new(), vec![2]]);
    assert_eq!(host.live_handles, [2]);
}

#[test]
fn mmapi_synthesis_errors_preserve_cancellation_without_gc_retry() {
    for (code, java_class) in [
        (
            "media-work-limit",
            Some("javax/microedition/media/MediaException"),
        ),
        ("execution-cancelled", None),
    ] {
        let mut host = BoundedMmapiContext::with_live_handle(1);
        host.transition_error = code;
        let mut state = NativeContextTestState::new(Heap::new(1_024));
        let mut context = state.context(&mut host, Vec::new());
        let error = context.mmapi_transition(1, 1, 0).unwrap_err();
        assert_eq!(error.code(), code);
        assert_eq!(java_error_class(&error), java_class);
        assert_eq!(host.transition_calls, 1);
        assert!(host.retain_calls.is_empty());
        assert_eq!(host.live_handles, [1]);
    }
}

#[test]
fn single_tone_pressure_collects_only_unreachable_players_and_retries_once() {
    for keep_alive in [false, true] {
        let mut host = BoundedMmapiContext::with_live_handle(1);
        let mut heap = Heap::new(1_024);
        let player = heap
            .allocate_object(
                "com/nokia/mid/sound/Sound",
                HashMap::from([(
                    "com/nokia/mid/sound/Sound.handle:J".to_owned(),
                    HeapValue::Long(1),
                )]),
            )
            .unwrap();
        let mut state = NativeContextTestState::new(heap);
        let roots = if keep_alive { vec![player] } else { Vec::new() };
        let mut context = state.context(&mut host, roots);
        let result = context.mmapi_play_tone(60, 100, 100, 0);
        if keep_alive {
            assert_eq!(result.unwrap_err().code(), "media-limit");
        } else {
            result.unwrap();
        }
        assert_eq!(context.heap.managed.get(player).is_ok(), keep_alive);
        assert_eq!(host.tone_calls, 2);
        assert_eq!(
            host.retain_calls,
            [if keep_alive { vec![1] } else { Vec::new() }]
        );
    }
}

#[test]
fn ordinary_gc_sweeps_unreachable_mmapi_wrappers() {
    let program = Program::new();
    let mut context = BoundedMmapiContext::with_live_handle(1);
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let stale = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/media/PlayerImpl",
            HashMap::from([(
                "javax/microedition/media/PlayerImpl.handle:J".to_owned(),
                HeapValue::Long(1),
            )]),
        )
        .unwrap();

    machine.collect_heap(Vec::new());
    assert!(machine.heap.managed.get(stale).is_err());
    drop(machine);

    assert!(context.live_handles.is_empty());
    assert_eq!(context.retain_calls, [Vec::<u64>::new()]);
}

#[test]
fn siemens_media_player_handles_participate_in_native_gc_ownership() {
    let mut heap = Heap::new(1_024);
    heap.allocate_object(
        "com/siemens/mp/media/PlayerImpl",
        HashMap::from([(
            "com/siemens/mp/media/PlayerImpl.handle:J".to_owned(),
            HeapValue::Long(17),
        )]),
    )
    .unwrap();

    assert_eq!(live_mmapi_handles(&heap), [17]);
}

#[test]
fn dynamic_native_strings_are_distinct_and_collectible() {
    for (limit, prefill) in [(12, false), (16, true)] {
        let mut host = DefaultNativeContext;
        let mut heap = Heap::new(limit);
        if prefill {
            // The first String object fits, but its payload requires GC. The
            // temporary root protecting that payload must end with the write.
            heap.allocate_object("java/lang/Object", HashMap::new())
                .unwrap();
        }
        let mut state = NativeContextTestState::new(heap);
        let mut context = state.context(&mut host, Vec::new());
        let first = Handle::from_raw(context.intern_java_utf16(&[u16::from(b'a')]).unwrap());
        let second = Handle::from_raw(context.intern_java_string("a").unwrap());
        assert_ne!(first, second);
        assert!(context.heap.managed.get(first).is_err());
        assert_eq!(
            context.read_java_utf16(second.to_raw()).unwrap(),
            [u16::from(b'a')]
        );
        assert_eq!(context.heap.string_values.len(), 1);
    }
}

#[test]
fn native_utf8_string_accounts_its_utf16_payload_once() {
    let mut host = DefaultNativeContext;
    let mut state = NativeContextTestState::new(Heap::new(14));
    let mut context = state.context(&mut host, Vec::new());
    let string = context.intern_java_string("я🙂").unwrap();
    assert_eq!(
        context.read_java_utf16(string).unwrap(),
        [0x044f, 0xd83d, 0xde42]
    );
    assert_eq!(context.heap.managed.bytes(), 14);
    assert!(context.heap.interned_strings.is_empty());
}

#[test]
fn native_context_cannot_reinitialize_a_canonical_string() {
    let mut host = DefaultNativeContext;
    let mut heap = Heap::new(64);
    let canonical = heap
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    heap.set_external_bytes(canonical, 4).unwrap();
    let mut state = NativeContextTestState::new(heap);
    state.heap.string_values =
        crate::machine::StringValues::from([(canonical, vec![u16::from(b'a')])]);
    state.heap.interned_strings = HashMap::from([(vec![u16::from(b'a')], canonical)]);
    let mut context = state.context(&mut host, vec![canonical]);

    let error = context
        .write_java_utf16(canonical.to_raw(), Box::new([u16::from(b'b')]))
        .unwrap_err();

    assert_eq!(error.code(), "illegal-state");
    assert_eq!(context.heap.string_values[&canonical], [u16::from(b'a')]);
    assert_eq!(context.heap.interned_strings.len(), 1);
}

pub(crate) struct NativeContextTestState {
    program: Program,
    pub(crate) heap: HeapState,
    m3g: M3gState,
    micro3d: Micro3dState,
    jsr239: Jsr239State,
    classes: ClassState,
    execution: ExecutionState,
    scheduler: SchedulerState,
    arguments: Vec<Value>,
}

impl NativeContextTestState {
    pub(crate) fn new(heap: Heap) -> Self {
        Self {
            program: Program::new(),
            heap: HeapState {
                managed: heap,
                string_values: crate::machine::StringValues::new(),
                interned_strings: HashMap::new(),
                weak_references: HashMap::new(),
                throwable_traces: HashMap::new(),
                managed_heap_limit_throwables: HashSet::new(),
                frame_roots: FrameRoots::default(),
                temporary_roots: Vec::new(),
                immutable_image_pixels: HashMap::new(),
            },
            m3g: M3gState {
                runtime: m3g::Runtime::default(),
                graphics3d: None,
                graphics: M3gGraphicsState::new(1, 1, m3g::RenderLimits::default()),
                metrics: M3gExecutionMetrics::default(),
                render_totals: m3g::RenderStats::default(),
            },
            micro3d: Micro3dState {
                runtime: micro3d::Runtime::default(),
                target: None,
                target_scissor: [0; 4],
                command_scissor: [0; 4],
                render_pending: false,
                render_diagnostic_calls: 0,
                render_diagnostics: VecDeque::new(),
            },
            classes: ClassState::default(),
            jsr239: Jsr239State::default(),
            execution: ExecutionState::default(),
            scheduler: SchedulerState::new(0, MAX_TIMER_POLL_INSTRUCTIONS),
            arguments: Vec::new(),
        }
    }

    pub(crate) fn context<'a>(
        &'a mut self,
        host: &'a mut dyn HostServices,
        roots: Vec<Handle>,
    ) -> MachineNativeContext<'a> {
        self.arguments = roots
            .into_iter()
            .map(|handle| Value::Reference(Some(handle)))
            .collect();
        MachineNativeContext {
            host,
            heap: &mut self.heap,
            m3g: &mut self.m3g,
            micro3d: &mut self.micro3d,
            jsr239: &self.jsr239,
            classes: &mut self.classes,
            program: &self.program,
            execution: &self.execution,
            scheduler: &mut self.scheduler,
            arguments: &self.arguments,
        }
    }
}

#[derive(Default)]
struct ForwardedHostContext {
    text_input_active: bool,
    console_output: String,
    execution_cancelled: bool,
}

impl HostServices for ForwardedHostContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn display_colors(&self) -> i32 {
        65_536
    }

    fn canvas_pointer_events(&self) -> bool {
        true
    }

    fn canvas_pointer_motion_events(&self) -> bool {
        true
    }

    fn set_text_input_active(&mut self, active: bool) {
        self.text_input_active = active;
    }

    fn execution_cancelled(&self) -> bool {
        self.execution_cancelled
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }

    fn lcd_dimensions(&self) -> (u32, u32) {
        (176, 208)
    }

    fn lcd_non_fullscreen_dimensions(&self) -> (u32, u32) {
        (176, 180)
    }

    fn lcd_ui_font_height(&self, size: i32) -> Option<i32> {
        (size == 8).then_some(13)
    }

    fn write_console_output(&mut self, text: &str, newline: bool) -> Result<(), EmuError> {
        self.console_output.push_str(text);
        if newline {
            self.console_output.push('\n');
        }
        Ok(())
    }
}

#[test]
fn native_context_reports_first_caller_outside_native_class() {
    let mut state = NativeContextTestState::new(Heap::new(1_024));
    let guest = runtime_method(
        "fixture/ui/Splash",
        "show",
        "()V",
        &[0xb1],
        0,
        0,
        vec![],
        true,
    );
    let wrapper = runtime_method(
        "javax/microedition/lcdui/Image",
        "createImage",
        "(Ljava/lang/String;)Ljavax/microedition/lcdui/Image;",
        &[0x01, 0xb0],
        1,
        1,
        vec![],
        true,
    );
    let native = runtime_method(
        "javax/microedition/lcdui/Image",
        "readResource",
        "(Ljava/lang/String;)[B",
        &[],
        1,
        1,
        vec![],
        true,
    );
    state.execution.call_stack = vec![
        guest.active_stack_frame(0),
        wrapper.active_stack_frame(0),
        native.active_stack_frame(0),
    ];
    let mut host = DefaultNativeContext;
    let context = state.context(&mut host, Vec::new());

    assert_eq!(
        context.native_caller_class().as_deref(),
        Some("fixture/ui/Splash")
    );
}

#[test]
fn native_entry_dispatches_by_complete_signature() {
    let cases = [
        ("N", "answer", "()I", Value::Int(42)),
        ("N", "answer", "()J", Value::Long(43)),
        ("N", "other", "()I", Value::Int(44)),
        ("M", "answer", "()I", Value::Int(45)),
    ];
    let mut program = Program::new();
    for (class, name, descriptor, value) in cases {
        let mut native = runtime_method(class, name, descriptor, &[], 0, 0, vec![None], true);
        native.is_native = true;
        program.methods.insert(native.key.clone(), native);
        program
            .native_registry_mut()
            .register(
                NativeSignature::new(class, name, descriptor),
                move |_, args| {
                    assert!(args.is_empty());
                    Ok(Some(value_to_native(value)))
                },
            )
            .unwrap();
    }
    for (class, name, descriptor, value) in cases {
        assert_eq!(
            program
                .execute(class, name, descriptor, Limits::default(), false)
                .unwrap()
                .value,
            Some(value)
        );
    }
}

#[test]
fn native_invocation_forwards_the_complete_host_surface() {
    let mut native = runtime_method("N", "host", "()I", &[], 0, 0, vec![None], true);
    native.is_native = true;
    let mut program = Program::new();
    program.methods.insert(native.key.clone(), native);
    program
        .native_registry_mut()
        .register(NativeSignature::new("N", "host", "()I"), |context, args| {
            assert!(args.is_empty());
            assert_eq!(context.display_colors(), 65_536);
            assert!(context.canvas_pointer_events());
            assert!(context.canvas_pointer_motion_events());
            assert_eq!(context.lcd_dimensions(), (176, 208));
            assert_eq!(context.lcd_non_fullscreen_dimensions(), (176, 180));
            assert_eq!(context.lcd_ui_font_height(8), Some(13));
            context.set_text_input_active(true);
            context.write_console_output("forwarded", true)?;
            Ok(Some(NativeValue::Int(7)))
        })
        .unwrap();
    let mut context = ForwardedHostContext::default();

    let execution = program
        .execute_with_context("N", "host", "()I", Limits::default(), false, &mut context)
        .unwrap();

    assert_eq!(execution.value, Some(Value::Int(7)));
    assert!(context.text_input_active);
    assert_eq!(context.console_output, "forwarded\n");
}

#[test]
fn native_collection_keeps_arguments_and_live_runtime_roots() {
    let mut native = runtime_method("N", "collect", "([I)I", &[], 0, 1, vec![None], true);
    native.is_native = true;
    let mut program = Program::new();
    program
        .native_registry_mut()
        .register(
            NativeSignature::new("N", "collect", "([I)I"),
            |context, args| {
                let NativeValue::Reference(Some(argument)) = args[0] else {
                    panic!("array argument")
                };
                // Six small arrays fit initially; this allocation requires GC.
                context.allocate_java_int_array(&[0; 180])?;
                assert_eq!(context.read_java_int_array(argument)?.len(), 8);
                Ok(Some(NativeValue::Int(1)))
            },
        )
        .unwrap();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 1024,
            ..Limits::default()
        },
        false,
        &mut host,
    );
    let handles = (0..6)
        .map(|_| {
            machine
                .heap
                .managed
                .allocate_array(ArrayKind::Int, 8)
                .unwrap()
        })
        .collect::<Vec<_>>();
    machine.heap.frame_roots.at_depth(1).push(handles[1]);
    machine
        .classes
        .static_fields
        .insert("static-root".into(), Value::Reference(Some(handles[2])));
    machine
        .scheduler
        .thread_states
        .insert(handles[3], ThreadState::Running);
    machine.jsr239.target = Some(handles[4]);
    let result = machine
        .call_native_inner(&native, &[Value::Reference(Some(handles[0]))], 2, None)
        .unwrap();
    assert!(matches!(result, CallOutcome::Return(Some(Value::Int(1)))));
    for handle in &handles[..5] {
        assert!(machine.heap.managed.get(*handle).is_ok());
    }
    assert!(machine.heap.managed.get(handles[5]).is_err());
}

#[test]
fn native_collection_releases_removed_runtime_roots() {
    let mut heap = Heap::new(64);
    let live = heap
        .allocate_object("java/lang/Object", HashMap::new())
        .unwrap();
    let garbage = heap
        .allocate_object("java/lang/Object", HashMap::new())
        .unwrap();
    let mut state = NativeContextTestState::new(heap);
    state
        .classes
        .static_fields
        .insert("static-root".into(), Value::Reference(Some(live)));
    let mut host = DefaultNativeContext;
    let mut context = state.context(&mut host, Vec::new());

    let first = Handle::from_raw(context.allocate_java_byte_array(&[0; 32]).unwrap());
    assert!(context.heap.managed.get(live).is_ok());
    assert!(context.heap.managed.get(garbage).is_err());

    context
        .classes
        .static_fields
        .insert("static-root".into(), Value::Reference(None));
    let second = context.allocate_java_byte_array(&[7; 40]).unwrap();
    assert!(context.heap.managed.get(live).is_err());
    assert!(context.heap.managed.get(first).is_err());
    assert_eq!(context.read_java_byte_array(second).unwrap(), [7; 40]);
}

#[test]
fn native_argument_buffers_preserve_types_at_the_inline_boundary() {
    let values = [
        Value::Int(i32::MIN),
        Value::Long(i64::MAX),
        Value::Float(f32::from_bits(0x7fc0_1234)),
        Value::Double(-0.0),
        Value::Reference(None),
        Value::Int(6),
        Value::Int(7),
        Value::Int(8),
        Value::Int(9),
    ];
    for (count, descriptor) in [
        (8, "(IJFDLjava/lang/Object;III)I"),
        (9, "(IJFDLjava/lang/Object;IIII)I"),
    ] {
        let mut native = runtime_method("N", "arguments", descriptor, &[], 0, 11, vec![None], true);
        native.is_native = true;
        let mut program = Program::new();
        program
            .native_registry_mut()
            .register(
                NativeSignature::new("N", "arguments", descriptor),
                move |_, args| {
                    assert_eq!(args.len(), count);
                    assert_eq!(args[0], NativeValue::Int(i32::MIN));
                    assert_eq!(args[1], NativeValue::Long(i64::MAX));
                    let NativeValue::Float(float) = args[2] else {
                        panic!("float argument")
                    };
                    let NativeValue::Double(double) = args[3] else {
                        panic!("double argument")
                    };
                    assert_eq!(float.to_bits(), 0x7fc0_1234);
                    assert_eq!(double.to_bits(), (-0.0_f64).to_bits());
                    assert_eq!(args[4], NativeValue::Reference(None));
                    for (index, value) in args.iter().enumerate().skip(5) {
                        assert_eq!(*value, NativeValue::Int(i32::try_from(index + 1).unwrap()));
                    }
                    Ok(Some(NativeValue::Int(1)))
                },
            )
            .unwrap();
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let result = machine
            .call_native_inner(&native, &values[..count], 1, None)
            .unwrap();
        assert!(matches!(result, CallOutcome::Return(Some(Value::Int(1)))));
    }
}

#[test]
fn host_cancellation_is_forwarded_to_the_interpreter() {
    let m = method(&[0xa7, 0, 0], 0, 0);
    let mut program = Program::new();
    program.methods.insert(m.key.clone(), m);
    let mut context = ForwardedHostContext {
        execution_cancelled: true,
        ..ForwardedHostContext::default()
    };
    let error = program
        .execute_with_context(
            "T",
            "main",
            "()I",
            Limits {
                max_instructions: 10_000,
                ..Limits::default()
            },
            false,
            &mut context,
        )
        .unwrap_err();
    assert_eq!(error.code(), "execution-cancelled");
}
