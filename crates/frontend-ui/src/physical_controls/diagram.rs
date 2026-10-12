use super::selection::{BindingTarget, ControlGroup, Picker};
use super::{GamepadButton, PhysicalBindings, PhysicalControl, egui};

struct Part {
    picker: Picker,
    label: &'static str,
    center: egui::Pos2,
    size: egui::Vec2,
    round: bool,
}

// All coordinates use the same undistorted 640-point artwork canvas. The UV
// rectangle trims transparent padding without changing the embedded PNG's alpha.
const BODY_BYTES: &[u8] = include_bytes!("../../assets/gamepad-body.png");
const DESIGN_WIDTH: f32 = 640.0;
const BODY_CROP: egui::Rect =
    egui::Rect::from_min_max(egui::pos2(128.0, 24.0), egui::pos2(1644.0, 863.0));

#[derive(Default)]
pub(super) struct Diagram {
    texture: Option<Result<egui::TextureHandle, String>>,
}

fn body_size(width: f32) -> egui::Vec2 {
    egui::vec2(width, width * BODY_CROP.height() / BODY_CROP.width())
}

fn parts(width: f32) -> Vec<Part> {
    use GamepadButton as B;
    let mut parts = Vec::new();
    for (right, trigger_x, shoulder_x) in [(false, 160.0, 238.0), (true, 480.0, 402.0)] {
        parts.push(Part {
            picker: Picker::Binding(BindingTarget::Trigger { right }),
            label: if right { "R2 / RT" } else { "L2 / LT" },
            center: egui::pos2(trigger_x, 72.0),
            size: egui::vec2(72.0, 48.0),
            round: false,
        });
        parts.push(Part {
            picker: Picker::Binding(BindingTarget::Control(PhysicalControl::Gamepad {
                button: if right {
                    B::RightShoulder
                } else {
                    B::LeftShoulder
                },
            })),
            label: if right { "R1 / RB" } else { "L1 / LB" },
            center: egui::pos2(shoulder_x, 72.0),
            size: egui::vec2(72.0, 48.0),
            round: false,
        });
    }
    parts.swap(2, 3);
    for (button, label, offset) in [
        (B::South, "A", egui::vec2(0.0, 50.0)),
        (B::East, "B", egui::vec2(50.0, 0.0)),
        (B::West, "X", egui::vec2(-50.0, 0.0)),
        (B::North, "Y", egui::vec2(0.0, -50.0)),
    ] {
        parts.push(Part {
            picker: Picker::Binding(BindingTarget::Control(PhysicalControl::Gamepad { button })),
            label,
            center: egui::pos2(508.0, 172.0) + offset,
            size: egui::Vec2::splat(48.0),
            round: true,
        });
    }
    parts.push(Part {
        picker: Picker::Group(ControlGroup::Dpad),
        label: "D-pad",
        center: egui::pos2(146.0, 174.0),
        size: egui::Vec2::splat(88.0),
        round: true,
    });
    for (group, label, x) in [
        (ControlGroup::LeftStick, "L stick", 270.0),
        (ControlGroup::RightStick, "R stick", 370.0),
    ] {
        parts.push(Part {
            picker: Picker::Group(group),
            label,
            center: egui::pos2(x, 182.0),
            size: egui::Vec2::splat(68.0),
            round: true,
        });
    }
    for (button, label, x) in [(B::Select, "Select", 286.0), (B::Start, "Start", 350.0)] {
        parts.push(Part {
            picker: Picker::Binding(BindingTarget::Control(PhysicalControl::Gamepad { button })),
            label,
            center: egui::pos2(x, 122.0),
            size: egui::Vec2::splat(48.0),
            round: false,
        });
    }
    let scale = width / DESIGN_WIDTH;
    for part in &mut parts {
        part.center = (part.center.to_vec2() * scale).to_pos2();
        part.size *= scale;
    }
    parts
}

fn description(picker: Picker, bindings: &PhysicalBindings, tr: crate::i18n::Translator) -> String {
    match picker {
        Picker::Binding(target) => format!(
            "{} - {}",
            tr.control(&target.label()),
            tr.control(&target.assignment(bindings))
        ),
        Picker::Group(group) => tr.text(group.label()),
    }
}

fn paint_dpad(
    painter: &egui::Painter,
    theme: &crate::MaterialTheme,
    center: egui::Pos2,
    outline: egui::Color32,
    scale: f32,
) {
    painter.circle_filled(center, 42.0 * scale, theme.surface_container);
    let cross = [
        (-10.0, -32.0),
        (10.0, -32.0),
        (10.0, -10.0),
        (32.0, -10.0),
        (32.0, 10.0),
        (10.0, 10.0),
        (10.0, 32.0),
        (-10.0, 32.0),
        (-10.0, 10.0),
        (-32.0, 10.0),
        (-32.0, -10.0),
        (-10.0, -10.0),
    ];
    painter.rect_filled(
        egui::Rect::from_center_size(center, egui::vec2(64.0, 20.0) * scale),
        3.0 * scale,
        theme.surface,
    );
    painter.rect_filled(
        egui::Rect::from_center_size(center, egui::vec2(20.0, 64.0) * scale),
        3.0 * scale,
        theme.surface,
    );
    painter.add(egui::Shape::closed_line(
        cross
            .into_iter()
            .map(|(x, y)| center + egui::vec2(x, y) * scale)
            .collect(),
        egui::Stroke::new(1.5 * scale, outline),
    ));
    painter.circle_filled(center, 4.0 * scale, theme.surface_container_highest);
}

fn paint_part(
    painter: &egui::Painter,
    theme: &crate::MaterialTheme,
    part: &Part,
    rect: egui::Rect,
    hovered: bool,
    scale: f32,
) {
    let center = rect.center();
    let outline = if hovered {
        theme.primary
    } else {
        theme.outline
    };
    match part.picker {
        Picker::Group(ControlGroup::Dpad) => {
            paint_dpad(painter, theme, center, outline, scale);
        }
        Picker::Group(_) => {
            painter.circle_filled(center, 33.0 * scale, theme.surface_container);
            painter.circle_stroke(
                center,
                31.0 * scale,
                egui::Stroke::new(2.0 * scale, theme.outline_variant),
            );
            painter.circle_filled(center, 28.0 * scale, theme.surface);
            painter.circle_stroke(
                center,
                27.0 * scale,
                egui::Stroke::new(1.5 * scale, outline),
            );
            painter.circle_stroke(
                center,
                22.0 * scale,
                egui::Stroke::new(1.0 * scale, theme.outline_variant),
            );
            paint_label(
                painter,
                &crate::i18n::Translator::from_context(painter.ctx()).text(part.label),
                egui::Rect::from_center_size(center, egui::vec2(36.0, 30.0) * scale),
                scale,
                theme.on_surface,
            );
        }
        Picker::Binding(_) if part.round => {
            let color = match (part.label, theme.mode) {
                ("A", crate::PlatformThemeMode::Light) => egui::Color32::from_rgb(36, 112, 58),
                ("B", crate::PlatformThemeMode::Light) => egui::Color32::from_rgb(181, 46, 55),
                ("X", crate::PlatformThemeMode::Light) => egui::Color32::from_rgb(35, 99, 173),
                (_, crate::PlatformThemeMode::Light) => egui::Color32::from_rgb(134, 94, 13),
                ("A", _) => egui::Color32::from_rgb(143, 214, 161),
                ("B", _) => egui::Color32::from_rgb(244, 150, 154),
                ("X", _) => egui::Color32::from_rgb(143, 192, 245),
                _ => egui::Color32::from_rgb(245, 209, 126),
            };
            painter.circle_filled(center, 23.0 * scale, theme.surface);
            painter.circle_stroke(
                center,
                22.0 * scale,
                egui::Stroke::new(
                    1.5 * scale,
                    if hovered {
                        theme.primary
                    } else {
                        color.gamma_multiply(0.6)
                    },
                ),
            );
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                part.label,
                egui::FontId::proportional(22.0 * scale),
                color,
            );
        }
        Picker::Binding(_) => {
            let face = egui::Rect::from_center_size(
                center,
                egui::vec2(rect.width() - 4.0 * scale, 32.0 * scale),
            );
            painter.rect_filled(face, 12.0 * scale, theme.surface_container_high);
            painter.rect_stroke(
                face,
                12.0 * scale,
                egui::Stroke::new(1.0 * scale, outline),
                egui::StrokeKind::Inside,
            );
            paint_label(
                painter,
                &crate::i18n::Translator::from_context(painter.ctx()).text(part.label),
                face.shrink(4.0 * scale),
                scale,
                theme.on_surface,
            );
        }
    }
}

fn paint_label(
    painter: &egui::Painter,
    label: &str,
    bounds: egui::Rect,
    scale: f32,
    color: egui::Color32,
) {
    // Shape and wrap complete Unicode clusters; character counts do not predict
    // the width or height of Arabic, Thai or Devanagari labels.
    let mut font_size = 10.0 * scale;
    for attempt in 0..3 {
        let galley = painter.layout(
            label.to_owned(),
            egui::FontId::proportional(font_size),
            color,
            bounds.width(),
        );
        let fit = (bounds.width() / galley.size().x).min(bounds.height() / galley.size().y);
        if fit >= 1.0 || attempt == 2 {
            painter.galley(bounds.center() - galley.size() * 0.5, galley, color);
            return;
        }
        font_size *= fit * 0.95;
    }
}

impl Diagram {
    pub(super) fn draw(
        &mut self,
        ui: &mut egui::Ui,
        theme: &crate::MaterialTheme,
        bindings: &PhysicalBindings,
        focus: bool,
    ) -> Option<Picker> {
        let texture = self.texture.get_or_insert_with(|| {
            // Trusted, immutable project asset; decoded only once per editor.
            image::load_from_memory_with_format(BODY_BYTES, image::ImageFormat::Png)
                .map(|image| {
                    let rgba = image.into_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    ui.ctx().load_texture(
                        "gamepad-body",
                        egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
                        egui::TextureOptions::LINEAR,
                    )
                })
                .map_err(|error| format!("Could not load the controller image: {error}"))
        });
        let mut selected = None;
        let width = ui.available_width().clamp(1.0, 800.0);
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let compact = width < DESIGN_WIDTH;
        let scale = width / DESIGN_WIDTH;
        let controls = parts(width);
        match texture {
            Ok(texture) => {
                let (slot, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), body_size(width).y),
                    egui::Sense::hover(),
                );
                let body = egui::Rect::from_center_size(slot.center(), body_size(width));
                let uv = egui::Rect::from_min_max(
                    (BODY_CROP.min.to_vec2() / texture.size_vec2()).to_pos2(),
                    (BODY_CROP.max.to_vec2() / texture.size_vec2()).to_pos2(),
                );
                ui.painter()
                    .image(texture.id(), body, uv, egui::Color32::WHITE);
                // Tab navigation follows rows; the compact grid groups controls.
                let mut spatial: Vec<_> = controls.iter().collect();
                spatial.sort_by(|a, b| {
                    a.center
                        .y
                        .total_cmp(&b.center.y)
                        .then(a.center.x.total_cmp(&b.center.x))
                });
                for part in spatial {
                    let rect =
                        egui::Rect::from_center_size(body.min + part.center.to_vec2(), part.size);
                    // Exact hit regions share the artwork transform. The compact
                    // grid supplies full touch targets when these become small.
                    let response = ui.interact(
                        rect,
                        ui.make_persistent_id(("gamepad-part", part.picker)),
                        egui::Sense::click(),
                    );
                    paint_part(ui.painter(), theme, part, rect, response.hovered(), scale);
                    if focus && !compact && part.picker == controls[0].picker {
                        response.request_focus();
                    }
                    let current = description(part.picker, bindings, tr);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            ui.is_enabled(),
                            &current,
                        )
                    });
                    crate::focus_ring::track(
                        ui,
                        &response,
                        if part.round {
                            rect.width() * 0.5
                        } else {
                            12.0 * scale
                        },
                    );
                    if response.clicked() {
                        selected = Some(part.picker);
                    }
                }
            }
            Err(error) => {
                ui.label(tr.error(error.as_str()));
                ui.collapsing(tr.text("Technical details"), |ui| {
                    ui.label(error.as_str());
                });
            }
        }
        if compact || texture.is_err() {
            selected = draw_compact_controls(ui, theme, bindings, focus, &controls).or(selected);
        }
        ui.add_space(8.0);
        selected
    }
}

fn draw_compact_controls(
    ui: &mut egui::Ui,
    theme: &crate::MaterialTheme,
    bindings: &PhysicalBindings,
    focus: bool,
    controls: &[Part],
) -> Option<Picker> {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let mut selected = None;
    ui.add_space(8.0);
    // Keep the whole gamepad visible on phones and high UI scales,
    // with equivalent, non-overlapping 48-point controls below it.
    let columns = (1..=4_u8)
        .rev()
        .find(|&columns| ui.available_width() >= f32::from(columns) * 80.0)
        .unwrap_or(1);
    let gap = ui.spacing().item_spacing.x;
    let cell_width =
        ((ui.available_width() - gap * f32::from(columns - 1)) / f32::from(columns)).max(1.0);
    for row in controls.chunks(usize::from(columns)) {
        ui.horizontal(|ui| {
            for part in row {
                let response = ui
                    .push_id(("gamepad-control", part.picker), |ui| {
                        ui.add_sized(
                            [cell_width, crate::SETTINGS_TOUCH_TARGET_HEIGHT],
                            crate::material_outlined_button(theme, tr.text(part.label)).wrap(),
                        )
                    })
                    .inner;
                if focus && part.picker == controls[0].picker {
                    response.request_focus();
                }
                let current = description(part.picker, bindings, tr);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &current)
                });
                if response.clicked() {
                    selected = Some(part.picker);
                }
            }
        });
    }
    selected
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/physical_controls/diagram.rs"]
mod tests;
