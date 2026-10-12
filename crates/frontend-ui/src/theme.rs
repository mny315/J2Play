use super::{Color32, RichText, Vec2, egui};
use frontend_core::{AccentColor, AppTheme};

mod palette;

/// The platform's current light/dark appearance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformThemeMode {
    Light,
    Dark,
}

/// Semantic Material colors supplied by the host.
///
/// Android 12 and newer populate these roles from the system dynamic-color
/// palette. A monochrome system palette therefore remains monochrome here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformThemeColors {
    pub background: [u8; 3],
    pub surface: [u8; 3],
    pub surface_container: [u8; 3],
    pub surface_container_high: [u8; 3],
    pub surface_container_highest: [u8; 3],
    pub primary: [u8; 3],
    pub on_primary: [u8; 3],
    pub primary_container: [u8; 3],
    pub on_primary_container: [u8; 3],
    pub secondary_container: [u8; 3],
    pub on_secondary_container: [u8; 3],
    pub on_surface: [u8; 3],
    pub on_surface_variant: [u8; 3],
    pub outline: [u8; 3],
    pub outline_variant: [u8; 3],
    pub error: [u8; 3],
    pub error_container: [u8; 3],
    pub on_error_container: [u8; 3],
}

/// Host preference and palettes for both modes, so an app override can retain
/// dynamic colors even when its appearance differs from the system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformTheme {
    pub mode: PlatformThemeMode,
    pub light_colors: Option<PlatformThemeColors>,
    pub dark_colors: Option<PlatformThemeColors>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MaterialTheme {
    pub mode: PlatformThemeMode,
    pub background: Color32,
    pub surface: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub error: Color32,
    pub error_container: Color32,
    pub on_error_container: Color32,
    pub warning: Color32,
    pub warning_container: Color32,
    pub on_warning_container: Color32,
}

const MATERIAL_WIDGET_RADIUS: u8 = 12;
const MATERIAL_CARD_RADIUS: u8 = 18;
const MATERIAL_BUTTON_RADIUS: u8 = 24;

impl MaterialTheme {
    pub(super) fn fallback(mode: PlatformThemeMode) -> Self {
        Self::preset(mode, AccentColor::Blue)
    }

    fn neutral(mode: PlatformThemeMode) -> Self {
        match mode {
            PlatformThemeMode::Light => Self {
                mode,
                background: Color32::from_gray(250),
                surface: Color32::from_gray(250),
                surface_container: Color32::from_gray(242),
                surface_container_high: Color32::from_gray(235),
                surface_container_highest: Color32::from_gray(227),
                primary: Color32::BLACK,
                on_primary: Color32::WHITE,
                primary_container: Color32::from_gray(230),
                on_primary_container: Color32::from_gray(25),
                secondary_container: Color32::from_gray(230),
                on_secondary_container: Color32::from_gray(25),
                on_surface: Color32::from_gray(28),
                on_surface_variant: Color32::from_gray(70),
                outline: Color32::from_gray(118),
                outline_variant: Color32::from_gray(198),
                error: Color32::from_rgb(179, 38, 30),
                error_container: Color32::from_rgb(249, 222, 220),
                on_error_container: Color32::from_rgb(65, 14, 11),
                warning: Color32::from_rgb(117, 83, 0),
                warning_container: Color32::from_rgb(255, 223, 158),
                on_warning_container: Color32::from_rgb(38, 26, 0),
            },
            PlatformThemeMode::Dark => Self {
                mode,
                background: Color32::from_gray(18),
                surface: Color32::from_gray(25),
                surface_container: Color32::from_gray(30),
                surface_container_high: Color32::from_gray(40),
                surface_container_highest: Color32::from_gray(51),
                primary: Color32::WHITE,
                on_primary: Color32::from_gray(20),
                primary_container: Color32::from_gray(50),
                on_primary_container: Color32::from_gray(235),
                secondary_container: Color32::from_gray(50),
                on_secondary_container: Color32::from_gray(235),
                on_surface: Color32::from_gray(230),
                on_surface_variant: Color32::from_gray(198),
                outline: Color32::from_gray(148),
                outline_variant: Color32::from_gray(72),
                error: Color32::from_rgb(255, 180, 171),
                error_container: Color32::from_rgb(147, 0, 10),
                on_error_container: Color32::from_rgb(255, 218, 214),
                warning: Color32::from_rgb(255, 184, 108),
                warning_container: Color32::from_rgb(55, 42, 25),
                on_warning_container: Color32::from_rgb(255, 220, 178),
            },
        }
    }

    pub(super) fn from_platform(theme: PlatformTheme, mode: PlatformThemeMode) -> Self {
        let fallback = Self::fallback(mode);
        let colors = match mode {
            PlatformThemeMode::Light => theme.light_colors,
            PlatformThemeMode::Dark => theme.dark_colors,
        };
        let Some(colors) = colors else {
            return fallback;
        };
        Self {
            mode,
            background: rgb(colors.background),
            surface: rgb(colors.surface),
            surface_container: rgb(colors.surface_container),
            surface_container_high: rgb(colors.surface_container_high),
            surface_container_highest: rgb(colors.surface_container_highest),
            primary: rgb(colors.primary),
            on_primary: rgb(colors.on_primary),
            primary_container: rgb(colors.primary_container),
            on_primary_container: rgb(colors.on_primary_container),
            secondary_container: rgb(colors.secondary_container),
            on_secondary_container: rgb(colors.on_secondary_container),
            on_surface: rgb(colors.on_surface),
            on_surface_variant: rgb(colors.on_surface_variant),
            outline: rgb(colors.outline),
            outline_variant: rgb(colors.outline_variant),
            error: rgb(colors.error),
            error_container: rgb(colors.error_container),
            on_error_container: rgb(colors.on_error_container),
            ..fallback
        }
    }

    pub(super) fn resolve(
        platform: PlatformTheme,
        preference: AppTheme,
        accent: AccentColor,
    ) -> Self {
        let mode = match preference {
            AppTheme::System => platform.mode,
            AppTheme::Light => PlatformThemeMode::Light,
            AppTheme::Dark | AppTheme::Oled => PlatformThemeMode::Dark,
        };
        let mut theme = if accent == AccentColor::System {
            Self::from_platform(platform, mode)
        } else {
            Self::preset(mode, accent)
        };
        if preference == AppTheme::Oled {
            theme.background = Color32::BLACK;
            theme.surface = Color32::BLACK;
            theme.surface_container = Color32::BLACK;
            theme.surface_container_high = Color32::from_gray(12);
            theme.surface_container_highest = Color32::from_gray(24);
        }
        theme
    }
}

const fn rgb(value: [u8; 3]) -> Color32 {
    Color32::from_rgb(value[0], value[1], value[2])
}

pub(super) fn apply_material_theme(ctx: &egui::Context, theme: &MaterialTheme) {
    // The cache belongs to the egui context, so recreated windows still receive
    // their full style while ordinary logic ticks reuse the existing allocation.
    let applied = egui::Id::new("j2play-material-theme");
    if ctx.data(|data| data.get_temp::<MaterialTheme>(applied)) == Some(*theme) {
        return;
    }
    let egui_theme = match theme.mode {
        PlatformThemeMode::Light => egui::Theme::Light,
        PlatformThemeMode::Dark => egui::Theme::Dark,
    };
    ctx.set_theme(egui_theme);
    let mut style = (*ctx.global_style()).clone();
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    // Keep the focus outline and its antialiasing inside every scroll viewport.
    style.spacing.scroll.content_margin = egui::Margin::same(super::focus_ring::OUTSET + 1);
    style.spacing.window_margin = egui::Margin::same(16);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.button_padding = Vec2::new(12.0, 8.0);
    style.spacing.interact_size = Vec2::new(48.0, 48.0);
    style.spacing.slider_rail_height = 4.0;
    style.animation_time = super::ui_motion::UI_MOTION_SECONDS;

    let mut visuals = egui_theme.default_visuals();
    visuals.override_text_color = Some(theme.on_surface);
    visuals.weak_text_color = Some(theme.on_surface_variant);
    visuals.panel_fill = theme.background;
    visuals.window_fill = theme.surface_container_high;
    visuals.window_stroke = egui::Stroke::new(1.0, theme.outline_variant);
    visuals.window_corner_radius = MATERIAL_CARD_RADIUS.into();
    let shadow_alpha: u8 = match theme.mode {
        PlatformThemeMode::Light => 48,
        PlatformThemeMode::Dark => 120,
    };
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 2,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 1,
        color: Color32::from_black_alpha(shadow_alpha.saturating_sub(10)),
    };
    visuals.menu_corner_radius = MATERIAL_WIDGET_RADIUS.into();
    visuals.extreme_bg_color = theme.surface;
    visuals.text_edit_bg_color = Some(theme.surface_container_highest);
    visuals.code_bg_color = theme.surface;
    visuals.faint_bg_color = theme.surface_container;
    visuals.hyperlink_color = theme.primary;
    visuals.warn_fg_color = theme.warning;
    visuals.error_fg_color = theme.error;
    visuals.selection.bg_fill = theme.primary_container;
    visuals.selection.stroke = egui::Stroke::new(1.5, theme.on_primary_container);
    visuals.slider_trailing_fill = true;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    visuals.widgets.noninteractive = widget_visuals(
        theme.surface_container,
        theme.outline_variant,
        theme.on_surface,
        0.0,
    );
    visuals.widgets.inactive = widget_visuals(
        theme.surface_container_high,
        theme.outline_variant,
        theme.on_surface,
        0.0,
    );
    visuals.widgets.hovered = widget_visuals(
        theme.surface_container_highest,
        theme.primary,
        theme.on_surface,
        1.0,
    );
    visuals.widgets.active = widget_visuals(
        theme.primary_container,
        theme.primary,
        theme.on_primary_container,
        0.0,
    );
    visuals.widgets.open = visuals.widgets.active;
    style.visuals = visuals;
    ctx.set_global_style(style);
    ctx.data_mut(|data| data.insert_temp(applied, *theme));
}

fn widget_visuals(
    background: Color32,
    outline: Color32,
    foreground: Color32,
    expansion: f32,
) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: background,
        weak_bg_fill: background,
        bg_stroke: egui::Stroke::new(1.0, outline),
        corner_radius: MATERIAL_WIDGET_RADIUS.into(),
        fg_stroke: egui::Stroke::new(1.5, foreground),
        expansion,
    }
}

pub(super) fn material_app_bar_frame(theme: &MaterialTheme) -> egui::Frame {
    material_card_frame(theme).inner_margin(egui::Margin::symmetric(12, 6))
}

pub(super) fn material_gameplay_app_bar_frame(theme: &MaterialTheme) -> egui::Frame {
    material_card_frame(theme).inner_margin(egui::Margin::symmetric(12, 6))
}

pub(super) fn material_card_frame(theme: &MaterialTheme) -> egui::Frame {
    egui::Frame::new()
        .fill(theme.surface_container)
        .stroke(egui::Stroke::new(1.0, theme.outline_variant))
        .corner_radius(MATERIAL_CARD_RADIUS)
        .inner_margin(egui::Margin::same(12))
}

pub(super) fn material_settings_slider_frame(theme: &MaterialTheme) -> egui::Frame {
    egui::Frame::new()
        .fill(theme.surface_container_high)
        .corner_radius(MATERIAL_WIDGET_RADIUS)
        .inner_margin(egui::Margin::symmetric(12, 4))
}

pub(super) fn material_library_card_frame(theme: &MaterialTheme) -> egui::Frame {
    material_card_frame(theme)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .outer_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 0,
            bottom: 2,
        })
}

pub(super) fn material_bottom_bar_frame(theme: &MaterialTheme) -> egui::Frame {
    egui::Frame::new()
        .fill(theme.surface_container)
        .stroke(egui::Stroke::new(1.0, theme.outline_variant))
        .inner_margin(egui::Margin::symmetric(12, 6))
}

pub(super) fn material_primary_button(
    theme: &MaterialTheme,
    label: impl Into<String>,
) -> super::focus_ring::FocusButton {
    egui::Button::new(RichText::new(label.into()).color(theme.on_primary).strong())
        .fill(theme.primary)
        .stroke(egui::Stroke::NONE)
        .corner_radius(MATERIAL_BUTTON_RADIUS)
        .min_size(Vec2::new(0.0, 48.0))
        .into()
}

pub(super) fn material_tonal_button(
    theme: &MaterialTheme,
    label: impl Into<String>,
) -> super::focus_ring::FocusButton {
    egui::Button::new(
        RichText::new(label.into())
            .color(theme.on_secondary_container)
            .strong(),
    )
    .fill(theme.secondary_container)
    .stroke(egui::Stroke::NONE)
    .corner_radius(MATERIAL_BUTTON_RADIUS)
    .min_size(Vec2::new(0.0, 48.0))
    .into()
}

pub(super) fn material_choice_button(
    label: impl Into<String>,
    selected: bool,
) -> super::focus_ring::FocusButton {
    egui::Button::new(RichText::new(label.into()).strong())
        .selected(selected)
        .corner_radius(MATERIAL_BUTTON_RADIUS)
        .min_size(Vec2::new(0.0, 48.0))
        .into()
}

pub(super) fn material_outlined_button(
    theme: &MaterialTheme,
    label: impl Into<String>,
) -> super::focus_ring::FocusButton {
    egui::Button::new(RichText::new(label.into()).color(theme.primary).strong())
        .fill(theme.surface_container)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(MATERIAL_BUTTON_RADIUS)
        .min_size(Vec2::new(0.0, 48.0))
        .into()
}

pub(super) fn material_text_button(
    theme: &MaterialTheme,
    label: impl Into<String>,
) -> super::focus_ring::FocusButton {
    egui::Button::new(RichText::new(label.into()).color(theme.primary).strong())
        .frame(false)
        .corner_radius(MATERIAL_BUTTON_RADIUS)
        .min_size(Vec2::new(0.0, 48.0))
        .into()
}

pub(super) fn material_danger_button(
    theme: &MaterialTheme,
    label: impl Into<String>,
) -> super::focus_ring::FocusButton {
    egui::Button::new(
        RichText::new(label.into())
            .color(theme.on_error_container)
            .strong(),
    )
    .fill(theme.error_container)
    .stroke(egui::Stroke::NONE)
    .corner_radius(MATERIAL_BUTTON_RADIUS)
    .min_size(Vec2::new(0.0, 48.0))
    .into()
}

pub(super) fn material_error_outlined_button(
    theme: &MaterialTheme,
    label: impl Into<String>,
) -> super::focus_ring::FocusButton {
    egui::Button::new(RichText::new(label.into()).color(theme.error).strong())
        .fill(theme.surface_container)
        .stroke(egui::Stroke::new(1.0, theme.error))
        .corner_radius(MATERIAL_BUTTON_RADIUS)
        .min_size(Vec2::new(0.0, 48.0))
        .into()
}

pub(super) fn material_supporting_text(theme: &MaterialTheme, text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(14.0)
        .color(theme.on_surface_variant)
}

pub(super) fn paint_material_scrim(ui: &egui::Ui, rect: egui::Rect, visible: bool) {
    let opacity = ui.ctx().animate_bool_with_time(
        egui::Id::new("material-dialog-scrim"),
        visible,
        super::ui_motion::UI_MOTION_SECONDS,
    );
    if opacity > 0.0 {
        ui.painter().rect_filled(
            rect,
            0.0,
            Color32::from_black_alpha(112).gamma_multiply(opacity),
        );
    }
}
