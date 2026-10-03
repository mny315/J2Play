use super::*;

#[test]
fn runtime_debug_rates_and_hud_are_frontend_global() {
    let started = Instant::now();
    let mut debug = RuntimeDebugState::new(true);
    debug.note_vm(
        started,
        VmDebugStats {
            instructions: 1_000_000,
            native_calls: 10,
            draw_image_calls: 20,
            ..VmDebugStats::default()
        },
    );
    debug.note_vm(
        started.checked_add(Duration::from_millis(500)).unwrap(),
        VmDebugStats {
            instructions: 2_000_000,
            heap_bytes: 2 * 1024 * 1024,
            runnable_threads: 2,
            native_calls: 110,
            draw_image_calls: 70,
            ..VmDebugStats::default()
        },
    );

    assert!((debug.vm_mips - 2.0).abs() < f64::EPSILON);
    assert_eq!(debug.native_per_second, 200);
    assert_eq!(debug.draw_image_per_second, 100);
    let lines = runtime_debug_lines(&debug, SessionState::Running, (240, 320), false, 3);
    assert!(lines[0].contains("RUNTIME RUNNING 240x320 FF Off"));
    assert!(lines.iter().any(|line| line.contains("VM 2.00MIPS")));
    assert_eq!(lines.last().map(String::as_str), Some("DIAGNOSTICS 3"));

    debug.reset_session();
    assert!(debug.enabled);
    assert!(debug.vm.is_none());
}

#[test]
fn runtime_debug_does_no_sampling_work_while_disabled() {
    let now = Instant::now();
    let mut debug = RuntimeDebugState::default();
    debug.note_frame(now);
    debug.note_vm(
        now,
        VmDebugStats {
            instructions: 123,
            ..VmDebugStats::default()
        },
    );
    assert_eq!(debug.frame_window_count, 0);
    assert!(debug.vm.is_none());
}

#[test]
fn hud_stays_english_and_wraps_without_clipping_in_every_ui_language() {
    let ctx = egui::Context::default();
    let mut debug = RuntimeDebugState::new(true);
    debug.note_vm(
        Instant::now(),
        VmDebugStats {
            instructions: 1_234_567,
            heap_bytes: 4 * 1024 * 1024,
            runnable_threads: 8,
            m3g_live_objects: 20,
            ..Default::default()
        },
    );
    for language in frontend_core::Language::ALL {
        crate::i18n::install(&ctx, language);
        for width in [160.0, 240.0, 453.0] {
            let available = Rect::from_min_size(egui::pos2(10.0, 10.0), Vec2::new(width, 600.0));
            let lines = runtime_debug_lines(
                &debug,
                SessionState::Paused {
                    user: true,
                    lifecycle: false,
                },
                (240, 320),
                false,
                30,
            );
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint_runtime_debug_hud(ui, available, &lines);
            });
            output.textures_delta.clear();
            let text = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((text, shape.clip_rect)),
                    _ => None,
                })
                .expect("the HUD must paint its text");
            assert!(
                text.1
                    .contains_rect(text.0.galley.rect.translate(text.0.pos.to_vec2())),
                "{language:?} at {width}: {:?}",
                text.0.galley.job.text
            );
            assert!(!text.0.galley.elided);
            assert!(text.0.galley.job.text.contains("1234567"));
            assert!(text.0.galley.job.text.contains("RUNTIME USER PAUSE"));
            output.drop_without_applying_deltas();
        }
    }
}
