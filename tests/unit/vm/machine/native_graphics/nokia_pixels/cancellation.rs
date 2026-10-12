use std::cell::Cell;
use std::rc::Rc;

use super::*;

#[test]
fn nokia_draw_pixels_cancels_during_snapshot_and_painting() {
    check_transfer_cancellation(true);
}

#[test]
fn nokia_get_pixels_cancels_during_snapshot_and_conversion() {
    check_transfer_cancellation(false);
}

fn check_transfer_cancellation(draw: bool) {
    let program = Program::new();
    for (width, height, padding) in [
        (8192, 1, 0),
        (1, 8192, 0),
        (3, 2731, 0),
        (1, 8192, 1),
        (3, 2731, 1),
    ] {
        for alias in [false, true] {
            let checks = Rc::new(Cell::new(0));
            let cancel_at = Rc::new(Cell::new(usize::MAX));
            let mut context = CancellationContext {
                checks: Rc::clone(&checks),
                cancel_at: Rc::clone(&cancel_at),
            };
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let target_width = width + if draw { 0 } else { padding };
            let (graphics, pixels) = direct_graphics_target(&mut machine, target_width, height);
            let other = if alias {
                pixels
            } else {
                machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Int, width * height)
                    .unwrap()
            };
            let (source, destination) = if draw {
                (other, pixels)
            } else {
                (pixels, other)
            };
            let color = 0xff12_3456_u32.cast_signed();
            machine
                .graphics_int_array_mut(source)
                .unwrap()
                .fill(HeapValue::Int(color));
            let args = if draw {
                direct_pixel_args(graphics, other, [1, 0, width, 0, 0, width, height, 0, 8888])
            } else {
                get_pixels_args(graphics, other, [0, width, 0, 0, width, height, 8888])
            };
            let transfer = |machine: &mut Machine<'_, '_>| {
                if draw {
                    machine.graphics_nokia_draw_pixels(&args)
                } else {
                    machine.graphics_nokia_get_pixels(&args)
                }
            };
            transfer(&mut machine).unwrap();
            let total_checks = checks.get();
            assert!(
                total_checks > 4,
                "long pixel transfer did not poll cancellation"
            );
            assert!(total_checks <= 64, "narrow rows polled once per pixel");
            assert_eq!(
                machine.graphics_int_array_snapshot(destination).unwrap(),
                vec![color; machine.heap.managed.array_length(destination).unwrap()]
            );
            for point in [1, 2, total_checks / 2, total_checks] {
                let initial = if alias { color } else { 0 };
                machine
                    .graphics_int_array_mut(destination)
                    .unwrap()
                    .fill(HeapValue::Int(initial));
                checks.set(0);
                cancel_at.set(point);
                assert_eq!(
                    transfer(&mut machine).unwrap_err().code(),
                    "execution-cancelled"
                );
                assert_eq!(checks.get(), point);
                let output = machine.graphics_int_array_snapshot(destination).unwrap();
                if point <= 2 || alias {
                    assert!(output.iter().all(|&pixel| pixel == initial));
                } else if point == total_checks {
                    assert!(output.contains(&0) && output.contains(&color));
                }
            }
        }
    }
}
