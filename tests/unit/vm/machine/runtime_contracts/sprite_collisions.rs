use super::*;
use std::{hint::black_box, time::Instant};

const SPRITE: &str = "javax/microedition/lcdui/game/Sprite";
const SPRITE_COLLISION: &str = "(Ljavax/microedition/lcdui/game/Sprite;Z)Z";

fn invoke(
    machine: &mut Machine<'_, '_>,
    receiver: Handle,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> CallOutcome {
    let method = &machine.program.methods[&MethodKey {
        class: if matches!(name, "setPosition" | "setVisible") {
            "javax/microedition/lcdui/game/Layer"
        } else {
            SPRITE
        }
        .into(),
        name: name.into(),
        descriptor: descriptor.into(),
    }];
    machine
        .call(
            method,
            std::iter::once(Value::Reference(Some(receiver)))
                .chain(args.iter().copied())
                .collect::<Vec<_>>(),
            1,
        )
        .unwrap()
}

fn sprite(machine: &mut Machine<'_, '_>, image: Handle, width: i32, height: i32) -> Handle {
    let receiver = machine
        .allocate_native_instance(SPRITE, &[Value::Reference(Some(image))])
        .unwrap();
    assert!(matches!(
        invoke(
            machine,
            receiver,
            "<init>",
            "(Ljavax/microedition/lcdui/Image;II)V",
            &[
                Value::Reference(Some(image)),
                Value::Int(width),
                Value::Int(height),
            ],
        ),
        CallOutcome::Return(None)
    ));
    receiver
}

fn set(machine: &mut Machine<'_, '_>, receiver: Handle, name: &str, values: &[i32]) {
    let descriptor = format!("({})V", "I".repeat(values.len()));
    assert!(matches!(
        invoke(
            machine,
            receiver,
            name,
            &descriptor,
            &values.iter().copied().map(Value::Int).collect::<Vec<_>>(),
        ),
        CallOutcome::Return(None)
    ));
}

fn collides(machine: &mut Machine<'_, '_>, a: Handle, b: Handle, pixels: bool) -> bool {
    let CallOutcome::Return(Some(Value::Int(result))) = invoke(
        machine,
        a,
        "collidesWith",
        SPRITE_COLLISION,
        &[Value::Reference(Some(b)), Value::Int(i32::from(pixels))],
    ) else {
        panic!("collision did not return a boolean");
    };
    result != 0
}

// Reference coordinates are constructed from forward-transformed individual
// pixels, independently of the runtime's inverse sampling and interval math.
fn forward(x: i64, y: i64, width: i64, height: i64, transform: i32) -> (i64, i64) {
    match transform {
        1 => (x, height - 1 - y),
        2 => (width - 1 - x, y),
        3 => (width - 1 - x, height - 1 - y),
        4 => (y, x),
        5 => (height - 1 - y, x),
        6 => (y, width - 1 - x),
        7 => (height - 1 - y, width - 1 - x),
        _ => (x, y),
    }
}

fn points(
    image: &[i32],
    frame: i32,
    transform: i32,
    position: [i32; 2],
    rectangle: [i32; 4],
    pixels: bool,
) -> Vec<(i64, i64)> {
    let [left, top, width, height] = rectangle;
    let mut points = Vec::new();
    for y in top..top + height {
        for x in left..left + width {
            if pixels {
                if !(0..3).contains(&x) || !(0..2).contains(&y) {
                    continue;
                }
                let offset = (frame / 2 * 2 + y) * 6 + frame % 2 * 3 + x;
                if image[offset as usize].cast_unsigned() >> 24 == 0 {
                    continue;
                }
            }
            let (x, y) = forward(i64::from(x), i64::from(y), 3, 2, transform);
            points.push((x + i64::from(position[0]), y + i64::from(position[1])));
        }
    }
    points
}

#[test]
fn sprite_collisions_match_pixels_for_all_transforms_frames_and_rectangles() {
    let program = program_with_bootstrap();
    for storage in ["argb", "indexed", "mutable"] {
        let pixels: Vec<i32> = (0..24)
            .map(|i| {
                ([0_u32, 0xff00_0000, 0x0100_0000, 0x7f00_0000][i % 4]
                    | if storage == "indexed" { 0 } else { i as u32 })
                .cast_signed()
            })
            .collect();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_instructions: u64::MAX,
                ..Limits::default()
            },
            false,
            &mut context,
        );
        let image = machine
            .allocate_image(6, 4, pixels.clone(), storage == "mutable", &[])
            .unwrap();
        let a = sprite(&mut machine, image, 3, 2);
        let b = sprite(&mut machine, image, 3, 2);
        let sequence = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 2)
            .unwrap();
        machine
            .heap
            .managed
            .array_set(sequence, 0, HeapValue::Int(3))
            .unwrap();
        machine
            .heap
            .managed
            .array_set(sequence, 1, HeapValue::Int(1))
            .unwrap();
        invoke(
            &mut machine,
            b,
            "setFrameSequence",
            "([I)V",
            &[Value::Reference(Some(sequence))],
        );
        for transform_a in 0..8 {
            set(&mut machine, a, "setTransform", &[transform_a]);
            set(&mut machine, a, "setFrame", &[transform_a % 4]);
            for transform_b in 0..8 {
                set(&mut machine, b, "setTransform", &[transform_b]);
                set(&mut machine, b, "setFrame", &[transform_b % 2]);
                for base in [[0, 0], [i32::MIN, i32::MAX - 4], [i32::MAX - 4, i32::MIN]] {
                    set(&mut machine, a, "setPosition", &base);
                    let other = [base[0] + 2, base[1] + 1];
                    set(&mut machine, b, "setPosition", &other);
                    for rectangle in [[0, 0, 3, 2], [-2, -1, 7, 5], [1, 0, 1, 2], [0, 0, 0, 2]] {
                        set(&mut machine, a, "defineCollisionRectangle", &rectangle);
                        for pixel_level in [false, true] {
                            let first = points(
                                &pixels,
                                transform_a % 4,
                                transform_a,
                                base,
                                rectangle,
                                pixel_level,
                            );
                            let second = points(
                                &pixels,
                                if transform_b % 2 == 0 { 3 } else { 1 },
                                transform_b,
                                other,
                                [0, 0, 3, 2],
                                pixel_level,
                            );
                            let expected = first.iter().any(|point| second.contains(point));
                            assert_eq!(
                                collides(&mut machine, a, b, pixel_level),
                                expected,
                                "storage={storage} transforms={transform_a}/{transform_b} base={base:?} rectangle={rectangle:?} pixel={pixel_level}"
                            );
                            assert_eq!(collides(&mut machine, b, a, pixel_level), expected);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn sprite_collision_uses_current_mutable_pixels_without_guest_allocations() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let image = machine
        .allocate_image(4, 4, vec![0; 16], true, &[])
        .unwrap();
    let a = sprite(&mut machine, image, 4, 4);
    let b = sprite(&mut machine, image, 4, 4);
    let pixels = machine
        .graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
        .unwrap();
    for alpha in [0, 1, 127, 255, 0] {
        machine
            .heap
            .managed
            .array_set(
                pixels,
                15,
                HeapValue::Int((alpha as u32 * 0x0100_0000).cast_signed()),
            )
            .unwrap();
        let allocations = machine.heap.managed.len();
        assert_eq!(collides(&mut machine, a, b, true), alpha != 0);
        assert!(collides(&mut machine, a, b, false));
        assert_eq!(machine.heap.managed.len(), allocations);
        let outcome = invoke(
            &mut machine,
            a,
            "collidesWith",
            "(Ljavax/microedition/lcdui/Image;IIZ)Z",
            &[
                Value::Reference(Some(image)),
                Value::Int(0),
                Value::Int(0),
                Value::Int(1),
            ],
        );
        assert!(
            matches!(outcome, CallOutcome::Return(Some(Value::Int(value))) if value == i32::from(alpha != 0))
        );
    }
    invoke(&mut machine, a, "setVisible", "(Z)V", &[Value::Int(0)]);
    assert!(!collides(&mut machine, a, b, false));
    let CallOutcome::Throw(exception) = invoke(
        &mut machine,
        a,
        "collidesWith",
        SPRITE_COLLISION,
        &[Value::Reference(None), Value::Int(0)],
    ) else {
        panic!("null collision target must throw even when the sprite is hidden");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/NullPointerException"
    );
}

#[test]
fn sprite_pixel_collision_clips_huge_rectangles_to_frame_bounds() {
    let program = program_with_bootstrap();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let image = machine
        .allocate_image(3, 2, vec![-1; 6], false, &[])
        .unwrap();
    let a = sprite(&mut machine, image, 3, 2);
    let b = sprite(&mut machine, image, 3, 2);
    for transform in 0..8 {
        for s in [a, b] {
            set(&mut machine, s, "setTransform", &[transform]);
            set(
                &mut machine,
                s,
                "defineCollisionRectangle",
                &[-1_000_000_000, -1_000_000_000, i32::MAX, i32::MAX],
            );
        }
        set(&mut machine, a, "setPosition", &[0, 0]);
        set(&mut machine, b, "setPosition", &[0, 0]);
        assert!(collides(&mut machine, a, b, true));
        set(&mut machine, b, "setPosition", &[1000, 1000]);
        assert!(collides(&mut machine, a, b, false));
        assert!(!collides(&mut machine, a, b, true));
    }
}

#[test]
fn sprite_collision_preserves_image_roots_across_yields_and_collection() {
    let program = program_with_bootstrap();
    for quantum in [1, 4, 64] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let image = machine
            .allocate_image(3, 2, vec![-1; 6], false, &[])
            .unwrap();
        let a = sprite(&mut machine, image, 3, 2);
        let b = sprite(&mut machine, image, 3, 2);
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
        let mut outcome = invoke(
            &mut machine,
            a,
            "collidesWith",
            SPRITE_COLLISION,
            &[Value::Reference(Some(b)), Value::Int(1)],
        );
        let mut yields = 0;
        while let CallOutcome::Suspend(continuation) = outcome {
            assert!(yields < 1024);
            let mut roots = machine.roots(&[], &[]);
            roots.extend([a, b]);
            continuation.roots(&mut roots);
            machine.collect_heap(roots);
            machine.scheduler.quantum_remaining = quantum;
            outcome = machine.resume_suspended_call(continuation, 1).unwrap();
            yields += 1;
        }
        assert!(matches!(outcome, CallOutcome::Return(Some(Value::Int(1)))));
        if quantum == 1 {
            assert!(yields > 0);
        }
    }
}

#[test]
fn sprite_collision_cancels_during_transparent_pixel_scan() {
    use std::{cell::Cell, rc::Rc};
    let program = program_with_bootstrap();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let image = machine
        .allocate_image(128, 128, vec![0; 128 * 128], false, &[])
        .unwrap();
    let a = sprite(&mut machine, image, 128, 128);
    let b = sprite(&mut machine, image, 128, 128);
    checks.set(0);
    cancel_at.set(3);
    let method = &program.methods[&MethodKey {
        class: SPRITE.into(),
        name: "collidesWith".into(),
        descriptor: SPRITE_COLLISION.into(),
    }];
    let outcome = machine.call(
        method,
        [
            Value::Reference(Some(a)),
            Value::Reference(Some(b)),
            Value::Int(1),
        ],
        1,
    );
    let Err(error) = outcome else {
        panic!("pixel scan ignored cancellation");
    };
    assert_eq!(error.code(), "execution-cancelled");
    assert_eq!(checks.get(), 3);
    cancel_at.set(usize::MAX);
    assert!(!collides(&mut machine, a, b, true));
}

#[test]
#[ignore = "manual release throughput measurement for Sprite collisions"]
fn sprite_collision_throughput() {
    let program = program_with_bootstrap();
    for size in [4, 64] {
        for case in ["bounds", "first", "last", "transparent"] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(
                Limits {
                    max_instructions: u64::MAX,
                    ..Limits::default()
                },
                false,
                &mut context,
            );
            let mut pixels = vec![0; (size * size) as usize];
            if case == "first" {
                pixels[0] = -1;
            }
            if case == "last" {
                *pixels.last_mut().unwrap() = -1;
            }
            let image = machine
                .allocate_image(size, size, pixels, false, &[])
                .unwrap();
            let a = sprite(&mut machine, image, size, size);
            let b = sprite(&mut machine, image, size, size);
            let iterations = if size == 4 { 512 } else { 32 };
            let started = Instant::now();
            let mut hits = 0;
            for _ in 0..iterations {
                hits += i32::from(collides(
                    &mut machine,
                    black_box(a),
                    black_box(b),
                    case != "bounds",
                ));
            }
            eprintln!(
                "sprite-collision size={size} case={case} ns={} hits={hits}",
                started.elapsed().as_nanos()
            );
            assert_eq!(hits, if case == "transparent" { 0 } else { iterations });
        }
    }
}
