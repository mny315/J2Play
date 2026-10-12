use super::*;
use frontend_core::LibraryView;

mod deletion;
mod folders;
mod hover;
mod navigation;
mod search;
mod selection;

#[test]
fn library_information_preserves_row_height_and_separate_settings_target() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    let prepared = inspect_import(ImportSource::new(
        None,
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/java-me/conformance.jar")).to_vec(),
        Some(b"MIDlet-1: Library Fixture, , fixtures.Stage7LifecycleMidlet\nMIDlet-Vendor: Fixture Studio\nMIDlet-Year: 2008\n".to_vec()),
    )).unwrap().select_midlet(1).unwrap();
    let settings = GameSettings {
        device_profile: ProfileChoice::Manual {
            profile_id: "se-jp8-keypad".to_owned(),
        },
        ..GameSettings::default()
    };
    app.entries
        .push(app.repository.commit_import(&prepared, settings).unwrap());
    assert_eq!(library_game_info(&app.entries[0]), "2008 · Fixture Studio");
    assert_eq!(
        english_library_profile_presentation(
            &app.entries[0],
            &app.profile_options,
            &app.catalog_fingerprint
        )
        .label,
        "SE 240×320"
    );

    for width in [240.0, 320.0, 457.0, 800.0] {
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        for _ in 0..2 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 300.0))),
                    ..Default::default()
                },
                |ui| {
                    let row_width = ui.available_width();
                    let top = ui.cursor().top();
                    let ids = app.draw_library_row(ui, 0, row_width);
                    let launch = ctx.read_response(ids[0]).unwrap();
                    let settings = ctx.read_response(ids[1]).unwrap();
                    assert!(!launch.rect.intersects(settings.rect));
                    assert!(settings.rect.height() >= 48.0);
                    assert!(settings.rect.right() <= ui.max_rect().right() + 0.01);
                    assert!(
                        (ui.min_rect().bottom() - top - LIBRARY_ROW_HEIGHT).abs() < 0.01,
                        "width {width}: {:?}",
                        ui.min_rect()
                    );
                },
            );
            for label in ["Library Fixture", "2008 · Fixture Studio", "SE 240×320"] {
                assert!(
                    output.shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Text(text) if text.galley.job.text == label)),
                    "{width}: {label}"
                );
            }
            output.drop_without_applying_deltas();
        }
    }
}

#[test]
fn library_orientation_uses_resolved_canvas_and_does_not_confuse_touch_with_landscape() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let repository = LibraryRepository::open(root).unwrap();
    let profiles = launch::builtin_device_profiles().unwrap();
    let options: Vec<_> = profiles.iter().map(ProfileOption::from_profile).collect();
    let fingerprint = catalog_fingerprint(profiles);
    let fixture = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/java-me/conformance.jar"
    ));
    for (name, dimensions, touch, orientation) in [
        (
            "fixture_NOKIA_5800_EN_L_build.jar",
            (640, 360),
            true,
            "Landscape",
        ),
        ("fixture_NOKIA_5800_build.jar", (360, 640), true, "Portrait"),
        (
            "fixture_nokia_e71_build.jar",
            (320, 240),
            false,
            "Landscape",
        ),
    ] {
        let prepared = inspect_import(ImportSource::new(
            Some(name.to_owned()),
            fixture.to_vec(),
            None,
        ))
        .unwrap()
        .select_midlet(1)
        .unwrap();
        let mut entry = repository
            .commit_import(&prepared, GameSettings::default())
            .unwrap();
        assert_eq!(
            entry.automatic_canvas_dimensions(&fingerprint),
            Some(dimensions),
            "{name}"
        );
        let presentation = english_library_profile_presentation(&entry, &options, &fingerprint);
        let screen = format!("{}×{}", dimensions.0, dimensions.1);
        assert!(presentation.label.contains(&screen));
        assert!(presentation.description.contains(&screen));
        assert!(presentation.description.contains(orientation));
        assert_eq!(
            presentation.icon.unwrap(),
            crate::layout::LibraryProfileIcon {
                canvas_dimensions: dimensions,
                touch
            }
        );

        let stale = english_library_profile_presentation(&entry, &options, "different-catalog");
        assert_eq!(stale.label, "Automatic");
        assert_eq!(stale.icon, None);
        assert_eq!(entry.automatic_canvas_dimensions("different-catalog"), None);

        repository
            .save_settings(
                &mut entry,
                GameSettings {
                    device_profile: ProfileChoice::Manual {
                        profile_id: "se-jp8-keypad".to_owned(),
                    },
                    ..GameSettings::default()
                },
            )
            .unwrap();
        assert_eq!(entry.automatic_canvas_dimensions(&fingerprint), None);
        let manual = english_library_profile_presentation(&entry, &options, "different-catalog");
        assert_eq!(manual.label, "SE 240×320");
        assert_eq!(
            manual.icon.unwrap(),
            crate::layout::LibraryProfileIcon {
                canvas_dimensions: (240, 320),
                touch: false
            }
        );
    }
}

fn library_fixture(root: &Path, count: u32) -> FrontendApp {
    let mut app = FrontendApp::new(root, Box::new(UnavailablePlatformBridge)).unwrap();
    for index in 1..=count {
        let prepared = inspect_import(ImportSource::new(
            None,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/java-me/conformance.jar"
            ))
            .to_vec(),
            None,
        ))
        .unwrap()
        .select_midlet(index)
        .unwrap();
        app.entries.push(
            app.repository
                .commit_import(&prepared, GameSettings::default())
                .unwrap(),
        );
    }
    app
}

#[test]
fn library_motion_tracks_import_reimport_and_record_removal() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 2);
    let entry_id = app.entries[0].id().to_owned();
    let id = crate::library_view::launch_id(&entry_id);
    let layout = crate::library_view::LibraryLayout::new(LibraryView::List, 360.0, 8.0);
    let rect = Rect::from_min_size(Pos2::ZERO, layout.item_size);
    let observe = |app: &mut FrontendApp, now| {
        app.library_motion.begin(
            app.entries
                .iter()
                .map(|entry| crate::library_view::launch_id(entry.id())),
            layout,
        );
        let opacity = app.library_motion.item(id, rect, 0, now).1;
        app.library_motion.finish();
        opacity
    };
    assert!(observe(&mut app, 0.0) < 0.1);
    let prepared = inspect_import(ImportSource::new(
        None,
        include_bytes!("../../fixtures/java-me/conformance.jar").to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    assert!(app.commit_import(&prepared, GameSettings::default()));
    assert!((observe(&mut app, 1.0) - 1.0).abs() < f32::EPSILON);

    app.execute_data_action(&DataAction {
        entry_ids: vec![entry_id.clone()],
        kind: DataActionKind::EntryRecord,
    });
    assert!(app.display_error.is_none());
    assert_eq!(app.entries.len(), 1);
    app.library_motion.begin(
        app.entries
            .iter()
            .map(|entry| crate::library_view::launch_id(entry.id())),
        layout,
    );
    app.library_motion.finish();
    assert!(app.commit_import(&prepared, GameSettings::default()));
    assert_eq!(app.entries.len(), 2);
    assert!(observe(&mut app, 2.0) < 0.1);
}

#[test]
fn wide_library_keeps_visible_icons_between_frames() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let mut app = library_fixture(&scratch.0, 1);
    let template = serde_json::to_value(&app.entries[0]).unwrap();
    app.entries = (0..200)
        .map(|index| {
            let mut entry = template.clone();
            entry["id"] = format!("{index:064x}").into();
            serde_json::from_value(entry).unwrap()
        })
        .collect();
    let ctx = egui::Context::default();
    app.app_settings.library_view = LibraryView::Tiles;
    app.prepare_library_icons(&ctx, app.entries.len(), true);
    for index in 0..app.entries.len() {
        wait_for_icon(&mut app, &ctx, index);
    }
    let mut texture_ids = HashMap::new();
    for _ in 0..2 {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(3840.0, 2160.0))),
                ..Default::default()
            },
            |ui| app.draw_library(ui),
        )
        .drop_without_applying_deltas();
        assert!(app.icons.contains_key(app.entries[0].id()));
        assert!(app.icons.len() > 64);
        for (id, texture_id) in &texture_ids {
            assert_eq!(app.icons[id].texture.as_ref().unwrap().id(), *texture_id);
        }
        if texture_ids.is_empty() {
            // Give every visible entry a real texture. The following frame must
            // reuse the handles, with no placeholder/reload cycle.
            for (id, icon) in &mut app.icons {
                let texture = ctx.load_texture(
                    id,
                    egui::ColorImage::filled([80, 80], Color32::WHITE),
                    egui::TextureOptions::NEAREST,
                );
                texture_ids.insert(id.clone(), texture.id());
                icon.texture = Some(texture);
            }
        }
    }
    app.prepare_library_icons(&ctx, texture_ids.len(), false);
    assert!(app.icons.is_empty());
    assert_eq!(app.library_icon_edge, 56);
}

#[test]
fn icon_cache_retains_recently_used_textures_and_placeholders() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = library_fixture(root, 2);
    let ctx = egui::Context::default();
    let texture = ctx.load_texture(
        "cached-fixture",
        egui::ColorImage::filled([1, 1], Color32::WHITE),
        egui::TextureOptions::NEAREST,
    );
    let first = app.entries[0].id().to_owned();
    let second = app.entries[1].id().to_owned();
    for texture in [None, Some(texture)] {
        app.icons.clear();
        let now = Instant::now();
        app.icons.insert(
            first.clone(),
            CachedIcon {
                texture: texture.clone(),
                last_used: now.checked_sub(Duration::from_secs(3)).unwrap(),
            },
        );
        for index in 1..64 {
            app.icons.insert(
                format!("offscreen-{index}"),
                CachedIcon {
                    texture: None,
                    last_used: now
                        .checked_sub(Duration::from_secs(if index == 1 { 2 } else { 1 }))
                        .unwrap(),
                },
            );
        }
        // A cache hit must refresh recency even when the icon is a placeholder.
        assert_eq!(
            app.load_library_assets(&ctx, 0)
                .as_ref()
                .map(TextureHandle::id),
            texture.as_ref().map(TextureHandle::id),
        );
        wait_for_icon(&mut app, &ctx, 1);
        assert_eq!(app.icons.len(), 64);
        assert!(app.icons.contains_key(&first));
        assert!(app.icons.contains_key(&second));
        assert!(!app.icons.contains_key("offscreen-1"));
    }
}

fn wait_for_icon(app: &mut FrontendApp, ctx: &egui::Context, index: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !app.icons.contains_key(app.entries[index].id()) {
        assert!(Instant::now() < deadline, "icon loading did not finish");
        app.load_library_assets(ctx, index);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn tiles_keep_bounded_titles_and_separate_touch_targets_even_in_a_narrow_window() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = library_fixture(root, 1);
    for width in [188.0, 240.0, 320.0] {
        let ctx = egui::Context::default();
        apply_material_theme(&ctx, &app.material_theme);
        let mut settings_rect = Rect::NOTHING;
        for _ in 0..2 {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 400.0))),
                    ..Default::default()
                },
                |ui| {
                    let tile_width = ui.available_width();
                    let top = ui.cursor().top();
                    let ids = app.draw_library_tile(ui, 0, tile_width);
                    let launch = ctx.read_response(ids[0]).unwrap();
                    let settings = ctx.read_response(ids[1]).unwrap();
                    assert!(!launch.rect.intersects(settings.rect));
                    assert!(settings.rect.height() >= 48.0);
                    assert!(settings.rect.right() <= ui.max_rect().right() + 0.01);
                    assert!(
                        (ui.min_rect().bottom() - top - 250.0).abs() < 0.01,
                        "{:?}",
                        ui.min_rect()
                    );
                    settings_rect = settings.rect;
                },
            )
            .drop_without_applying_deltas();
        }
        for pressed in [true, false] {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 400.0))),
                    events: vec![
                        Event::PointerMoved(settings_rect.center()),
                        Event::PointerButton {
                            pos: settings_rect.center(),
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    app.draw_library_tile(ui, 0, ui.available_width());
                },
            )
            .drop_without_applying_deltas();
        }
        assert!(matches!(app.screen, Screen::Settings(_)));
        assert_eq!(app.session.state(), SessionState::Idle);
        app.screen = Screen::Library;
    }
}

#[test]
fn auto_reflows_with_width_and_explicit_views_override_it() {
    let scratch = crate::tests::test_storage::Scratch::new();
    let root = &scratch.0;
    let mut app = library_fixture(root, 8);
    let ctx = egui::Context::default();
    apply_material_theme(&ctx, &app.material_theme);
    for (view, width, height, tiles) in [
        (LibraryView::Automatic, 400.0, 900.0, false),
        (LibraryView::Automatic, 800.0, 400.0, true),
        (LibraryView::Automatic, 800.0, 1100.0, true),
        (LibraryView::List, 1200.0, 600.0, false),
        (LibraryView::Tiles, 400.0, 900.0, true),
    ] {
        app.app_settings.library_view = view;
        navigation::paint_library_size(&mut app, &ctx, Vec2::new(width, height), None);
        let first = ctx
            .read_response(crate::library_view::launch_id(app.entries[0].id()))
            .unwrap();
        let second = ctx
            .read_response(crate::library_view::launch_id(app.entries[1].id()))
            .unwrap();
        if tiles {
            assert!((first.rect.top() - second.rect.top()).abs() < 0.01);
            assert!(second.rect.left() > first.rect.right());
        } else {
            assert!(second.rect.top() > first.rect.bottom());
        }
        assert!(first.rect.height() >= 48.0);
        assert!(second.rect.right() <= width);
    }
}

#[test]
fn material_library_card_height_matches_virtualized_row_stride() {
    let theme = MaterialTheme::fallback(PlatformThemeMode::Dark);
    egui::__run_test_ui(|ui| {
        let response = material_library_card_frame(&theme).show(ui, |ui| {
            ui.allocate_exact_size(Vec2::new(240.0, 60.0), egui::Sense::hover());
        });
        assert!((response.response.rect.height() - LIBRARY_ROW_HEIGHT).abs() < f32::EPSILON);
    });
}

#[test]
fn library_launch_target_includes_icon_text_and_the_gap_between_them() {
    let icon = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(64.0, 64.0));
    let text = Rect::from_min_max(Pos2::new(72.0, 0.0), Pos2::new(300.0, 64.0));
    let settings = Rect::from_min_max(Pos2::new(308.0, 10.0), Pos2::new(404.0, 54.0));
    let launch = library_launch_rect(icon, Some(text));

    assert!(launch.contains(icon.center()));
    assert!(launch.contains(text.center()));
    assert!(launch.contains(Pos2::new(68.0, 32.0)));
    assert!(!launch.intersects(settings));
}

#[test]
fn profile_names_are_unique_product_labels_instead_of_catalog_ids() {
    let profiles = launch::builtin_device_profiles().unwrap();
    let options = profiles
        .iter()
        .map(ProfileOption::from_profile)
        .collect::<Vec<_>>();
    let unique = options
        .iter()
        .map(|option| option.display_name.as_str())
        .collect::<std::collections::HashSet<_>>();

    assert_eq!(unique.len(), options.len());
    for (profile, option) in profiles.iter().zip(&options) {
        assert!(!option.display_name.contains(profile.profile_id()));
        assert!(
            option
                .display_name
                .contains(profile.device().manufacturer())
        );
        let (width, height) = profile.canvas_dimensions().unwrap();
        assert!(option.display_name.contains(&format!("{width}×{height}")));
    }

    assert_eq!(
        profile_display_name(&options, "se-jp8-keypad"),
        Some("Sony Ericsson · 240×320 · Keypad phone · G502")
    );
    assert_eq!(
        profile_display_name(&options, "nokia-s40-v2-keypad"),
        Some("Nokia · Series 40 v2 · 128×160 · Keypad phone")
    );
    assert_eq!(
        profile_display_name(&options, "nokia-s40-touch"),
        Some("Nokia · Series 40 v6 · 240×320 · Touch phone")
    );
}

fn english_library_profile_presentation(
    entry: &LibraryEntry,
    options: &[ProfileOption],
    fingerprint: &str,
) -> crate::layout::LibraryProfilePresentation {
    crate::layout::library_profile_presentation(
        entry,
        options,
        fingerprint,
        crate::i18n::Translator(frontend_core::Language::English),
    )
}
