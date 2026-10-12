use super::micro3d_affine::affine_object;
use super::micro3d_payloads::primitive_args;
use super::*;

const VERSION: i32 = -33_554_431;
const SELECT: i32 = 0x8700_0000_u32.cast_signed();
const FLUSH: i32 = 0x8200_0000_u32.cast_signed();
const TRIANGLE: [i32; 10] = [0x0301_0000, -2, -2, 0, 2, -2, 0, 0, 2, 0];

fn affines(machine: &mut Machine<'_, '_>, args: &[Value]) -> [Handle; 2] {
    let affines = std::array::from_fn(|index| {
        let affine = affine_object(machine);
        let mut matrix = micro3d::AffineTrans::IDENTITY;
        matrix.values[3] = index as i32 * 4 - 2;
        machine.micro3d_set_affine(affine, matrix).unwrap();
        affine
    });
    machine
        .micro3d
        .runtime
        .set_layout_affines(
            reference_argument(args, 4).unwrap().to_raw(),
            affines.map(Handle::to_raw).to_vec(),
        )
        .unwrap();
    affines
}

fn commands(machine: &mut Machine<'_, '_>, primitives: &[Value], words: &[i32]) -> Vec<Value> {
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, words.len() as i32)
        .unwrap();
    machine.m3g_write_int_array(array, words).unwrap();
    let mut args = primitives[..6].to_vec();
    args.push(Value::Reference(Some(array)));
    args
}

fn clear(machine: &mut Machine<'_, '_>, primitives: &[Value]) {
    let graphics = reference_argument(primitives, 0).unwrap();
    let (pixels, _, _) = machine.graphics_target(graphics).unwrap();
    machine
        .graphics_int_array_mut(pixels)
        .unwrap()
        .fill(HeapValue::Int(0));
    machine.micro3d.render_pending = false;
}

#[test]
fn command_matrices_follow_selection_and_guest_changes_between_lists() {
    let program = Program::new();
    let mut list_context = DefaultNativeContext;
    let mut direct_context = DefaultNativeContext;
    let mut list = program.machine(Limits::default(), false, &mut list_context);
    let mut direct = program.machine(Limits::default(), false, &mut direct_context);
    let list_primitives = primitive_args(&mut list, 9);
    let direct_primitives = primitive_args(&mut direct, 9);
    let list_affines = affines(&mut list, &list_primitives);
    let direct_affines = affines(&mut direct, &direct_primitives);
    let selections = [0, 0, 1, 1, 0, 1];
    let mut words = vec![VERSION];
    for (index, selected) in selections.into_iter().enumerate() {
        if index == 3 {
            words.push(FLUSH);
        }
        if index == 0 || selected != selections[index - 1] {
            words.push(SELECT | selected);
        }
        words.extend(TRIANGLE);
    }
    words.push(i32::MIN);
    let args = commands(&mut list, &list_primitives, &words);
    let mut previous_frame = None;
    for translation in [-2, 1] {
        for (machine, primitives, affine) in [
            (&mut list, &list_primitives, list_affines[0]),
            (&mut direct, &direct_primitives, direct_affines[0]),
        ] {
            let mut matrix = micro3d::AffineTrans::IDENTITY;
            matrix.values[3] = translation;
            machine.micro3d_set_affine(affine, matrix).unwrap();
            clear(machine, primitives);
        }
        list.micro3d_draw_command_list(&args).unwrap();
        for (index, selected) in selections.into_iter().enumerate() {
            if index == 3 {
                direct.micro3d_flush_target().unwrap();
            }
            direct
                .micro3d
                .runtime
                .select_layout_affine(
                    reference_argument(&direct_primitives, 4).unwrap().to_raw(),
                    selected as usize,
                )
                .unwrap();
            direct
                .micro3d_graphics_call(
                    reference_argument(&direct_primitives, 0).unwrap(),
                    "renderPrimitives",
                    "",
                    &direct_primitives,
                )
                .unwrap();
        }
        let frame = list.micro3d.runtime.target_pixels().to_vec();
        assert!(frame.contains(&u32::MAX));
        assert_eq!(frame, direct.micro3d.runtime.target_pixels());
        if let Some(previous) = previous_frame.replace(frame.clone()) {
            assert_ne!(frame, previous);
        }
        let layout = reference_argument(&list_primitives, 4).unwrap().to_raw();
        assert_eq!(
            list.micro3d
                .runtime
                .layout_render_snapshot(layout)
                .unwrap()
                .0
                .selected_affine,
            0
        );
    }
}

#[test]
fn command_matrices_are_read_only_when_a_primitive_uses_them() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let primitives = primitive_args(&mut machine, 9);
    let affines = affines(&mut machine, &primitives);
    machine
        .heap
        .managed
        .set_field(
            affines[1],
            "com/mascotcapsule/micro3d/v3/AffineTrans.m00:I",
            HeapValue::Float(0.0),
        )
        .unwrap();
    for mut words in [vec![VERSION], vec![VERSION, SELECT | 1]] {
        words.push(i32::MIN);
        let args = commands(&mut machine, &primitives, &words);
        machine.micro3d_draw_command_list(&args).unwrap();
    }
    for selected in [1, 2] {
        let mut words = vec![VERSION, SELECT | selected];
        words.extend(TRIANGLE);
        words.push(i32::MIN);
        let args = commands(&mut machine, &primitives, &words);
        assert!(machine.micro3d_draw_command_list(&args).is_err());
    }
}

#[test]
#[ignore = "manual release throughput comparison"]
fn micro3d_command_matrix_throughput() {
    use std::{hint::black_box, time::Instant};

    for matrices in [false, true] {
        for batches in [1, 16, 128] {
            for switch in [false, true] {
                if switch && !matrices {
                    continue;
                }
                let program = Program::new();
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let primitives = primitive_args(&mut machine, 9);
                if matrices {
                    affines(&mut machine, &primitives);
                }
                let mut words = vec![VERSION];
                for index in 0..batches {
                    if switch {
                        words.push(SELECT | (index % 2));
                    }
                    words.extend(TRIANGLE);
                }
                words.push(i32::MIN);
                let args = commands(&mut machine, &primitives, &words);
                let start = Instant::now();
                for _ in 0..16384 / batches {
                    machine.micro3d_draw_command_list(black_box(&args)).unwrap();
                }
                let elapsed = start.elapsed();
                let checksum = machine
                    .micro3d
                    .runtime
                    .target_pixels()
                    .iter()
                    .fold(0_u64, |sum, &pixel| {
                        sum.wrapping_mul(31).wrapping_add(u64::from(pixel))
                    });
                eprintln!(
                    "command-matrix-matrices={matrices}-batches={batches}-switch={switch}: elapsed={elapsed:?} checksum={checksum} calls={}",
                    machine.micro3d.runtime.metrics().render_calls
                );
            }
        }
    }
}
