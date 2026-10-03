use super::*;
use crate::{egui, i18n};

fn linear(factor: f32) -> PlatformTextScale {
    let mut points = PlatformTextScale::default().points;
    for point in &mut points {
        *point *= factor;
    }
    PlatformTextScale::from_points(points).unwrap()
}

#[test]
fn invalid_host_text_metrics_are_rejected() {
    for invalid in [f32::NAN, f32::INFINITY, -1.0, 0.0, 2000.0] {
        let mut points = PlatformTextScale::default().points;
        points[15] = invalid;
        assert!(PlatformTextScale::from_points(points).is_none());
    }
    let mut points = PlatformTextScale::default().points;
    points[15] = 14.0;
    assert!(PlatformTextScale::from_points(points).is_none());
}

fn measure(ctx: &egui::Context, scale: PlatformTextScale, size: f32, text: &str) -> egui::Vec2 {
    i18n::install_scaled(ctx, frontend_core::Language::English, scale);
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(457.0, 900.0),
        )),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(3.15);
    ctx.begin_pass(input);
    let result = ctx.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(
                text.to_owned(),
                egui::FontId::proportional(size),
                egui::Color32::WHITE,
            )
            .size()
    });
    assert!((ctx.pixels_per_point() - 3.15).abs() < 0.001);
    assert!((ctx.viewport_rect().width() - 457.0).abs() < 0.01);
    ctx.end_pass().textures_delta.clear();
    result
}

#[test]
fn text_scale_updates_layout_and_fallback_fonts_without_changing_display_scale() {
    let ctx = egui::Context::default();
    for text in [
        "App settings",
        "Настройки",
        "إعدادات اللعبة",
        "ภาษาไทย",
        "语言",
    ] {
        let normal = measure(&ctx, PlatformTextScale::default(), 18.0, text);
        let small = measure(&ctx, linear(0.85), 18.0, text);
        assert!(
            small.x < normal.x && small.y < normal.y,
            "{text}: {small:?} / {normal:?}"
        );
        let expected = measure(&ctx, PlatformTextScale::default(), 15.3, text);
        assert!(
            (small - expected).length() < 0.2,
            "{text}: {small:?} / {expected:?}"
        );
        let restored = measure(&ctx, PlatformTextScale::default(), 18.0, text);
        assert_eq!(restored, normal);
    }
}

#[test]
fn nonlinear_text_sizes_use_the_host_curve_for_layout_and_row_height() {
    let ctx = egui::Context::default();
    let mut points = PlatformTextScale::default().points;
    for point in &mut points {
        *point = if *point <= 18.0 {
            *point * 2.0
        } else {
            36.0 + (*point - 18.0)
        };
    }
    let nonlinear = PlatformTextScale::from_points(points).unwrap();
    for (nominal, actual) in [(12.0, 24.0), (18.0, 36.0), (23.5, 41.5), (26.0, 44.0)] {
        let scaled = measure(&ctx, nonlinear, nominal, "Text عرض");
        let expected = measure(&ctx, PlatformTextScale::default(), actual, "Text عرض");
        assert_eq!(scaled, expected);
    }
}
