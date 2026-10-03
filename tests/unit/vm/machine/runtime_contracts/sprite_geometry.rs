use super::*;
use std::{hint::black_box, time::Instant};

fn rectangle_method(program: &Program) -> &Method {
    &program.methods[&MethodKey {
        class: "javax/microedition/lcdui/game/Sprite".into(),
        name: "rectFor".into(),
        descriptor: "(IIIIIIIII)[J".into(),
    }]
}

fn rectangle_result(machine: &Machine<'_, '_>, outcome: &CallOutcome) -> [i64; 4] {
    let CallOutcome::Return(Some(Value::Reference(Some(result)))) = outcome else {
        panic!("Sprite rectangle must return its four bounds");
    };
    assert_eq!(machine.heap.managed.array_length(*result).unwrap(), 4);
    std::array::from_fn(|index| {
        let Value::Long(value) = machine
            .heap
            .managed
            .array_get(*result, i32::try_from(index).unwrap())
            .unwrap()
        else {
            panic!("rectangle bound must be a long");
        };
        value
    })
}

// Independently transform all four pixel corners, then enclose them in
// half-open world bounds. Keep Java's int subtraction for the frame edges.
fn corner_bounds(args: [i32; 9]) -> [i64; 4] {
    let [
        world_x,
        world_y,
        x,
        y,
        width,
        height,
        frame_width,
        frame_height,
        transform,
    ] = args;
    let right = i64::from(frame_width.wrapping_sub(1));
    let bottom = i64::from(frame_height.wrapping_sub(1));
    let x = i64::from(x);
    let y = i64::from(y);
    let corners = [
        (x, y),
        (x + i64::from(width) - 1, y),
        (x, y + i64::from(height) - 1),
        (x + i64::from(width) - 1, y + i64::from(height) - 1),
    ]
    .map(|(x, y)| match transform {
        1 => (x, bottom - y),
        2 => (right - x, y),
        3 => (right - x, bottom - y),
        4 => (y, x),
        5 => (bottom - y, x),
        6 => (y, right - x),
        7 => (bottom - y, right - x),
        _ => (x, y),
    });
    [
        i64::from(world_x) + corners.iter().map(|p| p.0).min().unwrap(),
        i64::from(world_y) + corners.iter().map(|p| p.1).min().unwrap(),
        i64::from(world_x) + corners.iter().map(|p| p.0).max().unwrap() + 1,
        i64::from(world_y) + corners.iter().map(|p| p.1).max().unwrap() + 1,
    ]
}

#[test]
fn sprite_rectangle_matches_corner_geometry_at_transform_and_integer_boundaries() {
    let program = program_with_bootstrap();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for input in [
        [20, -30, 2, 3, 7, 5, 16, 24, 0],
        [-20, 30, -7, -5, 31, 45, 16, 24, 0],
        [0, 0, 0, 0, 0, 0, 1, 1, 0],
        [0, 0, 1, 2, 1, 0, 16, 24, 0],
        [3, 4, 10, 20, -7, -2, 16, 24, 0],
        [
            i32::MAX,
            i32::MIN,
            i32::MIN,
            i32::MAX,
            i32::MAX,
            i32::MIN,
            i32::MIN,
            i32::MAX,
            0,
        ],
        [
            i32::MIN,
            i32::MAX,
            i32::MAX,
            i32::MIN,
            i32::MIN,
            i32::MAX,
            i32::MAX,
            i32::MIN,
            0,
        ],
    ] {
        for transform in [-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, i32::MIN, i32::MAX] {
            let mut args = input;
            args[8] = transform;
            let allocations = machine.heap.managed.len();
            let outcome = machine
                .call(rectangle_method(&program), args.map(Value::Int), 1)
                .unwrap();
            assert_eq!(
                rectangle_result(&machine, &outcome),
                corner_bounds(args),
                "{args:?}"
            );
            assert_eq!(machine.heap.managed.len(), allocations + 1);
        }
    }
}

#[test]
fn sprite_rectangle_preserves_bounds_across_yields_and_collection() {
    let program = program_with_bootstrap();
    for transform in 0..8 {
        for quantum in [0, 1, 4, 64] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let worker = machine
                .heap
                .managed
                .allocate_object("java/lang/Thread", HashMap::new())
                .unwrap();
            machine.scheduler.current_thread = worker.to_raw();
            machine
                .scheduler
                .thread_states
                .insert(worker, ThreadState::Running);
            machine.scheduler.quantum_remaining = quantum;
            let args = [100, -200, -7, 11, 13, 17, 32, 24, transform];
            let mut outcome = machine
                .call(rectangle_method(&program), args.map(Value::Int), 1)
                .unwrap();
            let mut yields = 0;
            while let CallOutcome::Suspend(continuation) = outcome {
                assert!(yields < 4096, "rectangle failed to make progress");
                let mut roots = machine.roots(&[], &[]);
                continuation.roots(&mut roots);
                machine.collect_heap(roots);
                machine.scheduler.quantum_remaining = quantum.max(1);
                outcome = machine.resume_suspended_call(continuation, 1).unwrap();
                yields += 1;
            }
            if quantum <= 1 {
                assert!(yields > 0);
            }
            assert_eq!(rectangle_result(&machine, &outcome), corner_bounds(args));
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement for Sprite collision bounds"]
fn sprite_rectangle_throughput() {
    let program = program_with_bootstrap();
    for transform in 0..8 {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let args = [100, -200, -7, 11, 13, 17, 32, 24, transform];
        let expected = corner_bounds(args);
        let method = rectangle_method(&program);
        let started = Instant::now();
        let mut sum = [0; 4];
        for _ in 0..1024 {
            let outcome = machine
                .call(method, black_box(args).map(Value::Int), 1)
                .unwrap();
            let bounds = rectangle_result(&machine, &outcome);
            for (sum, value) in sum.iter_mut().zip(bounds) {
                *sum += value;
            }
        }
        let elapsed = started.elapsed();
        assert_eq!(sum, expected.map(|value| value * 1024));
        eprintln!("sprite-rectangle-{transform} elapsed={elapsed:?} sum={sum:?}");
    }
}
