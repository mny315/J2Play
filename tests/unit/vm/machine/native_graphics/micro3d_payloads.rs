use super::graphics_support::direct_graphics_target;
use super::*;

pub(super) fn primitive_args(machine: &mut Machine<'_, '_>, capacity: i32) -> [Value; 12] {
    let (graphics, _) = direct_graphics_target(machine, 8, 8);
    machine.micro3d.target = Some(graphics);
    machine.micro3d.target_scissor = [0, 0, 8, 8];
    machine.micro3d.command_scissor = [0, 0, 8, 8];
    let layout = machine
        .heap
        .managed
        .allocate_object("com/mascotcapsule/micro3d/v3/FigureLayout", HashMap::new())
        .unwrap();
    machine
        .micro3d
        .runtime
        .create(
            layout.to_raw(),
            micro3d::ObjectKind::Layout(micro3d::FigureLayoutState {
                center: [4, 4],
                scale: [4096, 4096],
                ..micro3d::FigureLayoutState::default()
            }),
        )
        .unwrap();
    let effect = machine
        .heap
        .managed
        .allocate_object("com/mascotcapsule/micro3d/v3/Effect3D", HashMap::new())
        .unwrap();
    machine
        .micro3d
        .runtime
        .create(
            effect.to_raw(),
            micro3d::ObjectKind::Effect(micro3d::EffectState::default()),
        )
        .unwrap();
    let arrays: [Handle; 4] = std::array::from_fn(|_| {
        machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, capacity)
            .unwrap()
    });
    machine
        .m3g_write_int_array(arrays[0], &[-2, -2, 0, 2, -2, 0, 0, 2, 0])
        .unwrap();
    [
        Value::Reference(Some(graphics)),
        Value::Reference(None),
        Value::Int(0),
        Value::Int(0),
        Value::Reference(Some(layout)),
        Value::Reference(Some(effect)),
        Value::Int(0x0300_0000),
        Value::Int(1),
        Value::Reference(Some(arrays[0])),
        Value::Reference(Some(arrays[1])),
        Value::Reference(Some(arrays[2])),
        Value::Reference(Some(arrays[3])),
    ]
}

fn draw(machine: &mut Machine<'_, '_>, args: &[Value; 12]) -> Result<CallOutcome, EmuError> {
    machine.micro3d_graphics_call(
        reference_argument(args, 0).unwrap(),
        "renderPrimitives",
        "",
        args,
    )
}

#[test]
fn primitive_arrays_ignore_unused_tails_and_still_reject_short_payloads() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let args = primitive_args(&mut machine, 32);
    for (argument, first_unused) in [(8, 9), (9, 0), (10, 0), (11, 0)] {
        let array = reference_argument(&args, argument).unwrap();
        machine.graphics_int_array_mut(array).unwrap()[first_unused..]
            .fill(HeapValue::Float(f32::NAN));
    }
    draw(&mut machine, &args).unwrap();
    assert!(machine.micro3d.runtime.target_pixels().contains(&u32::MAX));
    let before = machine.micro3d.runtime.target_pixels().to_vec();
    for length in [0, 8] {
        let mut short = args;
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, length)
            .unwrap();
        short[8] = Value::Reference(Some(array));
        assert_eq!(
            draw(&mut machine, &short).err().unwrap().code(),
            "primitive-coordinates"
        );
        assert_eq!(machine.micro3d.runtime.target_pixels(), before);
    }
    for (command, argument, length, code) in [
        (0x0300_0300, 9, 8, "primitive-normals"),
        (0x0300_3000, 10, 5, "primitive-uv"),
        (0x0300_0400, 11, 0, "primitive-color"),
    ] {
        let mut short = args;
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, length)
            .unwrap();
        short[6] = Value::Int(command);
        short[argument] = Value::Reference(Some(array));
        assert_eq!(draw(&mut machine, &short).err().unwrap().code(), code);
        assert_eq!(machine.micro3d.runtime.target_pixels(), before);
    }
}

#[test]
fn command_lists_observe_stop_between_commands_without_throwing_a_guest_exception() {
    use std::cell::Cell;
    use std::rc::Rc;

    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let program = program_with_exception("java/lang/IllegalArgumentException");
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let primitives = primitive_args(&mut machine, 9);
    let commands = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 98)
        .unwrap();
    let mut words = vec![-33_554_431];
    words.extend(std::iter::repeat_n(0x8100_0000_u32.cast_signed(), 96));
    words.push(i32::MIN);
    machine.m3g_write_int_array(commands, &words).unwrap();
    let mut args = primitives[..6].to_vec();
    args.push(Value::Reference(Some(commands)));
    let receiver = reference_argument(&args, 0).unwrap();
    machine
        .micro3d_graphics_call(receiver, "drawCommandList", "", &args)
        .unwrap();
    let count = checks.get();
    assert!(count >= 98, "each command must observe cancellation");
    for at in 1..=count {
        checks.set(0);
        cancel_at.set(at);
        let error = machine
            .micro3d_graphics_call(receiver, "drawCommandList", "", &args)
            .err()
            .expect("Stop must leave native execution");
        assert_eq!(error.code(), "execution-cancelled");
        assert_eq!(checks.get(), at);
    }
    cancel_at.set(usize::MAX);
    machine
        .micro3d_graphics_call(receiver, "drawCommandList", "", &args)
        .unwrap();
}

#[test]
#[ignore = "manual primitive array copy throughput measurement"]
fn micro3d_primitive_payload_throughput() {
    for capacity in [9, 65_536] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_heap_bytes: 16 * 1024 * 1024,
                ..Limits::default()
            },
            false,
            &mut context,
        );
        let args = primitive_args(&mut machine, capacity);
        let started = std::time::Instant::now();
        for _ in 0..512 {
            draw(&mut machine, std::hint::black_box(&args)).unwrap();
        }
        eprintln!("capacity={capacity} elapsed={:?}", started.elapsed());
        assert!(machine.micro3d.runtime.target_pixels().contains(&u32::MAX));
    }
}

#[test]
fn command_lists_share_one_render_budget_across_primitives_and_flushes() {
    for (triangles, fragments) in [(1, 1024), (8, 40)] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                m3g_render: m3g::RenderLimits {
                    triangles,
                    fragments,
                    ..m3g::RenderLimits::default()
                },
                ..Limits::default()
            },
            false,
            &mut context,
        );
        let primitives = primitive_args(&mut machine, 9);
        let run_list = |machine: &mut Machine<'_, '_>, count| {
            let mut words = vec![-33_554_431];
            for _ in 0..count {
                words.extend([
                    0x0301_0000,
                    -2,
                    -2,
                    0,
                    2,
                    -2,
                    0,
                    0,
                    2,
                    0,
                    0x8200_0000_u32.cast_signed(),
                ]);
            }
            words.push(i32::MIN);
            let commands = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Int, i32::try_from(words.len()).unwrap())
                .unwrap();
            machine.m3g_write_int_array(commands, &words).unwrap();
            let mut args = primitives[..6].to_vec();
            args.push(Value::Reference(Some(commands)));
            machine.micro3d_draw_command_list(&args)
        };
        run_list(&mut machine, 1).unwrap();
        assert_eq!(
            run_list(&mut machine, 100).unwrap_err().code(),
            "render-budget"
        );
        assert_eq!(machine.micro3d.runtime.metrics().render_calls, 3);
        run_list(&mut machine, 1).unwrap();
    }
}

#[test]
fn primitive_tracing_preserves_rendering_and_collects_snapshots_only_on_request() {
    let program = Program::new();
    let mut results = Vec::new();
    for trace in [false, true] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), trace, &mut context);
        let args = primitive_args(&mut machine, 9);
        for _ in 0..256 {
            draw(&mut machine, &args).unwrap();
        }
        assert_eq!(machine.micro3d.render_diagnostics.is_empty(), !trace);
        assert!(machine.micro3d.render_diagnostics.len() <= 64);
        results.push((
            machine.micro3d.runtime.target_pixels().to_vec(),
            machine.micro3d.runtime.metrics(),
        ));
    }
    assert_eq!(results[0], results[1]);
}
