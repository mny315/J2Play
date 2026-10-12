use super::*;

pub(super) fn paint_library_size(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    size: Vec2,
    key: Option<Key>,
) {
    paint_library_navigation(app, ctx, size, key, false);
}

pub(super) fn paint_library_navigation(
    app: &mut FrontendApp,
    ctx: &egui::Context,
    size: Vec2,
    key: Option<Key>,
    controller: bool,
) {
    use frontend_core::physical_input::{GamepadButton, PhysicalControl, PhysicalInputEvent};
    let mut button = key.filter(|_| controller).map(|key| match key {
        Key::ArrowUp => GamepadButton::DpadUp,
        Key::ArrowDown => GamepadButton::DpadDown,
        Key::ArrowLeft => GamepadButton::DpadLeft,
        Key::ArrowRight => GamepadButton::DpadRight,
        Key::Enter => GamepadButton::South,
        _ => panic!("unsupported fixture navigation key"),
    });
    let mut events: Vec<_> = key
        .filter(|_| !controller)
        .into_iter()
        .flat_map(|key| {
            [true, false].map(|pressed| Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            })
        })
        .collect();
    // egui resolves focus, the scroll target, then the new interaction rectangles.
    for _ in 0..4 {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events: std::mem::take(&mut events),
                ..Default::default()
            },
            |ui| {
                if let Some(button) = button.take() {
                    for pressed in [true, false] {
                        app.process_physical_event(
                            ctx,
                            PhysicalInputEvent::Button {
                                device: 1,
                                control: PhysicalControl::Gamepad { button },
                                pressed,
                            },
                            false,
                        );
                    }
                }
                app.process_library_keyboard(ctx);
                app.draw_library(ui);
                app.focus_ring
                    .paint(ctx, &app.material_theme, true, ui.clip_rect());
            },
        )
        .drop_without_applying_deltas();
    }
}

#[test]
fn tile_navigation_scrolls_to_partial_rows_and_keeps_the_focused_action_after_reflow() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = library_fixture(root, 8);
    let ctx = egui::Context::default();
    apply_material_theme(&ctx, &app.material_theme);
    let mut style = (*ctx.global_style()).clone();
    style.scroll_animation = egui::style::ScrollAnimation::none();
    ctx.set_global_style(style);
    let size = Vec2::new(640.0, 320.0); // Three columns with a partial last row.
    let launches: Vec<_> = app
        .entries
        .iter()
        .map(|entry| crate::library_view::launch_id(entry.id()))
        .collect();
    paint_library_size(&mut app, &ctx, size, None);
    assert!(
        app.icons.len() < app.entries.len(),
        "offscreen rows must stay lazy"
    );
    paint_library_size(&mut app, &ctx, size, Some(Key::ArrowDown));
    assert_eq!(ctx.memory(egui::Memory::focused), Some(launches[0]));
    for expected in &launches[1..=2] {
        paint_library_size(&mut app, &ctx, size, Some(Key::ArrowRight));
        assert_eq!(ctx.memory(egui::Memory::focused), Some(*expected));
    }
    for index in [5, 7] {
        paint_library_size(&mut app, &ctx, size, Some(Key::ArrowDown));
        assert!(
            !launches.contains(&ctx.memory(egui::Memory::focused).unwrap()),
            "Settings is separately reachable"
        );
        paint_library_size(&mut app, &ctx, size, Some(Key::ArrowDown));
        assert_eq!(ctx.memory(egui::Memory::focused), Some(launches[index]));
        let focused = ctx.read_response(launches[index]).unwrap();
        assert!(focused.interact_rect.height() >= 48.0, "{focused:?}");
    }
    paint_library_size(&mut app, &ctx, size, Some(Key::ArrowDown));
    let settings_id = ctx.memory(egui::Memory::focused).unwrap();
    // Keep the same game and Settings action after changing both view and column count.
    for size in [Vec2::new(400.0, 700.0), Vec2::new(800.0, 400.0), size] {
        paint_library_size(&mut app, &ctx, size, None);
        assert_eq!(ctx.memory(egui::Memory::focused), Some(settings_id));
        let response = ctx.read_response(settings_id).unwrap();
        assert!(
            response.interact_rect.height() >= 48.0,
            "size {size:?}: {response:?}"
        );
    }
    paint_library_size(&mut app, &ctx, size, Some(Key::Enter));
    assert!(matches!(app.screen, Screen::Settings(_)));
    assert_eq!(app.session.state(), SessionState::Idle);
}

#[test]
fn pointer_swipes_keep_the_library_position_after_navigation_focus() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 8);
    for view in [LibraryView::List, LibraryView::Tiles] {
        app.app_settings.library_view = view;
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        let mut time = 0.0;
        let mut paint = |app: &mut FrontendApp, events| {
            time += 1.0 / 60.0;
            let mut offset = 0.0;
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 320.0))),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    app.process_library_keyboard(&ctx);
                    app.focus_ring.observe_input(&ctx, false);
                    app.draw_library(ui);
                    app.focus_ring
                        .paint(&ctx, &app.material_theme, true, ui.clip_rect());
                    offset = egui::scroll_area::State::load(
                        &ctx,
                        ui.make_persistent_id(egui::IdSalt::new("game-library")),
                    )
                    .expect("library scroll state")
                    .offset
                    .y;
                },
            )
            .drop_without_applying_deltas();
            offset
        };
        for _ in 0..30 {
            paint(&mut app, vec![]);
        }
        ctx.memory_mut(|memory| {
            memory.request_focus(crate::library_view::launch_id(app.entries[0].id()));
        });
        for _ in 0..30 {
            paint(&mut app, vec![]);
        }
        let mut pos = ctx
            .read_response(crate::library_view::launch_id(app.entries[0].id()))
            .unwrap()
            .interact_rect
            .center();
        let pointer = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        paint(&mut app, vec![Event::PointerMoved(pos), pointer(pos, true)]);
        let mut offset = 0.0;
        for _ in 0..7 {
            pos.y -= 24.0;
            offset = paint(&mut app, vec![Event::PointerMoved(pos)]);
        }
        assert!(offset > 130.0, "{view:?}: swipe offset {offset}");
        paint(
            &mut app,
            vec![
                pointer(pos, false),
                Event::PointerMoved(Pos2::new(120.0, 280.0)),
            ],
        );
        for _ in 0..90 {
            let settled = paint(&mut app, vec![]);
            assert!(
                settled >= offset - 2.0,
                "{view:?}: returned from {offset} to {settled}"
            );
        }
        assert!(matches!(app.screen, Screen::Library));
        assert_eq!(app.session.state(), SessionState::Idle);
    }
}

#[test]
fn up_from_the_first_game_reaches_app_settings_and_returns_to_the_library() {
    for view in [LibraryView::Tiles, LibraryView::List] {
        for size in [Vec2::new(1280.0, 800.0), Vec2::new(453.0, 440.0)] {
            let scratch = test_storage::Scratch::new();
            let mut app = library_fixture(&scratch.0, 8);
            app.app_settings.library_view = view;
            let ctx = egui::Context::default();
            apply_material_theme(&ctx, &app.material_theme);
            for controller in [false, true] {
                app.screen = Screen::Library;
                let paint = |app: &mut FrontendApp, key| {
                    paint_library_navigation(app, &ctx, size, key, controller);
                };
                paint(&mut app, None);
                ctx.memory_mut(|memory| {
                    if let Some(id) = memory.focused() {
                        memory.surrender_focus(id);
                    }
                });
                paint(&mut app, Some(Key::ArrowDown));
                let first = ctx.memory(egui::Memory::focused).unwrap();
                paint(&mut app, Some(Key::ArrowUp));
                let folder = ctx.memory(egui::Memory::focused).unwrap();
                assert_ne!(folder, first);
                paint(&mut app, Some(Key::ArrowUp));
                let settings = ctx.memory(egui::Memory::focused).unwrap();
                let response = ctx.read_response(settings).unwrap();
                assert_ne!(
                    settings, first,
                    "{view:?} {size:?}: Up stays on the first game"
                );
                assert!(response.rect.bottom() < ctx.read_response(first).unwrap().rect.top());
                assert!(response.rect.height() <= 60.0, "{response:?}");
                paint(&mut app, Some(Key::ArrowRight));
                assert_ne!(ctx.memory(egui::Memory::focused), Some(settings));
                paint(&mut app, Some(Key::ArrowLeft));
                assert_eq!(ctx.memory(egui::Memory::focused), Some(settings));
                paint(&mut app, Some(Key::ArrowDown));
                assert_eq!(ctx.memory(egui::Memory::focused), Some(folder));
                paint(&mut app, Some(Key::ArrowDown));
                assert_eq!(ctx.memory(egui::Memory::focused), Some(first));
                paint(&mut app, Some(Key::ArrowUp));
                paint(&mut app, Some(Key::ArrowUp));
                paint(&mut app, Some(Key::Enter));
                assert!(matches!(app.screen, Screen::AppSettings(_)));
            }
        }
    }
}
