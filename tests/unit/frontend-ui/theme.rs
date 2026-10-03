use super::*;
use frontend_core::{AccentColor, AppTheme};

fn host_colors(shade: u8) -> PlatformThemeColors {
    PlatformThemeColors {
        background: [shade; 3],
        surface: [shade; 3],
        surface_container: [shade; 3],
        surface_container_high: [shade; 3],
        surface_container_highest: [shade; 3],
        primary: [shade; 3],
        on_primary: [shade; 3],
        primary_container: [shade; 3],
        on_primary_container: [shade; 3],
        secondary_container: [shade; 3],
        on_secondary_container: [shade; 3],
        on_surface: [shade; 3],
        on_surface_variant: [shade; 3],
        outline: [shade; 3],
        outline_variant: [shade; 3],
        error: [shade; 3],
        error_container: [shade; 3],
        on_error_container: [shade; 3],
    }
}

#[test]
fn system_palettes_and_manual_appearance_are_independent() {
    let mut host = PlatformTheme {
        mode: PlatformThemeMode::Light,
        light_colors: Some(host_colors(210)),
        dark_colors: Some(host_colors(40)),
    };
    for (preference, expected_mode, shade) in [
        (AppTheme::System, PlatformThemeMode::Light, 210),
        (AppTheme::Light, PlatformThemeMode::Light, 210),
        (AppTheme::Dark, PlatformThemeMode::Dark, 40),
        (AppTheme::Oled, PlatformThemeMode::Dark, 40),
    ] {
        let theme = MaterialTheme::resolve(host, preference, AccentColor::System);
        assert_eq!(theme.mode, expected_mode);
        assert_eq!(theme.primary, Color32::from_gray(shade));
        assert_eq!(
            theme.background,
            if preference == AppTheme::Oled {
                Color32::BLACK
            } else {
                Color32::from_gray(shade)
            }
        );
    }
    let manual = MaterialTheme::resolve(host, AppTheme::Dark, AccentColor::Blue);
    assert!(manual.primary.b() > manual.primary.r());
    assert!(manual.background.b() > manual.background.r());
    host.mode = PlatformThemeMode::Dark;
    host.dark_colors = Some(host_colors(70));
    assert_eq!(
        MaterialTheme::resolve(host, AppTheme::Dark, AccentColor::Blue),
        manual
    );
    let automatic = MaterialTheme::resolve(host, AppTheme::System, AccentColor::System);
    assert_eq!(automatic.primary, Color32::from_gray(70));
    host.dark_colors = None;
    assert_eq!(
        MaterialTheme::resolve(host, AppTheme::System, AccentColor::System),
        manual
    );
}

fn contrast(first: Color32, second: Color32) -> f64 {
    let luminance = |color: Color32| {
        let linear = |channel: u8| {
            let value = f64::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    };
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

#[test]
fn built_in_palettes_keep_text_readable_and_oled_surfaces_black() {
    let host = PlatformTheme {
        mode: PlatformThemeMode::Light,
        light_colors: None,
        dark_colors: None,
    };
    for preference in [AppTheme::Light, AppTheme::Dark, AppTheme::Oled] {
        for accent in [
            AccentColor::System,
            AccentColor::Blue,
            AccentColor::Teal,
            AccentColor::Green,
            AccentColor::Amber,
            AccentColor::Orange,
            AccentColor::Red,
            AccentColor::Pink,
            AccentColor::Purple,
        ] {
            let theme = MaterialTheme::resolve(host, preference, accent);
            for (foreground, background) in [
                (theme.on_surface, theme.background),
                (theme.on_surface_variant, theme.surface_container_highest),
                (theme.primary, theme.surface_container),
                (theme.on_primary, theme.primary),
                (theme.on_primary_container, theme.primary_container),
                (theme.on_secondary_container, theme.secondary_container),
                (theme.error, theme.surface_container),
                (theme.on_error_container, theme.error_container),
                (theme.warning, theme.surface_container),
                (theme.on_warning_container, theme.warning_container),
            ] {
                assert!(
                    contrast(foreground, background) >= 4.5,
                    "{preference:?}/{accent:?}: {foreground:?} on {background:?}"
                );
            }
            if preference == AppTheme::Oled {
                assert_eq!(theme.background, Color32::BLACK);
                assert_eq!(theme.surface, Color32::BLACK);
                assert_eq!(theme.surface_container, Color32::BLACK);
                assert!(theme.surface_container_high.r() <= 16);
                let ctx = egui::Context::default();
                apply_material_theme(&ctx, &theme);
                assert_eq!(ctx.global_style().visuals.panel_fill, Color32::BLACK);
                assert_eq!(ctx.theme(), egui::Theme::Dark);
            }
        }
    }
}

struct ThemeBridge {
    updates: Arc<std::sync::Mutex<VecDeque<PlatformTheme>>>,
    modes: Arc<std::sync::Mutex<Vec<PlatformThemeMode>>>,
}

impl PlatformBridge for ThemeBridge {
    fn poll_theme(&mut self) -> Option<Result<PlatformTheme, EmuError>> {
        self.updates.lock().unwrap().pop_front().map(Ok)
    }

    fn set_theme_mode(&mut self, mode: PlatformThemeMode) -> Result<(), EmuError> {
        self.modes.lock().unwrap().push(mode);
        Ok(())
    }

    fn request_document(&mut self, _kind: DocumentKind) -> Result<(), EmuError> {
        unreachable!("appearance tests do not import files")
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }
}

#[test]
fn saved_appearance_survives_theme_events_and_cancel_restores_the_saved_choice() {
    let scratch = test_storage::Scratch::new();
    let root = &scratch.0;
    let updates = Arc::new(std::sync::Mutex::new(VecDeque::from([PlatformTheme {
        mode: PlatformThemeMode::Light,
        light_colors: Some(host_colors(210)),
        dark_colors: Some(host_colors(40)),
    }])));
    let modes = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut app = FrontendApp::new(
        root,
        Box::new(ThemeBridge {
            updates: updates.clone(),
            modes: modes.clone(),
        }),
    )
    .unwrap();
    let ctx = egui::Context::default();
    app.process_platform_theme(&ctx);
    let system = app.material_theme;
    let mut draft = app.app_settings.clone();
    draft.theme = AppTheme::Oled;
    draft.accent_color = AccentColor::Green;
    app.screen = Screen::AppSettings(draft.clone().into());
    app.process_platform_theme(&ctx);
    assert_eq!(app.material_theme, system);
    app.cancel_settings_screen();
    app.process_platform_theme(&ctx);
    assert_eq!(app.material_theme, system);
    app.screen = Screen::AppSettings(draft.into());
    app.save_settings_screen();
    app.process_platform_theme(&ctx);
    let saved = app.material_theme;
    assert_eq!(saved.background, Color32::BLACK);
    assert_eq!(saved.mode, PlatformThemeMode::Dark);
    updates.lock().unwrap().push_back(PlatformTheme {
        mode: PlatformThemeMode::Light,
        light_colors: Some(host_colors(220)),
        dark_colors: Some(host_colors(50)),
    });
    app.process_platform_theme(&ctx);
    assert_eq!(app.material_theme, saved);
    let mut draft = app.app_settings.clone();
    draft.theme = AppTheme::System;
    draft.accent_color = AccentColor::System;
    app.screen = Screen::AppSettings(draft.into());
    app.save_settings_screen();
    app.process_platform_theme(&ctx);
    assert_eq!(app.material_theme.primary, Color32::from_gray(220));
    assert_eq!(
        *modes.lock().unwrap(),
        [
            PlatformThemeMode::Light,
            PlatformThemeMode::Dark,
            PlatformThemeMode::Light
        ]
    );
    drop(app);
}

#[test]
fn material_theme_keeps_android_touch_targets_and_surface_roles_explicit() {
    let ctx = egui::Context::default();
    let theme = MaterialTheme::fallback(PlatformThemeMode::Dark);
    apply_material_theme(&ctx, &theme);
    let style = ctx.global_style();

    assert_eq!(ctx.theme(), egui::Theme::Dark);
    assert_eq!(style.visuals.panel_fill, theme.background);
    assert_eq!(style.visuals.window_fill, theme.surface_container_high);
    assert_eq!(style.visuals.selection.bg_fill, theme.primary_container);
    assert!(style.spacing.interact_size.y >= SETTINGS_TOUCH_TARGET_HEIGHT);
    apply_material_theme(&ctx, &theme);
    assert!(Arc::ptr_eq(&style, &ctx.global_style()));

    egui::__run_test_ui(|ui| {
        let response = ui.add(material_primary_button(&theme, "Continue"));
        assert!(response.rect.height() >= SETTINGS_TOUCH_TARGET_HEIGHT);
    });
}

#[test]
fn settings_sliders_use_the_elevated_container_instead_of_the_black_surface() {
    let mut theme = MaterialTheme::fallback(PlatformThemeMode::Dark);
    theme.surface = Color32::BLACK;
    theme.surface_container_high = Color32::from_rgb(36, 36, 36);

    let frame = material_settings_slider_frame(&theme);
    assert_eq!(frame.fill, theme.surface_container_high);
    assert_ne!(frame.fill, theme.surface);
}

#[test]
fn material_theme_applies_light_and_platform_monochrome_schemes() {
    let light = MaterialTheme::fallback(PlatformThemeMode::Light);
    let ctx = egui::Context::default();
    apply_material_theme(&ctx, &light);
    assert_eq!(ctx.theme(), egui::Theme::Light);
    assert_eq!(ctx.global_style().visuals.panel_fill, light.background);

    let shades = (0_u8..18)
        .map(|index| [index.saturating_mul(11); 3])
        .collect::<Vec<_>>();
    let platform = PlatformTheme {
        mode: PlatformThemeMode::Dark,
        light_colors: None,
        dark_colors: Some(PlatformThemeColors {
            background: shades[0],
            surface: shades[1],
            surface_container: shades[2],
            surface_container_high: shades[3],
            surface_container_highest: shades[4],
            primary: shades[5],
            on_primary: shades[6],
            primary_container: shades[7],
            on_primary_container: shades[8],
            secondary_container: shades[9],
            on_secondary_container: shades[10],
            on_surface: shades[11],
            on_surface_variant: shades[12],
            outline: shades[13],
            outline_variant: shades[14],
            error: shades[15],
            error_container: shades[16],
            on_error_container: shades[17],
        }),
    };
    let monochrome = MaterialTheme::from_platform(platform, platform.mode);
    assert_eq!(monochrome.primary, Color32::from_rgb(55, 55, 55));
    assert_eq!(monochrome.on_surface, Color32::from_rgb(121, 121, 121));
    assert_eq!(monochrome.error_container, Color32::from_rgb(176, 176, 176));
    apply_material_theme(&ctx, &monochrome);
    assert_eq!(ctx.theme(), egui::Theme::Dark);
    assert_eq!(ctx.global_style().visuals.panel_fill, monochrome.background);
    assert_eq!(ctx.global_style().visuals.error_fg_color, monochrome.error);

    let mut changed = monochrome;
    changed.primary_container = Color32::from_rgb(32, 64, 96);
    apply_material_theme(&ctx, &changed);
    let style = ctx.global_style();
    assert_eq!(style.visuals.selection.bg_fill, changed.primary_container);
    apply_material_theme(&ctx, &changed);
    assert!(Arc::ptr_eq(&style, &ctx.global_style()));

    let recreated = egui::Context::default();
    apply_material_theme(&recreated, &changed);
    let recreated_style = recreated.global_style();
    assert_eq!(recreated_style.visuals, style.visuals);
    assert_eq!(recreated_style.spacing, style.spacing);
    assert_eq!(recreated_style.text_styles, style.text_styles);
    apply_material_theme(&ctx, &light);
    assert_eq!(ctx.theme(), egui::Theme::Light);
    assert_eq!(ctx.global_style().visuals.panel_fill, light.background);
}
