use super::*;

#[test]
fn hebrew_words_numbers_and_brackets_use_visual_order() {
    for (logical, visual) in [
        ("שלום", "םולש"),
        ("שלום עולם", "םלוע םולש"),
        ("שלום 123", "123 םולש"),
        ("ABC שלום 123", "ABC 123 םולש"),
        ("שלום (ABC)", "(ABC) םולש"),
        ("(שלום)", "(םולש)"),
        ("שלום [123]", "[123] םולש"),
    ] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        for size in [8, 0, 16] {
            let (_, advance) = lcd_ui_font_dimensions(size);
            let (_, reference, reference_pixels) =
                unicode_graphics_target(&mut machine, size, 240, 32);
            for (index, unit) in visual.encode_utf16().enumerate() {
                machine
                    .graphics_unicode_draw_text(
                        reference,
                        TextSource::Utf16(&[unit]),
                        4 + index as i32 * advance,
                        3,
                        20,
                    )
                    .unwrap();
            }
            let expected = machine
                .graphics_int_array_snapshot(reference_pixels)
                .unwrap();
            let text = logical.encode_utf16().collect::<Vec<_>>();
            let width = text.len() as i32 * advance;
            let string = machine.intern_string_units(text.clone(), &[], &[]).unwrap();
            for anchor in [0, 20, 17, 24] {
                let x = 4 + match anchor {
                    17 => width / 2,
                    24 => width,
                    _ => 0,
                };
                for source in [TextSource::Utf16(&text), TextSource::JavaString(string)] {
                    let (_, graphics, pixels) =
                        unicode_graphics_target(&mut machine, size, 240, 32);
                    machine
                        .graphics_unicode_draw_text(graphics, source, x, 3, anchor)
                        .unwrap();
                    assert_eq!(
                        machine.graphics_int_array_snapshot(pixels).unwrap(),
                        expected,
                        "{logical:?}, size={size}, anchor={anchor}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn bidi_text_throughput() {
    let program = Program::new();
    for sample in ["שלום", "مرحبا", "ABC שלום 123"] {
        for repeats in [1, 256] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let (_, graphics, pixels) = unicode_graphics_target(&mut machine, 0, 240, 20);
            let text = sample.repeat(repeats).encode_utf16().collect::<Vec<_>>();
            let string = machine.intern_string_units(text, &[], &[]).unwrap();
            let started = std::time::Instant::now();
            for _ in 0..1_024 {
                machine
                    .graphics_unicode_draw_text(
                        graphics,
                        TextSource::JavaString(std::hint::black_box(string)),
                        0,
                        0,
                        20,
                    )
                    .unwrap();
            }
            let elapsed = started.elapsed();
            let pixels = machine.graphics_int_array_snapshot(pixels).unwrap();
            let checksum = pixels.iter().fold(0_u64, |sum, pixel| {
                sum.rotate_left(1) ^ u64::from(pixel.cast_unsigned())
            });
            eprintln!(
                "bidi text={sample:?} repeats={repeats} elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}
