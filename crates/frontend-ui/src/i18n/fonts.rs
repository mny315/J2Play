use crate::egui;
use frontend_core::Language;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct InstalledFonts {
    scale: crate::PlatformTextScale,
    pass: u64,
}

pub(super) fn install(ctx: &egui::Context, scale: crate::PlatformTextScale) {
    let key = egui::Id::new("product-fonts");
    if ctx
        .data(|data| data.get_temp::<InstalledFonts>(key))
        .is_some_and(|installed| installed.scale == scale)
    {
        return;
    }
    let mut fonts = egui::FontDefinitions::default();
    let bundled: [(&str, &[u8]); 5] = [
        (
            "Noto Sans",
            include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"),
        ),
        (
            "Noto Arabic",
            include_bytes!("../../assets/fonts/NotoSansArabic-Regular.ttf"),
        ),
        (
            "Noto Devanagari",
            include_bytes!("../../assets/fonts/NotoSansDevanagari-Regular.ttf"),
        ),
        (
            "Noto Thai",
            include_bytes!("../../assets/fonts/NotoSansThai-Regular.ttf"),
        ),
        (
            "J2Play Chinese",
            include_bytes!("../../assets/fonts/J2PlayChinese-Regular.otf"),
        ),
    ];
    for (name, bytes) in bundled {
        fonts.font_data.insert(
            name.to_owned(),
            Arc::new(egui::FontData::from_static(bytes)),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push(name.to_owned());
        }
    }
    let size_map = scale.font_size_map();
    for font in fonts.font_data.values_mut() {
        Arc::make_mut(font).tweak.size_map.clone_from(&size_map);
    }
    ctx.set_fonts(fonts);
    let installed = InstalledFonts {
        scale,
        pass: ctx.cumulative_pass_nr(),
    };
    ctx.data_mut(|data| {
        data.insert_temp(key, installed);
        data.remove::<MenuWarmup>(egui::Id::new("language-menu-fonts"));
    });
    ctx.request_repaint();
}

#[derive(Clone, Copy)]
struct MenuWarmup {
    pixels_per_point: f32,
    next: usize,
    pass: u64,
}

/// Rasterize one language name per painted frame, before a user opens the menu.
/// The first mixed-script popup must not populate every fallback font at once.
pub(crate) fn warm_language_menu(ctx: &egui::Context) {
    let pass = ctx.cumulative_pass_nr();
    if ctx
        .data(|data| data.get_temp::<InstalledFonts>(egui::Id::new("product-fonts")))
        .is_none_or(|installed| pass <= installed.pass)
    {
        return;
    }
    let key = egui::Id::new("language-menu-fonts");
    let pixels_per_point = ctx.pixels_per_point();
    let mut state = ctx
        .data(|data| data.get_temp::<MenuWarmup>(key))
        .filter(|state| state.pixels_per_point.to_bits() == pixels_per_point.to_bits())
        .unwrap_or(MenuWarmup {
            pixels_per_point,
            next: 0,
            pass: u64::MAX,
        });
    if state.pass == pass {
        return;
    }
    let Some(language) = Language::ALL.get(state.next) else {
        return;
    };
    ctx.fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            language.native_name().to_owned(),
            egui::FontId::proportional(crate::SETTINGS_BODY_TEXT_SIZE),
            egui::Color32::WHITE,
        );
    });
    state.next += 1;
    state.pass = pass;
    ctx.data_mut(|data| data.insert_temp(key, state));
    if state.next < Language::ALL.len() {
        ctx.request_repaint();
    }
}
