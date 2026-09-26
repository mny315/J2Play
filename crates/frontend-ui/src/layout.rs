use super::{
    Color32, FULLSCREEN_EXIT_GESTURE_FRACTION, FULLSCREEN_EXIT_GESTURE_MAX_HEIGHT,
    FULLSCREEN_EXIT_GESTURE_MIN_HEIGHT, GAMEPLAY_CONTENT_LEFT_MARGIN, GAMEPLAY_CONTENT_MARGIN,
    GAMEPLAY_MIN_CONTROLS_HEIGHT, GAMEPLAY_MIN_FRAME_HEIGHT, GAMEPLAY_SECTION_GAP, GameScale,
    GameplayLayout, Instant, KEYPAD_REFERENCE_LANDSCAPE, KEYPAD_REFERENCE_PORTRAIT, LibraryEntry,
    MaterialTheme, PlatformInsets, Pos2, ProfileChoice, ProfileOption, Rect, RichText,
    SETTINGS_BODY_TEXT_SIZE, SETTINGS_HEADING_TEXT_SIZE, SETTINGS_SLIDER_VALUE_RESERVE,
    SETTINGS_TOUCH_TARGET_HEIGHT, SLIDER_HAPTIC_MIN_INTERVAL, TextureHandle, Vec2, egui,
};

pub(super) fn gameplay_panel_margin(fullscreen: bool) -> egui::Margin {
    if fullscreen {
        egui::Margin::ZERO
    } else {
        egui::Margin {
            left: GAMEPLAY_CONTENT_LEFT_MARGIN,
            right: GAMEPLAY_CONTENT_MARGIN,
            top: GAMEPLAY_CONTENT_MARGIN,
            bottom: GAMEPLAY_CONTENT_MARGIN,
        }
    }
}

pub(super) fn fullscreen_exit_gesture_rect(rect: Rect) -> Rect {
    let height = (rect.height() * FULLSCREEN_EXIT_GESTURE_FRACTION)
        .clamp(
            FULLSCREEN_EXIT_GESTURE_MIN_HEIGHT,
            FULLSCREEN_EXIT_GESTURE_MAX_HEIGHT,
        )
        .min(rect.height());
    Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + height))
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn inset_content_rect(
    rect: Rect,
    insets: PlatformInsets,
    pixels_per_point: f32,
) -> Rect {
    let pixels_per_point = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
        pixels_per_point
    } else {
        1.0
    };
    let left = (insets.left as f32 / pixels_per_point).clamp(0.0, rect.width());
    let right = (insets.right as f32 / pixels_per_point).clamp(0.0, (rect.width() - left).max(0.0));
    let top = (insets.top as f32 / pixels_per_point).clamp(0.0, rect.height());
    let bottom =
        (insets.bottom as f32 / pixels_per_point).clamp(0.0, (rect.height() - top).max(0.0));
    Rect::from_min_max(
        Pos2::new(rect.left() + left, rect.top() + top),
        Pos2::new(rect.right() - right, rect.bottom() - bottom),
    )
}

pub(super) fn frame_scale_for_fit(game_scale: GameScale, fit: f32) -> f32 {
    let fit = fit.max(0.0);
    match game_scale {
        GameScale::AutomaticFit => fit,
        GameScale::Manual { percent } => (f32::from(percent) / 100.0).min(fit),
    }
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn game_canvas_rect(
    available: Rect,
    (width, height): (u32, u32),
    game_scale: GameScale,
) -> Rect {
    if width == 0 || height == 0 {
        return Rect::from_center_size(available.center(), Vec2::ZERO);
    }
    let fit = (available.width() / width as f32)
        .min(available.height() / height as f32)
        .max(0.0);
    let scale = frame_scale_for_fit(game_scale, fit);
    Rect::from_center_size(
        available.center(),
        Vec2::new(width as f32, height as f32) * scale,
    )
}

pub(super) fn keypad_slot_dimensions(frame_dimensions: (u32, u32)) -> (u32, u32) {
    let (width, height) = frame_dimensions;
    if width == 0 || height == 0 {
        return frame_dimensions;
    }
    let reference = if width > height {
        KEYPAD_REFERENCE_LANDSCAPE
    } else {
        KEYPAD_REFERENCE_PORTRAIT
    };
    (width.max(reference.0), height.max(reference.1))
}

impl GameplayLayout {
    pub(super) fn allocate_game_area(self, ui: &mut egui::Ui) -> Rect {
        let (slot, _) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                self.game_top_padding + self.game_height,
            ),
            egui::Sense::hover(),
        );
        Rect::from_min_max(slot.min + Vec2::new(0.0, self.game_top_padding), slot.max)
    }
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn gameplay_layout(
    available: Vec2,
    frame_dimensions: (u32, u32),
    pointer_events: bool,
    landscape: bool,
) -> GameplayLayout {
    let available_width = available.x.max(0.0);
    let available_height = available.y.max(0.0);
    if pointer_events {
        return GameplayLayout {
            game_top_padding: 0.0,
            game_height: available_height,
            controls_height: 0.0,
            gap: 0.0,
            controls_overlay: false,
        };
    }

    let gap = GAMEPLAY_SECTION_GAP.min(available_height);
    let content_height = (available_height - gap).max(0.0);
    let minimum_frame_height = GAMEPLAY_MIN_FRAME_HEIGHT.min(content_height);
    let minimum_controls_height = GAMEPLAY_MIN_CONTROLS_HEIGHT.min(content_height);
    let maximum_frame_height = (content_height - minimum_controls_height)
        .max(minimum_frame_height)
        .min(content_height);
    let (frame_width, frame_height) = keypad_slot_dimensions(frame_dimensions);
    let game_height = if frame_width == 0 || frame_height == 0 || available_width == 0.0 {
        maximum_frame_height
    } else {
        let fit = (available_width / frame_width as f32)
            .min(maximum_frame_height / frame_height as f32)
            .max(0.01);
        (frame_height as f32 * fit).clamp(minimum_frame_height, maximum_frame_height)
    };

    let controls_height = (content_height - game_height).max(0.0);
    // Side controls need enough space around the fitted Canvas. Wide guest
    // screens and narrow windows use the separate keypad area instead.
    let controls_overlay = landscape
        && super::controls::landscape_controls_fit(
            Rect::from_min_size(
                Pos2::new(0.0, available_height - controls_height),
                Vec2::new(available_width, controls_height),
            ),
            game_canvas_rect(
                Rect::from_min_size(Pos2::ZERO, available),
                frame_dimensions,
                GameScale::AutomaticFit,
            ),
        );
    GameplayLayout {
        game_top_padding: 0.0,
        game_height: if controls_overlay {
            available_height
        } else {
            game_height
        },
        controls_height,
        gap: if controls_overlay { 0.0 } else { gap },
        controls_overlay,
    }
}

pub(super) fn slider_haptic_due(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|last| now.saturating_duration_since(last) >= SLIDER_HAPTIC_MIN_INTERVAL)
}

pub(super) fn settings_slider_width(available_width: f32) -> f32 {
    (available_width - SETTINGS_SLIDER_VALUE_RESERVE).max(SETTINGS_TOUCH_TARGET_HEIGHT)
}

pub(super) fn draw_library_icon(
    ui: &mut egui::Ui,
    icon: Option<&TextureHandle>,
    theme: &MaterialTheme,
) -> Rect {
    if let Some(texture) = icon {
        return ui
            .add(
                egui::Image::new(texture)
                    .fit_to_exact_size(Vec2::splat(super::library_icons::LIST_ICON_POINTS))
                    .corner_radius(14)
                    .sense(egui::Sense::hover()),
            )
            .rect;
    }
    let (rect, _) = ui.allocate_exact_size(
        Vec2::splat(super::library_icons::LIST_ICON_POINTS),
        egui::Sense::hover(),
    );
    paint_library_icon(ui, icon, rect, theme);
    rect
}

pub(super) fn paint_library_icon(
    ui: &egui::Ui,
    icon: Option<&TextureHandle>,
    rect: Rect,
    theme: &MaterialTheme,
) {
    if let Some(texture) = icon {
        let source = texture.size_vec2();
        let scale = (rect.width() / source.x).min(rect.height() / source.y);
        let rect = Rect::from_center_size(rect.center(), source * scale);
        egui::Image::new(texture)
            .fit_to_exact_size(rect.size())
            .corner_radius(14)
            .paint_at(ui, rect);
        return;
    }
    ui.painter()
        .rect_filled(rect, 14.0, theme.primary_container);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "J2",
        egui::FontId::proportional(22.0),
        theme.on_surface,
    );
}

pub(super) fn draw_library_text(
    ui: &mut egui::Ui,
    title: &str,
    game_info: &str,
    profile: &LibraryProfilePresentation,
    width: f32,
    theme: &MaterialTheme,
) -> Option<Rect> {
    if width <= 0.0 {
        return None;
    }
    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(width, 60.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_min_size(Vec2::new(width, 60.0));
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.spacing_mut().interact_size.y = 18.0;
                ui.add_space(if game_info.is_empty() { 9.0 } else { 2.0 });
                ui.add_sized(
                    [width, 20.0],
                    egui::Label::new(RichText::new(title).size(16.0).strong())
                        .selectable(false)
                        .truncate()
                        .halign(egui::Align::Min),
                );
                if !game_info.is_empty() {
                    ui.add_sized(
                        [width, 18.0],
                        egui::Label::new(
                            RichText::new(game_info)
                                .size(13.0)
                                .color(theme.on_surface_variant),
                        )
                        .selectable(false)
                        .truncate()
                        .halign(egui::Align::Min),
                    );
                }
                draw_library_profile(ui, profile, width, theme);
            },
        )
        .response;
    Some(response.rect)
}

pub(super) fn draw_library_profile(
    ui: &mut egui::Ui,
    profile: &LibraryProfilePresentation,
    width: f32,
    theme: &MaterialTheme,
) {
    ui.horizontal(|ui| {
        let color = theme.on_surface_variant;
        let icon_width = if let Some(icon) = profile.icon {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(18.0_f32.min(width), 18.0), egui::Sense::hover());
            icon.paint(
                &ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())),
                rect,
                color,
            );
            rect.width()
        } else {
            0.0
        };
        if width > icon_width {
            ui.add_sized(
                [width - icon_width, 18.0],
                egui::Label::new(RichText::new(&profile.label).size(12.0).color(color))
                    .selectable(false)
                    .truncate()
                    .halign(egui::Align::Min),
            );
        }
    });
}

pub(super) fn library_game_info(entry: &LibraryEntry) -> String {
    match (entry.vendor(), entry.release_year()) {
        (Some(vendor), Some(year)) => format!("{year} · {vendor}"),
        (Some(vendor), None) => vendor.to_owned(),
        (None, Some(year)) => year.to_string(),
        (None, None) => String::new(),
    }
}

pub(super) fn library_launch_rect(icon_rect: Rect, text_rect: Option<Rect>) -> Rect {
    text_rect.map_or(icon_rect, |text_rect| icon_rect.union(text_rect))
}

pub(super) fn profile_display_name<'a>(
    options: &'a [ProfileOption],
    profile_id: &str,
) -> Option<&'a str> {
    options
        .iter()
        .find(|option| option.profile_id == profile_id)
        .map(|option| option.display_name.as_str())
}

pub(super) struct LibraryProfilePresentation {
    pub(super) label: String,
    pub(super) description: String,
    pub(super) icon: Option<LibraryProfileIcon>,
}

impl LibraryProfilePresentation {
    pub(super) fn launch_description(
        &self,
        title: &str,
        game_info: &str,
        tr: crate::i18n::Translator,
    ) -> String {
        format!(
            "{}\n{game_info}\n{}: {}",
            tr.format("Open {title}", &[("title", title)]),
            tr.text("Profile"),
            self.description
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LibraryProfileIcon {
    pub(super) canvas_dimensions: (u32, u32),
    pub(super) touch: bool,
}

impl LibraryProfileIcon {
    fn orientation(self) -> &'static str {
        match self.canvas_dimensions.0.cmp(&self.canvas_dimensions.1) {
            std::cmp::Ordering::Less => "Portrait",
            std::cmp::Ordering::Equal => "Square",
            std::cmp::Ordering::Greater => "Landscape",
        }
    }

    fn paint(self, painter: &egui::Painter, rect: Rect, color: Color32) {
        let (width, height) = self.canvas_dimensions;
        let portrait_size = if width == height {
            Vec2::splat(12.0)
        } else {
            Vec2::new(9.0, 14.0)
        };
        // Rotate the entire device, including its screen and keypad, together.
        let point = |x, y| {
            let local = portrait_size * Vec2::new(x, y);
            rect.center()
                + if width > height {
                    Vec2::new(-local.y, local.x)
                } else {
                    local
                }
        };
        let phone = Rect::from_two_pos(point(-0.5, -0.5), point(0.5, 0.5));
        painter.rect_stroke(
            phone,
            2.0,
            egui::Stroke::new(1.1, color),
            egui::StrokeKind::Inside,
        );
        let screen_bottom = if self.touch { 0.34 } else { 0.05 };
        let screen = Rect::from_two_pos(point(-0.28, -0.34), point(0.28, screen_bottom));
        painter.rect_stroke(
            screen,
            1.0,
            egui::Stroke::new(0.8, color),
            egui::StrokeKind::Inside,
        );
        if !self.touch {
            for y in [0.22, 0.35] {
                for x in [-0.15, 0.15] {
                    painter.circle_filled(point(x, y), 0.55, color);
                }
            }
        }
    }
}

pub(super) fn library_profile_presentation(
    entry: &LibraryEntry,
    options: &[ProfileOption],
    catalog_fingerprint: &str,
    tr: crate::i18n::Translator,
) -> LibraryProfilePresentation {
    let automatic = matches!(entry.settings().device_profile, ProfileChoice::Automatic);
    let Some(option) = entry
        .effective_profile_id(catalog_fingerprint)
        .and_then(|id| options.iter().find(|option| option.profile_id == id))
    else {
        return LibraryProfilePresentation {
            label: tr.text("Automatic"),
            description: tr.text("Resolve on launch (Automatic)"),
            icon: None,
        };
    };
    let dimensions = entry
        .automatic_canvas_dimensions(catalog_fingerprint)
        .or(option.canvas_dimensions);
    let icon = dimensions.map(|canvas_dimensions| LibraryProfileIcon {
        canvas_dimensions,
        touch: option.touch,
    });
    let mut label = dimensions.map_or_else(
        || option.short_manufacturer.clone(),
        |(width, height)| format!("{} {width}×{height}", option.short_manufacturer),
    );
    let mut description = tr.profile_name(&option.display_name);
    if let (Some((base_width, base_height)), Some((width, height))) =
        (option.canvas_dimensions, dimensions)
        && (base_width, base_height) != (width, height)
    {
        description = description.replacen(
            &format!("{base_width}×{base_height}"),
            &format!("{width}×{height}"),
            1,
        );
    }
    if automatic {
        label.push_str(" · ");
        label.push_str(&tr.text("Auto"));
        description.push_str(" (");
        description.push_str(&tr.text("Automatic"));
        description.push(')');
    }
    if let Some(icon) = icon {
        description = format!(
            "{description}\n{}: {} · {}",
            tr.text("Screen"),
            tr.text(icon.orientation()),
            tr.text(if icon.touch {
                "Touch phone"
            } else {
                "Keypad phone"
            })
        );
    }
    LibraryProfilePresentation {
        label,
        description,
        icon,
    }
}

pub(super) fn friendly_profile_name(profile: &device_profile::DeviceProfile) -> String {
    let manufacturer = profile.device().manufacturer();
    let screen = profile.canvas_dimensions().map_or_else(
        || "Unknown screen".to_owned(),
        |(width, height)| format!("{width}×{height}"),
    );
    let input = crate::profile_picker::input_label(profile);
    if manufacturer == "Nokia" {
        let family = friendly_nokia_family(profile);
        return format!("Nokia · {family} · {screen} · {input}");
    }

    let mut name = format!("{manufacturer} · {screen} · {input}");
    if let Some(model) = representative_model(profile) {
        name.push_str(" · ");
        name.push_str(model);
    }
    name
}

pub(super) fn friendly_nokia_family(profile: &device_profile::DeviceProfile) -> String {
    let persona = profile.composition().persona();
    let generation = persona.generation();
    match persona.lineage() {
        "nokia-s40-keypad" if generation == 6 => "Series 40 v3–v6".to_owned(),
        "nokia-s40-keypad" | "nokia-s40-touch" => format!("Series 40 v{generation}"),
        "nokia-s60-keypad" | "nokia-s60-touch" => format!("S60 v{generation}"),
        "nokia-asha-touch" => "Asha 1.0–1.4".to_owned(),
        _ => "Mobile phone".to_owned(),
    }
}

pub(super) fn representative_model(profile: &device_profile::DeviceProfile) -> Option<&str> {
    let dimensions = profile.canvas_dimensions();
    let hints = profile.device().archive_name_hints();
    let model = hints
        .iter()
        .find_map(|hint| {
            let canvas = hint.fullscreen_canvas();
            (dimensions == Some((canvas.width(), canvas.height())))
                .then(|| hint.model_names().next())
                .flatten()
        })
        .or_else(|| hints.iter().find_map(|hint| hint.model_names().next()))
        .unwrap_or_else(|| profile.device().model());
    let model = model
        .strip_prefix(profile.device().manufacturer())
        .unwrap_or(model)
        .trim();
    let concise = if hints.is_empty() {
        model.split_whitespace().next().unwrap_or(model)
    } else {
        model
    };
    (!concise.is_empty()).then_some(concise)
}

pub(super) fn apply_settings_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::proportional(SETTINGS_BODY_TEXT_SIZE),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(SETTINGS_BODY_TEXT_SIZE),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(SETTINGS_HEADING_TEXT_SIZE),
    );
    style.spacing.interact_size.y = SETTINGS_TOUCH_TARGET_HEIGHT;
    style.spacing.button_padding = Vec2::new(12.0, 8.0);
}
