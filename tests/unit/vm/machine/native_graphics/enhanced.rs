use super::graphics_support::direct_graphics_target;
use super::*;

mod image_copies;
mod image_regions;
mod shapes;

#[derive(Default)]
struct EnhancedHost {
    frame: Option<(u32, u32, u32, Vec<u32>)>,
    original: bool,
    cancellation: Option<CancellationContext>,
}

impl HostServices for EnhancedHost {
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
    fn render_3d_scale(&self) -> u32 {
        if self.original { 1 } else { 2 }
    }
    fn execution_cancelled(&self) -> bool {
        self.cancellation
            .as_ref()
            .is_some_and(HostServices::execution_cancelled)
    }
    fn present_lcdui_frame_scaled(
        &mut self,
        w: u32,
        h: u32,
        scale: u32,
        pixels: &[u32],
    ) -> Result<(), EmuError> {
        self.frame = Some((w, h, scale, pixels.to_vec()));
        Ok(())
    }
}

#[test]
#[ignore = "manual full-frame 3D and LCDUI throughput measurement"]
fn enhanced_frame_throughput() {
    measure_enhanced_frames(false);
}

#[test]
#[ignore = "manual full-frame throughput with interleaved guest array writes"]
fn enhanced_interleaved_heap_frame_throughput() {
    measure_enhanced_frames(true);
}

fn measure_enhanced_frames(interleaved_heap_writes: bool) {
    for scale in [1, 2] {
        for meshes in [1_u32, 24] {
            let program = Program::new();
            let mut host = EnhancedHost {
                original: scale == 1,
                ..EnhancedHost::default()
            };
            let mut machine = program.machine(
                Limits {
                    lcd_width: 240,
                    lcd_height: 320,
                    ..Limits::default()
                },
                false,
                &mut host,
            );
            let (_, graphics, pixels) =
                super::text::unicode_graphics_target(&mut machine, 0, 240, 320);
            bind(&mut machine, graphics);
            let scratch = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Int, 16)
                .unwrap();
            let text: Vec<u16> = "Score 12345 — уровень 2".encode_utf16().collect();
            for repeat in 0..3 {
                let started = std::time::Instant::now();
                for frame in 0_u32..60 {
                    machine.m3g_refresh_bound_target().unwrap();
                    machine.m3g.graphics.renderer.clear(Some(0xff10_2030), true);
                    machine.m3g_publish_bound_target().unwrap();
                    for mesh in 0..meshes {
                        if interleaved_heap_writes {
                            machine
                                .heap
                                .managed
                                .array_set(scratch, 0, HeapValue::Int((frame + mesh).cast_signed()))
                                .unwrap();
                        }
                        machine.m3g_refresh_bound_target().unwrap();
                        machine.m3g.graphics.renderer.reset_stats();
                        let x = -0.9 + (mesh % 6) as f32 * 0.3;
                        let y = -0.9 + (mesh / 6) as f32 * 0.4;
                        let extent = if meshes == 1 { 1.8 } else { 0.25 };
                        let color = 0xff90_4020 + frame * 13 + mesh * 17;
                        machine
                            .m3g
                            .graphics
                            .renderer
                            .draw_triangle([
                                m3g::Vertex::new(m3g::Vec4::new(x, y, 0.0, 1.0), color),
                                m3g::Vertex::new(m3g::Vec4::new(x + extent, y, 0.0, 1.0), color),
                                m3g::Vertex::new(m3g::Vec4::new(x, y + extent, 0.0, 1.0), color),
                            ])
                            .unwrap();
                        machine.m3g_publish_bound_target().unwrap();
                    }
                    machine
                        .graphics_fill_rect(&[
                            Value::Reference(Some(graphics)),
                            Value::Int(4),
                            Value::Int(4),
                            Value::Int(236),
                            Value::Int(40),
                            Value::Int(0x8040_6060_u32.cast_signed()),
                        ])
                        .unwrap();
                    machine
                        .graphics_unicode_draw_text(
                            graphics,
                            crate::machine::graphics_dispatch::TextSource::Utf16(&text),
                            8,
                            8,
                            20,
                        )
                        .unwrap();
                    if scale == 2 {
                        std::hint::black_box(machine.graphics.enhanced.pixels(pixels).unwrap());
                    }
                    std::hint::black_box(machine.graphics_int_array_snapshot(pixels).unwrap());
                }
                let elapsed = started.elapsed();
                let native = machine.graphics_int_array_snapshot(pixels).unwrap();
                let samples = machine
                    .graphics
                    .enhanced
                    .pixels(pixels)
                    .unwrap()
                    .unwrap_or_default();
                let checksum = native
                    .iter()
                    .map(|v| v.cast_unsigned())
                    .chain(samples)
                    .fold(0_u64, |sum, pixel| {
                        sum.wrapping_mul(31).wrapping_add(u64::from(pixel))
                    });
                eprintln!(
                    "frame scale={scale} meshes={meshes} repeat={repeat} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}

fn bind(machine: &mut Machine<'_, '_>, graphics: Handle) {
    let object = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Graphics3D", HashMap::new())
        .unwrap();
    machine
        .invoke_m3g_graphics3d_native(
            "javax/microedition/m3g/Graphics3D",
            "bindTarget",
            "(Ljava/lang/Object;)V",
            &[
                Value::Reference(Some(object)),
                Value::Reference(Some(graphics)),
            ],
        )
        .unwrap();
}

#[test]
fn high_resolution_m3g_keeps_geometry_samples_under_alpha_hud_and_presents_logical_canvas() {
    let program = Program::new();
    let mut host = EnhancedHost::default();
    let mut machine = program.machine(
        Limits {
            lcd_width: 8,
            lcd_height: 8,
            ..Limits::default()
        },
        false,
        &mut host,
    );
    let (graphics, pixels) = direct_graphics_target(&mut machine, 8, 8);
    bind(&mut machine, graphics);
    assert_eq!(machine.m3g.graphics.renderer.resolution_scale(), 2);
    assert_eq!(
        (
            machine.m3g.graphics.renderer.width(),
            machine.m3g.graphics.renderer.height()
        ),
        (8, 8)
    );
    machine.m3g.graphics.renderer.clear(Some(0xff00_0000), true);
    machine
        .m3g
        .graphics
        .renderer
        .draw_triangle([
            m3g::Vertex::new(m3g::Vec4::new(-1.0, -1.0, 0.0, 1.0), 0xffff_ffff),
            m3g::Vertex::new(m3g::Vec4::new(1.0, -1.0, 0.0, 1.0), 0xffff_ffff),
            m3g::Vertex::new(m3g::Vec4::new(-1.0, 1.0, 0.0, 1.0), 0xffff_ffff),
        ])
        .unwrap();
    machine.m3g_publish_bound_target().unwrap();
    let before = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
    assert!((0..8).any(|y| (0..8).any(|x| {
        [1, 16, 17]
            .iter()
            .any(|offset| before[y * 32 + x * 2] != before[y * 32 + x * 2 + offset])
    })));
    let original = machine.graphics_int_array_snapshot(pixels).unwrap();
    assert_eq!(original.len(), 64);
    machine
        .graphics_fill_rect(&[
            Value::Reference(Some(graphics)),
            Value::Int(1),
            Value::Int(2),
            Value::Int(7),
            Value::Int(5),
            Value::Int(0x8000_ff00_u32.cast_signed()),
        ])
        .unwrap();
    let after = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
    for y in 0..16 {
        for x in 0..16 {
            let index = y * 16 + x;
            let expected = if (2..14).contains(&x) && (4..10).contains(&y) {
                crate::machine::pixel_ops::blend_argb(
                    0x8000_ff00_u32.cast_signed(),
                    before[index].cast_signed(),
                )
                .cast_unsigned()
            } else {
                before[index]
            };
            assert_eq!(after[index], expected);
        }
    }
    let mut method = runtime_method(
        "javax/microedition/lcdui/Canvas",
        "present",
        "([I)V",
        &[],
        0,
        1,
        vec![None],
        true,
    );
    method.is_native = true;
    machine
        .schedule_frame_presentation(&method, &[Value::Reference(Some(pixels))])
        .unwrap();
    assert!(
        machine
            .java_frame_matches_presented(pixels, (8, 8))
            .unwrap()
    );
    machine
        .m3g
        .graphics
        .renderer
        .clear(Some(0xff00_0000), false);
    machine.m3g_publish_bound_target().unwrap();
    assert!(
        !machine
            .java_frame_matches_presented(pixels, (8, 8))
            .unwrap()
    );
    drop(machine);
    assert_eq!(host.frame, Some((8, 8, 2, after)));
}

#[test]
fn enhanced_offscreen_image_blits_rotate_samples_and_preserve_native_arrays() {
    for transform in 0..8 {
        let program = Program::new();
        let mut host = EnhancedHost::default();
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let (_source_graphics, source) = direct_graphics_target(&mut machine, 2, 2);
        let (destination_graphics, destination) = direct_graphics_target(&mut machine, 2, 2);
        assert!(machine.ensure_enhanced_target(source, 2, 2).unwrap());
        let samples: Vec<u32> = (0..16).map(|x| 0xff00_0000 | x).collect();
        machine
            .graphics
            .enhanced
            .store(source, &samples, [0, 0, 2, 2], false)
            .unwrap();
        let image = machine
            .heap
            .managed
            .allocate_object(
                "javax/microedition/lcdui/Image",
                HashMap::from([
                    (
                        "javax/microedition/lcdui/Image.width:I".into(),
                        HeapValue::Int(2),
                    ),
                    (
                        "javax/microedition/lcdui/Image.height:I".into(),
                        HeapValue::Int(2),
                    ),
                    (
                        "javax/microedition/lcdui/Image.pixels:[I".into(),
                        HeapValue::Reference(Some(source)),
                    ),
                    (
                        "javax/microedition/lcdui/Image.mutable:Z".into(),
                        HeapValue::Int(1),
                    ),
                ]),
            )
            .unwrap();
        let source_before = machine.graphics_int_array_snapshot(source).unwrap();
        machine
            .graphics_draw_region(&[
                Value::Reference(Some(destination_graphics)),
                Value::Reference(Some(image)),
                Value::Int(0),
                Value::Int(0),
                Value::Int(2),
                Value::Int(2),
                Value::Int(transform),
                Value::Int(0),
                Value::Int(0),
                Value::Int(2),
                Value::Int(2),
            ])
            .unwrap();
        let result = machine
            .graphics
            .enhanced
            .pixels(destination)
            .unwrap()
            .unwrap();
        for y in 0..4 {
            for x in 0..4 {
                let (sx, sy) = match transform {
                    0 => (x, y),
                    1 => (x, 3 - y),
                    2 => (3 - x, y),
                    3 => (3 - x, 3 - y),
                    4 => (y, x),
                    5 => (y, 3 - x),
                    6 => (3 - y, x),
                    _ => (3 - y, 3 - x),
                };
                assert_eq!(
                    result[y * 4 + x],
                    samples[sy * 4 + sx],
                    "transform {transform} at {x},{y}"
                );
            }
        }
        assert_eq!(
            machine.graphics_int_array_snapshot(source).unwrap(),
            source_before
        );
        assert_eq!(
            machine
                .graphics_int_array_snapshot(destination)
                .unwrap()
                .len(),
            4
        );
    }
}

#[test]
fn enhanced_micro3d_flush_preserves_clip_and_deferred_publishing() {
    let program = Program::new();
    let mut host = EnhancedHost::default();
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 2);
    machine.micro3d.target = Some(graphics);
    machine.micro3d.target_scissor = [1, 0, 1, 2];
    machine.micro3d.command_scissor = machine.micro3d.target_scissor;
    machine.micro3d_prepare_render_target().unwrap();
    assert_eq!(machine.micro3d.runtime.render_resolution_scale(), 2);
    machine
        .micro3d
        .runtime
        .load_target_samples(&[0xffff_0000; 24])
        .unwrap();
    assert_eq!(machine.graphics_int_array_snapshot(pixels).unwrap(), [0; 6]);
    machine.micro3d.render_pending = true;
    machine.micro3d_flush_target().unwrap();
    let samples = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
    for row in samples.chunks_exact(6) {
        assert_eq!(row, [0, 0, 0xffff_0000, 0xffff_0000, 0, 0]);
    }
    assert!(!machine.micro3d.render_pending);
}

#[test]
fn failed_sample_compositing_restores_guest_array_and_clears_partial_samples() {
    let program = Program::new();
    let mut host = EnhancedHost::default();
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 2, 2);
    machine.ensure_enhanced_target(pixels, 2, 2).unwrap();
    let mut calls = 0;
    let result = machine.enhanced_2d(graphics, None, 0, |machine| {
        calls += 1;
        machine
            .heap
            .managed
            .array_set(pixels, 0, HeapValue::Int(calls))
            .unwrap();
        if calls == 3 {
            return Err(EmuError::new(
                diagnostics::Category::Vm,
                "execution-cancelled",
                "synthetic cancellation",
            ));
        }
        Ok(())
    });
    assert_eq!(result.unwrap_err().code(), "execution-cancelled");
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [1, 0, 0, 0]
    );
    assert!(!machine.enhanced_2d_pending());
    assert!(!machine.graphics.enhanced.has_target(pixels));
}

#[test]
fn bound_renderers_rebuild_host_samples_after_checkpoint_restore() {
    let program = Program::new();
    let mut host = EnhancedHost::default();
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 3, 2);
    bind(&mut machine, graphics);
    machine.graphics.enhanced =
        crate::machine::graphics_dispatch::enhanced::EnhancedGraphics::default();
    machine.m3g_refresh_bound_target().unwrap();
    machine.m3g.graphics.renderer.clear(Some(0xff12_3456), true);
    machine.m3g_publish_bound_target().unwrap();
    assert_eq!(
        machine.graphics.enhanced.pixels(pixels).unwrap().unwrap(),
        [0xff12_3456; 24]
    );
    machine.micro3d.target = Some(graphics);
    machine.micro3d.target_scissor = [0, 0, 3, 2];
    machine.micro3d.command_scissor = machine.micro3d.target_scissor;
    machine.micro3d_prepare_render_target().unwrap();
    machine
        .micro3d
        .runtime
        .load_target_samples(&[0xffab_cdef; 24])
        .unwrap();
    machine.micro3d.render_pending = true;
    machine.graphics.enhanced =
        crate::machine::graphics_dispatch::enhanced::EnhancedGraphics::default();
    machine.micro3d_flush_target().unwrap();
    assert_eq!(
        machine.graphics.enhanced.pixels(pixels).unwrap().unwrap(),
        [0xffab_cdef; 24]
    );
}

#[test]
fn optimized_hud_matches_independent_sample_passes() {
    let render = |reference: bool| {
        let program = Program::new();
        let mut host = EnhancedHost::default();
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let (_, graphics, pixels) = super::text::unicode_graphics_target(&mut machine, 0, 32, 24);
        machine.ensure_enhanced_target(pixels, 32, 24).unwrap();
        let samples: Vec<u32> = (0..32 * 24 * 4)
            .map(|i| 0xff00_0000 | ((i * 7919) & 0x00ff_ffff))
            .collect();
        machine
            .graphics
            .enhanced
            .store(pixels, &samples, [0, 0, 32, 24], false)
            .unwrap();
        let source = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 64)
            .unwrap();
        for i in 0..64 {
            machine
                .heap
                .managed
                .array_set(
                    source,
                    i,
                    HeapValue::Int(((i as u32 * 31) << 24 | 0x00a0_4020).cast_signed()),
                )
                .unwrap();
        }
        let image = machine
            .heap
            .managed
            .allocate_object(
                "javax/microedition/lcdui/Image",
                HashMap::from([
                    (
                        "javax/microedition/lcdui/Image.width:I".into(),
                        HeapValue::Int(8),
                    ),
                    (
                        "javax/microedition/lcdui/Image.height:I".into(),
                        HeapValue::Int(8),
                    ),
                    (
                        "javax/microedition/lcdui/Image.pixels:[I".into(),
                        HeapValue::Reference(Some(source)),
                    ),
                    (
                        "javax/microedition/lcdui/Image.mutable:Z".into(),
                        HeapValue::Int(1),
                    ),
                ]),
            )
            .unwrap();
        for operation in 0..4 {
            let draw = |machine: &mut Machine<'_, '_>| match operation {
                0 => machine.graphics_fill_rect(&[
                    Value::Reference(Some(graphics)),
                    Value::Int(3),
                    Value::Int(2),
                    Value::Int(29),
                    Value::Int(22),
                    Value::Int(0x8040_20a0_u32.cast_signed()),
                ]),
                1 => machine.graphics_unicode_draw_text(
                    graphics,
                    crate::machine::graphics_dispatch::TextSource::Utf16(&[
                        0x41, 0x391, 0x301, 0x42f,
                    ]),
                    -2,
                    5,
                    20,
                ),
                2 => machine.graphics_draw_rgb(&[
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(source)),
                    Value::Int(56),
                    Value::Int(-8),
                    Value::Int(2),
                    Value::Int(3),
                    Value::Int(8),
                    Value::Int(8),
                    Value::Int(1),
                ]),
                _ => machine.graphics_draw_image(&[
                    Value::Reference(Some(graphics)),
                    Value::Reference(Some(image)),
                    Value::Int(6),
                    Value::Int(7),
                ]),
            };
            if reference {
                machine
                    .enhanced_2d(graphics, (operation == 3).then_some(image), 0, draw)
                    .unwrap();
            } else {
                draw(&mut machine).unwrap();
            }
        }
        (
            machine.graphics_int_array_snapshot(pixels).unwrap(),
            machine.graphics.enhanced.pixels(pixels).unwrap().unwrap(),
        )
    };
    assert_eq!(render(false), render(true));
}

#[test]
fn cached_m3g_attachment_refreshes_after_2d_and_discards_unpublished_writes() {
    let program = Program::new();
    let mut host = EnhancedHost::default();
    let mut machine = program.machine(Limits::default(), false, &mut host);
    let (graphics, pixels) = direct_graphics_target(&mut machine, 8, 8);
    bind(&mut machine, graphics);
    machine.m3g.graphics.renderer.clear(Some(0x1234_5678), true);
    machine.m3g_publish_bound_target().unwrap();
    machine.m3g_refresh_bound_target().unwrap();
    assert_eq!(machine.m3g.graphics.renderer.pixels(), &[0xff34_5678; 256]);
    machine
        .graphics_fill_rect(&[
            Value::Reference(Some(graphics)),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
            Value::Int(5),
            Value::Int(-1),
        ])
        .unwrap();
    let expected = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
    machine.m3g_refresh_bound_target().unwrap();
    assert_eq!(machine.m3g.graphics.renderer.pixels(), expected);
    machine.m3g_publish_bound_target().unwrap();
    // A render that fails before publishing must not become the next call's input.
    machine.m3g.graphics.renderer.clear(Some(0xffff_0000), true);
    machine.m3g_refresh_bound_target().unwrap();
    assert_eq!(machine.m3g.graphics.renderer.pixels(), expected);
}

#[test]
#[ignore = "manual Micro3D attachment transfer throughput measurement"]
fn enhanced_micro3d_transfer_throughput() {
    for optimized in [false, true] {
        let program = Program::new();
        let mut host = EnhancedHost::default();
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let (graphics, pixels) = direct_graphics_target(&mut machine, 240, 320);
        machine.micro3d.target = Some(graphics);
        machine.micro3d.target_scissor = [0, 0, 240, 320];
        machine.micro3d.command_scissor = machine.micro3d.target_scissor;
        machine.micro3d_prepare_render_target().unwrap();
        let samples: Vec<u32> = (0..240 * 320 * 4).map(|i| 0xff00_0000 | i).collect();
        for repeat in 0..3 {
            let started = std::time::Instant::now();
            for _ in 0..120 {
                if optimized {
                    machine.micro3d_prepare_render_target().unwrap();
                } else {
                    // Previous transfer path: native snapshot, expand, then load samples.
                    let original = machine
                        .graphics_int_array_snapshot(pixels)
                        .unwrap()
                        .into_iter()
                        .map(i32::cast_unsigned)
                        .collect();
                    machine
                        .micro3d
                        .runtime
                        .replace_target_clipped(240, 320, original, [0, 0, 240, 320])
                        .unwrap();
                    let high = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
                    machine.micro3d.runtime.load_target_samples(&high).unwrap();
                }
                machine
                    .micro3d
                    .runtime
                    .load_target_samples(std::hint::black_box(&samples))
                    .unwrap();
                machine.micro3d.render_pending = true;
                if optimized {
                    machine.micro3d_flush_target().unwrap();
                } else {
                    machine
                        .graphics
                        .enhanced
                        .store(
                            pixels,
                            machine.micro3d.runtime.target_pixels(),
                            [0, 0, 240, 320],
                            false,
                        )
                        .unwrap();
                    let resolved = machine.micro3d.runtime.resolved_target_pixels();
                    for (destination, pixel) in machine
                        .graphics_int_array_mut(pixels)
                        .unwrap()
                        .iter_mut()
                        .zip(resolved)
                    {
                        *destination = HeapValue::Int(pixel.cast_signed());
                    }
                    machine.micro3d.render_pending = false;
                }
            }
            let checksum = machine
                .graphics_int_array_snapshot(pixels)
                .unwrap()
                .iter()
                .fold(0_u64, |sum, v| {
                    sum.wrapping_mul(31)
                        .wrapping_add(u64::from(v.cast_unsigned()))
                });
            eprintln!(
                "micro3d transfer optimized={optimized} repeat={repeat} elapsed={:?} checksum={checksum:016x}",
                started.elapsed()
            );
        }
    }
}

#[test]
fn micro3d_sample_attachment_resizes_between_batches() {
    let program = Program::new();
    let mut host = EnhancedHost::default();
    let mut machine = program.machine(Limits::default(), false, &mut host);
    for (width, height) in [(3, 2), (2, 3), (4, 1)] {
        let (graphics, pixels) = direct_graphics_target(&mut machine, width, height);
        machine.micro3d.target = Some(graphics);
        machine.micro3d.target_scissor = [0, 0, width as u32, height as u32];
        machine.micro3d.command_scissor = machine.micro3d.target_scissor;
        machine.micro3d_prepare_render_target().unwrap();
        let expected: Vec<u32> = (0..width * height * 4)
            .map(|i| (i as u32).wrapping_mul(0x61c8_9e27))
            .collect();
        machine
            .micro3d
            .runtime
            .load_target_samples(&expected)
            .unwrap();
        let resolved = machine.micro3d.runtime.resolved_target_pixels();
        machine.micro3d.render_pending = true;
        machine.micro3d_flush_target().unwrap();
        assert_eq!(
            machine.graphics.enhanced.pixels(pixels).unwrap().unwrap(),
            expected
        );
        assert_eq!(
            machine
                .graphics_int_array_snapshot(pixels)
                .unwrap()
                .into_iter()
                .map(i32::cast_unsigned)
                .collect::<Vec<_>>(),
            resolved
        );
        assert!(
            machine
                .micro3d
                .runtime
                .replace_target_samples(
                    width as u32,
                    height as u32,
                    vec![0; (width * height) as usize],
                    [0, 0, width as u32, height as u32],
                )
                .is_err()
        );
        assert_eq!(machine.micro3d.runtime.target_pixels(), expected);
    }
}

#[test]
#[ignore = "manual double-density frame presentation throughput measurement"]
fn enhanced_presentation_throughput() {
    for (width, height) in [(240, 320), (480, 640)] {
        for promoted in [false, true] {
            let program = Program::new();
            let mut host = EnhancedHost::default();
            let mut machine = program.machine(
                Limits {
                    lcd_width: width as u32,
                    lcd_height: height as u32,
                    ..Limits::default()
                },
                false,
                &mut host,
            );
            let (graphics, pixels) = direct_graphics_target(&mut machine, width, height);
            bind(&mut machine, graphics);
            machine.m3g.graphics.renderer.clear(Some(0xff10_2030), true);
            machine
                .m3g
                .graphics
                .renderer
                .draw_triangle([
                    m3g::Vertex::new(m3g::Vec4::new(-1.0, -1.0, 0.0, 1.0), 0xff12_3456),
                    m3g::Vertex::new(m3g::Vec4::new(1.0, -1.0, 0.0, 1.0), 0xff98_7654),
                    m3g::Vertex::new(m3g::Vec4::new(-1.0, 1.0, 0.0, 1.0), 0xffab_cdef),
                ])
                .unwrap();
            machine.m3g_publish_bound_target().unwrap();
            if promoted {
                machine
                    .graphics_draw_line(&[
                        Value::Reference(Some(graphics)),
                        Value::Int(0),
                        Value::Int(0),
                        Value::Int(1),
                        Value::Int(1),
                    ])
                    .unwrap();
            }
            let expected = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
            let mut method = runtime_method(
                "javax/microedition/lcdui/Canvas",
                "present",
                "([I)V",
                &[],
                0,
                1,
                vec![None],
                true,
            );
            method.is_native = true;
            let args = [Value::Reference(Some(pixels))];
            for _ in 0..2 {
                machine.schedule_frame_presentation(&method, &args).unwrap();
            }
            let started = std::time::Instant::now();
            for _ in 0..128 {
                std::hint::black_box(machine.schedule_frame_presentation(&method, &args).unwrap());
            }
            let elapsed = started.elapsed();
            assert!(
                machine
                    .java_frame_matches_presented(pixels, (width as u32, height as u32))
                    .unwrap()
            );
            drop(machine);
            assert_eq!(host.frame.as_ref().unwrap().3, expected);
            let checksum = expected.iter().fold(0_u64, |sum, pixel| {
                sum.wrapping_mul(31).wrapping_add(u64::from(*pixel))
            });
            eprintln!(
                "present {width}x{height} promoted={promoted} elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}

#[test]
fn enhanced_presentation_recovers_after_sink_errors_and_canvas_mode_changes() {
    struct FallibleHost {
        inner: EnhancedHost,
        fail: std::rc::Rc<std::cell::Cell<bool>>,
    }
    impl HostServices for FallibleHost {
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
        fn render_3d_scale(&self) -> u32 {
            2
        }
        fn present_lcdui_frame_scaled(
            &mut self,
            width: u32,
            height: u32,
            scale: u32,
            pixels: &[u32],
        ) -> Result<(), EmuError> {
            if self.fail.get() {
                return Err(vm_error("test-frame-sink", "synthetic frame sink failure"));
            }
            self.inner
                .present_lcdui_frame_scaled(width, height, scale, pixels)
        }
    }
    let program = Program::new();
    let fail = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut host = FallibleHost {
        inner: EnhancedHost::default(),
        fail: fail.clone(),
    };
    let mut machine = program.machine(
        Limits {
            lcd_width: 8,
            lcd_height: 8,
            lcd_normal_width: 4,
            lcd_normal_height: 4,
            ..Limits::default()
        },
        false,
        &mut host,
    );
    let (full_graphics, full_pixels) = direct_graphics_target(&mut machine, 8, 8);
    let (normal_graphics, normal_pixels) = direct_graphics_target(&mut machine, 4, 4);
    let mut method = runtime_method(
        "javax/microedition/lcdui/Canvas",
        "present",
        "([I)V",
        &[],
        0,
        1,
        vec![None],
        true,
    );
    method.is_native = true;
    let receiver = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Graphics3D", HashMap::new())
        .unwrap();
    let release_args = [Value::Reference(Some(receiver))];
    bind(&mut machine, full_graphics);
    machine.m3g.graphics.renderer.clear(Some(0xff12_3456), true);
    machine.m3g_publish_bound_target().unwrap();
    let full_args = [Value::Reference(Some(full_pixels))];
    machine
        .schedule_frame_presentation(&method, &full_args)
        .unwrap();
    assert!(
        machine
            .java_frame_matches_presented(full_pixels, (8, 8))
            .unwrap()
    );
    fail.set(true);
    let error = machine
        .schedule_frame_presentation(&method, &full_args)
        .err()
        .unwrap();
    assert_eq!(error.code(), "test-frame-sink");
    assert!(
        !machine
            .java_frame_matches_presented(full_pixels, (8, 8))
            .unwrap()
    );
    fail.set(false);
    machine
        .schedule_frame_presentation(&method, &full_args)
        .unwrap();
    assert!(
        machine
            .java_frame_matches_presented(full_pixels, (8, 8))
            .unwrap()
    );
    machine
        .invoke_m3g_graphics3d_native(
            "javax/microedition/m3g/Graphics3D",
            "releaseTarget",
            "()V",
            &release_args,
        )
        .unwrap();
    bind(&mut machine, normal_graphics);
    machine.m3g.graphics.renderer.clear(Some(0xff98_7654), true);
    machine.m3g_publish_bound_target().unwrap();
    machine
        .schedule_frame_presentation(&method, &[Value::Reference(Some(normal_pixels))])
        .unwrap();
    assert!(
        machine
            .java_frame_matches_presented(normal_pixels, (4, 4))
            .unwrap()
    );
    assert!(
        !machine
            .java_frame_matches_presented(full_pixels, (8, 8))
            .unwrap()
    );
    machine
        .invoke_m3g_graphics3d_native(
            "javax/microedition/m3g/Graphics3D",
            "releaseTarget",
            "()V",
            &release_args,
        )
        .unwrap();
    bind(&mut machine, full_graphics);
    machine
        .schedule_frame_presentation(&method, &full_args)
        .unwrap();
    assert!(
        machine
            .java_frame_matches_presented(full_pixels, (8, 8))
            .unwrap()
    );
    drop(machine);
    assert_eq!(host.inner.frame, Some((8, 8, 2, vec![0xff12_3456; 256])));
}

fn triangle_graphics_state(
    machine: &mut Machine<'_, '_>,
    graphics: Handle,
    color: i32,
    translation: (i32, i32),
    clip: (i32, i32, i32, i32),
) {
    for (field, value) in [
        ("color:I", color),
        ("tx:I", translation.0),
        ("ty:I", translation.1),
        ("clipX:I", clip.0),
        ("clipY:I", clip.1),
        ("clipW:I", clip.2),
        ("clipH:I", clip.3),
    ] {
        machine
            .heap
            .managed
            .set_field(
                graphics,
                &format!("javax/microedition/lcdui/Graphics.{field}"),
                HeapValue::Int(value),
            )
            .unwrap();
    }
}

fn triangle_arguments(graphics: Handle, points: [(i32, i32); 3]) -> [Value; 7] {
    [
        Value::Reference(Some(graphics)),
        Value::Int(points[0].0),
        Value::Int(points[0].1),
        Value::Int(points[1].0),
        Value::Int(points[1].1),
        Value::Int(points[2].0),
        Value::Int(points[2].1),
    ]
}

#[test]
fn enhanced_fill_triangle_matches_independent_logical_rasters_for_all_samples() {
    check_enhanced_fill_triangle(false);
    check_enhanced_fill_triangle(true);
}

fn check_enhanced_fill_triangle(promoted: bool) {
    const WIDTH: i32 = 8;
    const HEIGHT: i32 = 6;
    let cases = [
        ([(0, 0), (7, 0), (0, 5)], (0, 0), (0, 0, 8, 6)),
        ([(-4, -2), (7, 0), (0, 7)], (1, -1), (1, 1, 5, 4)),
        ([(1, 1), (1, 1), (6, 4)], (-2, 1), (0, 0, 8, 6)),
        (
            [(i32::MIN, i32::MAX), (i32::MAX, i32::MIN), (3, 4)],
            (2, -3),
            (0, 0, 8, 6),
        ),
        ([(0, 0), (7, 0), (0, 5)], (0, 0), (3, 2, 0, 0)),
    ];
    for color in [0xff90_4020_u32, 0x8042_9bd7, 0x0042_9bd7] {
        for (points, translation, clip) in cases {
            let program = Program::new();
            let limits = Limits {
                lcd_width: WIDTH as u32,
                lcd_height: HEIGHT as u32,
                ..Limits::default()
            };
            let mut host = EnhancedHost::default();
            let mut machine = program.machine(limits.clone(), false, &mut host);
            let (graphics, pixels) = direct_graphics_target(&mut machine, WIDTH, HEIGHT);
            assert!(
                machine
                    .ensure_enhanced_target(pixels, WIDTH as u32, HEIGHT as u32)
                    .unwrap()
            );
            if promoted {
                machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
            }
            let initial: Vec<u32> = (0..WIDTH * HEIGHT * 4)
                .map(|index| {
                    let index = index as u32;
                    [0x0000_0000, 0x4000_0000, 0x8000_0000, 0xff00_0000][index as usize % 4]
                        | index.wrapping_mul(0x0005_374b) & 0x00ff_ffff
                })
                .collect();
            machine
                .graphics
                .enhanced
                .store(pixels, &initial, [0, 0, WIDTH as u32, HEIGHT as u32], false)
                .unwrap();
            let guest = machine.graphics_int_array_snapshot(pixels).unwrap();
            triangle_graphics_state(
                &mut machine,
                graphics,
                color.cast_signed(),
                translation,
                clip,
            );
            machine
                .graphics_fill_triangle(&triangle_arguments(graphics, points))
                .unwrap();
            let mut expected_samples = initial.clone();
            for phase in 0..5 {
                let mut reference_host = DefaultNativeContext;
                let mut reference = program.machine(limits.clone(), false, &mut reference_host);
                let (reference_graphics, reference_pixels) =
                    direct_graphics_target(&mut reference, WIDTH, HEIGHT);
                let original: Vec<i32> = if phase == 4 {
                    guest.clone()
                } else {
                    (0..WIDTH * HEIGHT)
                        .map(|index| {
                            let (x, y) = (index % WIDTH, index / WIDTH);
                            initial[((y * 2 + phase / 2) * WIDTH * 2 + x * 2 + phase % 2) as usize]
                                .cast_signed()
                        })
                        .collect()
                };
                for (slot, pixel) in reference
                    .graphics_int_array_mut(reference_pixels)
                    .unwrap()
                    .iter_mut()
                    .zip(original)
                {
                    *slot = HeapValue::Int(pixel);
                }
                triangle_graphics_state(
                    &mut reference,
                    reference_graphics,
                    color.cast_signed(),
                    translation,
                    clip,
                );
                reference
                    .graphics_fill_triangle(&triangle_arguments(reference_graphics, points))
                    .unwrap();
                let expected = reference
                    .graphics_int_array_snapshot(reference_pixels)
                    .unwrap();
                if phase == 4 {
                    assert_eq!(
                        machine.graphics_int_array_snapshot(pixels).unwrap(),
                        expected
                    );
                } else {
                    for (index, pixel) in expected.into_iter().enumerate() {
                        let (x, y) = (index as i32 % WIDTH, index as i32 / WIDTH);
                        expected_samples
                            [((y * 2 + phase / 2) * WIDTH * 2 + x * 2 + phase % 2) as usize] =
                            pixel.cast_unsigned();
                    }
                }
            }
            assert_eq!(
                machine.graphics.enhanced.pixels(pixels).unwrap().unwrap(),
                expected_samples,
                "promoted={promoted} color={color:08x} points={points:?} translation={translation:?} clip={clip:?}"
            );
        }
    }
}

#[test]
#[ignore = "manual mixed-density triangle throughput measurement"]
fn enhanced_triangle_throughput() {
    for (width, height) in [(240, 320), (480, 640)] {
        for scale in [1, 2] {
            for alpha in [false, true] {
                let program = Program::new();
                let mut host = EnhancedHost {
                    original: scale == 1,
                    ..EnhancedHost::default()
                };
                let mut machine = program.machine(
                    Limits {
                        lcd_width: width as u32,
                        lcd_height: height as u32,
                        ..Limits::default()
                    },
                    false,
                    &mut host,
                );
                let (graphics, pixels) = direct_graphics_target(&mut machine, width, height);
                assert_eq!(
                    machine
                        .ensure_enhanced_target(pixels, width as u32, height as u32)
                        .unwrap(),
                    scale == 2
                );
                let args = triangle_arguments(
                    graphics,
                    [(3, 5), (width - 4, height / 3), (width / 4, height - 5)],
                );
                let started = std::time::Instant::now();
                for frame in 0_u32..64 {
                    let color = (if alpha { 0x8000_0000 } else { 0xff00_0000 })
                        | (0x0042_9bd7 ^ (frame * 997));
                    machine
                        .heap
                        .managed
                        .set_field(
                            graphics,
                            "javax/microedition/lcdui/Graphics.color:I",
                            HeapValue::Int(color.cast_signed()),
                        )
                        .unwrap();
                    machine
                        .graphics_fill_triangle(std::hint::black_box(&args))
                        .unwrap();
                }
                let elapsed = started.elapsed();
                let actual = if scale == 2 {
                    machine.graphics.enhanced.pixels(pixels).unwrap().unwrap()
                } else {
                    machine
                        .graphics_int_array_snapshot(pixels)
                        .unwrap()
                        .into_iter()
                        .map(i32::cast_unsigned)
                        .collect()
                };
                let checksum = actual.into_iter().fold(0_u64, |sum, pixel| {
                    sum.wrapping_mul(31).wrapping_add(u64::from(pixel))
                });
                eprintln!(
                    "triangle scale={scale} size={width}x{height} alpha={alpha} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}

#[test]
fn enhanced_fill_triangle_cancels_within_a_long_row_and_can_render_again() {
    use std::cell::Cell;
    use std::rc::Rc;

    for promoted in [false, true] {
        for color in [0xff90_4020_u32, 0x8042_9bd7] {
            let program = Program::new();
            let checks = Rc::new(Cell::new(0));
            let cancel_at = Rc::new(Cell::new(usize::MAX));
            let mut host = EnhancedHost {
                cancellation: Some(CancellationContext {
                    checks: Rc::clone(&checks),
                    cancel_at: Rc::clone(&cancel_at),
                }),
                ..EnhancedHost::default()
            };
            let mut machine = program.machine(Limits::default(), false, &mut host);
            let (graphics, pixels) = direct_graphics_target(&mut machine, 8192, 1);
            assert!(machine.ensure_enhanced_target(pixels, 8192, 1).unwrap());
            if promoted {
                machine.enhanced_2d(graphics, None, 0, |_| Ok(())).unwrap();
            }
            triangle_graphics_state(
                &mut machine,
                graphics,
                color.cast_signed(),
                (0, 0),
                (0, 0, 8192, 1),
            );
            let args = triangle_arguments(graphics, [(0, 0), (8191, 0), (4096, 1)]);
            checks.set(0);
            cancel_at.set(2);
            assert_eq!(
                machine.graphics_fill_triangle(&args).unwrap_err().code(),
                "execution-cancelled"
            );
            let partial = machine.graphics_int_array_snapshot(pixels).unwrap();
            assert!(partial.contains(&0));
            assert!(partial.iter().any(|pixel| *pixel != 0));
            cancel_at.set(usize::MAX);
            assert!(machine.ensure_enhanced_target(pixels, 8192, 1).unwrap());
            machine.graphics_fill_triangle(&args).unwrap();
            let logical = machine.graphics_int_array_snapshot(pixels).unwrap();
            let expected: Vec<u32> = logical
                .iter()
                .flat_map(|pixel| [pixel.cast_unsigned(); 2])
                .collect();
            let samples = machine.graphics.enhanced.pixels(pixels).unwrap().unwrap();
            assert_eq!(&samples[..expected.len()], expected);
            assert_eq!(&samples[expected.len()..], expected);
        }
    }
}

#[test]
fn enhanced_fill_triangle_keeps_checked_backing_errors_and_opaque_tag_replacement() {
    for malformed_tag in [false, true] {
        for color in [0xff90_4020_u32, 0x8042_9bd7] {
            let program = Program::new();
            let mut host = EnhancedHost::default();
            let mut machine = program.machine(Limits::default(), false, &mut host);
            let (graphics, pixels) = direct_graphics_target(&mut machine, 8, 6);
            assert!(machine.ensure_enhanced_target(pixels, 8, 6).unwrap());
            let Allocation::Array { elements, .. } = machine.heap.managed.get_mut(pixels).unwrap()
            else {
                panic!("expected pixel array");
            };
            if malformed_tag {
                elements.fill(HeapValue::Reference(None));
            } else {
                elements.truncate(1);
            }
            triangle_graphics_state(
                &mut machine,
                graphics,
                color.cast_signed(),
                (0, 0),
                (0, 0, 8, 6),
            );
            let result = machine
                .graphics_fill_triangle(&triangle_arguments(graphics, [(0, 0), (7, 0), (0, 5)]));
            if malformed_tag && color >> 24 == 255 {
                result.unwrap();
                assert_eq!(
                    machine.graphics_int_array_mut(pixels).unwrap()[0],
                    HeapValue::Int(color.cast_signed())
                );
            } else {
                assert_eq!(
                    result.unwrap_err().code(),
                    if malformed_tag {
                        "type-mismatch"
                    } else {
                        "array-index-out-of-bounds-exception"
                    }
                );
            }
        }
    }
}

mod native_fields;
