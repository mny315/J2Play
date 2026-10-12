use super::*;

fn paint(app: &mut FrontendApp, ctx: &egui::Context, time: f64, events: Vec<Event>) {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 600.0))),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ui| {
            app.process_library_keyboard(ctx);
            app.draw_library(ui);
        },
    )
    .drop_without_applying_deltas();
}

fn renamed_fixture(title: &str) -> PreparedImport {
    inspect_import(ImportSource::new(
        None,
        include_bytes!("../../../fixtures/java-me/conformance.jar").to_vec(),
        Some(format!("MIDlet-1: {title},,fixtures.Stage7LifecycleMidlet\n").into_bytes()),
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap()
}

#[test]
fn search_filters_unicode_titles_and_back_restores_the_library() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 8);
    assert!(app.commit_import(
        &renamed_fixture("Космическая игра"),
        GameSettings::default()
    ));
    let ctx = egui::Context::default();
    apply_material_theme(&ctx, &app.material_theme);
    app.library_added = None;
    app.library_search.open = true;
    app.library_search.focus = true;
    paint(&mut app, &ctx, 0.0, vec![]);
    paint(
        &mut app,
        &ctx,
        0.1,
        vec![Event::Text(" КОСМИЧЕСКАЯ ".into())],
    );
    assert_eq!(app.library_search.indices.len(), 1);
    let index = app.library_search.indices[0];
    assert_eq!(app.entries[index].title(), "Космическая игра");
    assert_eq!(
        ctx.memory(egui::Memory::focused),
        Some(crate::library_search::field_id())
    );
    app.navigate_back();
    assert!(!app.exit_confirmation);
    paint(&mut app, &ctx, 0.2, vec![]);
    assert_eq!(app.library_search.indices.len(), 8);
    assert!(app.library_search.query.is_empty());
}

#[test]
fn filtered_navigation_opens_the_matching_games_settings_in_both_views() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        let scratch = test_storage::Scratch::new();
        let mut app = library_fixture(&scratch.0, 8);
        app.app_settings.library_view = view;
        app.library_search.open = true;
        app.library_search.query = app.entries[6].title().to_uppercase();
        let expected = app.entries[6].id().to_owned();
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        let size = Vec2::new(640.0, 700.0);
        navigation::paint_library_size(&mut app, &ctx, size, None);
        assert_eq!(app.library_search.indices, [6]);
        navigation::paint_library_size(&mut app, &ctx, size, Some(Key::ArrowDown));
        assert_eq!(
            ctx.memory(egui::Memory::focused),
            Some(crate::library_view::launch_id(&expected))
        );
        navigation::paint_library_size(
            &mut app,
            &ctx,
            size,
            Some(if view == LibraryView::Tiles {
                Key::ArrowDown
            } else {
                Key::ArrowRight
            }),
        );
        navigation::paint_library_size(&mut app, &ctx, size, Some(Key::Enter));
        let Screen::Settings(settings) = &app.screen else {
            panic!("settings expected")
        };
        assert!(
            matches!(&settings.target, SettingsTarget::Existing { entry_id } if entry_id == &expected)
        );
    }
}

#[test]
fn import_clears_search_reveals_the_sorted_game_and_fades_for_ten_seconds() {
    for view in [LibraryView::List, LibraryView::Tiles] {
        let scratch = test_storage::Scratch::new();
        let mut app = library_fixture(&scratch.0, 8);
        app.app_settings.library_view = view;
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        paint(&mut app, &ctx, 0.0, vec![]);
        app.library_search.open = true;
        app.library_search.query = "no match".into();
        assert!(app.commit_import(&renamed_fixture("ZZZZ Added"), GameSettings::default()));
        assert!(!app.library_search.open);
        assert!(app.library_search.query.is_empty());
        let index = app
            .entries
            .iter()
            .position(|entry| entry.title() == "ZZZZ Added")
            .unwrap();
        assert_eq!(index, 7);
        let id = crate::library_view::launch_id(app.entries[index].id());
        for step in 0..4 {
            paint(&mut app, &ctx, 100.0 + f64::from(step) * 0.01, vec![]);
        }
        let response = ctx
            .read_response(id)
            .expect("imported game must be rendered");
        assert!(
            response.interact_rect.height() >= 48.0,
            "{view:?}: {response:?}"
        );
        let added = app.library_added.as_mut().unwrap();
        assert!(added.strength(100.1) > 0.99);
        assert!((added.strength(105.0) - 0.5).abs() < 0.01);
        paint(&mut app, &ctx, 110.1, vec![]);
        assert!(app.library_added.is_none());
    }
}

#[test]
fn native_search_ime_composes_unicode_and_deletes_utf16_without_guest_input() {
    let scratch = test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 1);
    let ctx = egui::Context::default();
    app.library_search.open = true;
    app.library_search.focus = true;
    paint(&mut app, &ctx, 0.0, vec![]);
    let mut time = 0.0;
    let mut input = |app: &mut FrontendApp, event: PlatformTextInputEvent| {
        time += 0.1;
        ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                ..Default::default()
            },
            |ui| {
                app.library_search.native_input(&ctx, event.clone());
                app.draw_library_search(ui);
            },
        )
        .drop_without_applying_deltas();
    };
    input(
        &mut app,
        PlatformTextInputEvent::Composition {
            text: "Игр".into(),
            selection_start: 3,
            selection_end: 3,
        },
    );
    assert_eq!(app.library_search.query, "Игр");
    input(&mut app, PlatformTextInputEvent::Commit("Игра😀".into()));
    assert_eq!(app.library_search.query, "Игра😀");
    input(
        &mut app,
        PlatformTextInputEvent::DeleteUtf16 {
            before: 2,
            after: 0,
        },
    );
    assert_eq!(app.library_search.query, "Игра");
    input(&mut app, PlatformTextInputEvent::MoveCaret { offset: -1 });
    input(&mut app, PlatformTextInputEvent::Commit("!".into()));
    assert_eq!(app.library_search.query, "Игр!а");
    assert_eq!(app.session.state(), SessionState::Idle);
}
