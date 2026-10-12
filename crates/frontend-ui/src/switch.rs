//! Shared accessible switch row for settings and control editors.

use crate::{MaterialTheme, RichText, SETTINGS_TOUCH_TARGET_HEIGHT, egui};

pub(super) fn draw(
    ui: &mut egui::Ui,
    theme: &MaterialTheme,
    value: &mut bool,
    label: &str,
) -> egui::Response {
    let switch_size = egui::vec2(52.0, 32.0);
    let gap = 12.0;
    let width = ui.available_width();
    let text = egui::WidgetText::from(RichText::new(label).strong()).into_galley(
        ui,
        Some(egui::TextWrapMode::Wrap),
        (width - switch_size.x - gap).max(1.0),
        egui::FontSelection::Default,
    );
    let height = SETTINGS_TOUCH_TARGET_HEIGHT.max(text.size().y + 12.0);
    let (rect, mut response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
    });
    let progress = ui.ctx().animate_bool_with_time(response.id, *value, 0.12);
    let track = egui::Rect::from_center_size(
        egui::pos2(rect.right() - switch_size.x * 0.5, rect.center().y),
        switch_size,
    );
    if ui.is_rect_visible(rect) {
        ui.painter().galley(
            egui::pos2(rect.left(), rect.center().y - text.size().y * 0.5),
            text,
            theme.on_surface,
        );
        ui.painter().rect(
            track,
            16.0,
            if *value {
                theme.primary
            } else {
                theme.surface_container_highest
            },
            egui::Stroke::new(2.0, if *value { theme.primary } else { theme.outline }),
            egui::StrokeKind::Inside,
        );
        ui.painter().circle_filled(
            egui::pos2(
                egui::lerp(track.left() + 16.0..=track.right() - 16.0, progress),
                track.center().y,
            ),
            egui::lerp(8.0..=12.0, progress),
            if *value {
                theme.on_primary
            } else {
                theme.outline
            },
        );
    }
    crate::focus_ring::track(ui, &response, 12.0);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
