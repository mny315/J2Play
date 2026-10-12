//! Preview painting for the control grid, keypad and selected control.

use super::CONTROL_GRID_UNITS;
use super::selection::{selected_transform, selection_bounds};
use crate::{
    CONTROL_POSITION_UNITS, ControlSelection, DIRECTION_PAD_BUTTONS, MaterialTheme,
    NUMBER_ROW_KEYS, Rect, VIRTUAL_BUTTON_CORNER_RADIUS, Vec2, VirtualControlGeometry,
    VirtualControlIcon, VirtualControlLayout, button_rect, control_corner_radius, egui,
    number_button_size, paint_direction_icon, paint_virtual_control_icon,
};

const CONTROL_GRID_MAJOR_INTERVAL: i16 = 1_000;

pub(super) fn paint_editor_grid(ui: &egui::Ui, theme: &MaterialTheme, preview: Rect) {
    let steps = CONTROL_POSITION_UNITS / CONTROL_GRID_UNITS;
    for step in 1..steps {
        let units = step * CONTROL_GRID_UNITS;
        let fraction = f32::from(units) / f32::from(CONTROL_POSITION_UNITS);
        let major = units % CONTROL_GRID_MAJOR_INTERVAL == 0;
        let stroke = egui::Stroke::new(
            if major { 1.25 } else { 0.75 },
            if major {
                theme.primary.gamma_multiply(0.8)
            } else {
                theme.outline.gamma_multiply(0.7)
            },
        );
        let x = preview.left() + preview.width() * fraction;
        let y = preview.top() + preview.height() * fraction;
        ui.painter().line_segment(
            [
                egui::Pos2::new(x, preview.top()),
                egui::Pos2::new(x, preview.bottom()),
            ],
            stroke,
        );
        ui.painter().line_segment(
            [
                egui::Pos2::new(preview.left(), y),
                egui::Pos2::new(preview.right(), y),
            ],
            stroke,
        );
    }
}

pub(super) fn paint_editor_geometry(
    ui: &egui::Ui,
    theme: &MaterialTheme,
    geometry: VirtualControlGeometry,
    layout: &VirtualControlLayout,
    selected: ControlSelection,
) {
    let direction_bounds = selection_bounds(geometry, ControlSelection::DirectionPad);
    let direction_corner_radius = if layout.stick_enabled {
        direction_bounds.width() * 0.5
    } else {
        control_corner_radius(
            geometry.direction_button_size * 0.62,
            geometry.corner_radius_percent,
        )
    };
    if layout.stick_enabled {
        crate::controls::stick::paint(
            ui.painter(),
            theme,
            direction_bounds,
            Vec2::ZERO,
            false,
            layout.direction_pad.visible,
        );
    } else {
        ui.painter().rect_filled(
            direction_bounds,
            direction_corner_radius,
            editor_fill(theme, layout.direction_pad.visible),
        );
        ui.painter().rect_stroke(
            direction_bounds,
            direction_corner_radius,
            egui::Stroke::new(1.25, editor_outline(theme, layout.direction_pad.visible)),
            egui::StrokeKind::Inside,
        );
        for (direction, _, column, row) in DIRECTION_PAD_BUTTONS {
            paint_direction_icon(
                ui.painter(),
                button_rect(
                    geometry.direction_pad_center.x + column * geometry.direction_button_size,
                    geometry.direction_pad_center.y + row * geometry.direction_button_size,
                    geometry.direction_button_size,
                ),
                direction,
                editor_icon_color(theme, layout.direction_pad.visible),
            );
        }
    }
    paint_selection(
        ui,
        direction_bounds,
        selected == ControlSelection::DirectionPad,
        direction_corner_radius,
    );

    for (selection, icon) in [
        (ControlSelection::Fire, VirtualControlIcon::Fire),
        (ControlSelection::LeftSoftKey, VirtualControlIcon::SoftLeft),
        (
            ControlSelection::RightSoftKey,
            VirtualControlIcon::SoftRight,
        ),
    ] {
        paint_editor_icon(
            ui,
            theme,
            selection_bounds(geometry, selection),
            icon,
            selected == selection,
            geometry.corner_radius_percent,
            selected_transform(layout, selection).visible,
        );
    }

    paint_editor_number_keys(ui, theme, geometry, layout, selected);
}

fn paint_editor_number_keys(
    ui: &egui::Ui,
    theme: &MaterialTheme,
    geometry: VirtualControlGeometry,
    layout: &VirtualControlLayout,
    selected: ControlSelection,
) {
    for (label, _, index) in NUMBER_ROW_KEYS {
        let (_, size) = geometry.number_keys[usize::from(index)];
        let rect = selection_bounds(geometry, ControlSelection::NumberKey(index));
        let visual = Rect::from_center_size(rect.center(), Vec2::splat(number_button_size(size)));
        let corner_radius = control_corner_radius(
            f32::from(VIRTUAL_BUTTON_CORNER_RADIUS),
            geometry.corner_radius_percent,
        );
        let visible = layout.number_keys[usize::from(index)].visible;
        ui.painter()
            .rect_filled(visual, corner_radius, editor_fill(theme, visible));
        ui.painter().rect_stroke(
            visual,
            corner_radius,
            egui::Stroke::new(0.75, editor_outline(theme, visible)),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            visual.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional((size * 0.48).clamp(10.0, 20.0)),
            editor_icon_color(theme, visible),
        );
        paint_selection(
            ui,
            rect,
            selected == ControlSelection::NumberKey(index),
            corner_radius,
        );
    }
}

fn paint_editor_icon(
    ui: &egui::Ui,
    theme: &MaterialTheme,
    rect: Rect,
    icon: VirtualControlIcon,
    selected: bool,
    corner_radius_percent: u8,
    visible: bool,
) {
    let radius = rect.width().min(rect.height()) / 2.0;
    let corner_radius = control_corner_radius(radius, corner_radius_percent);
    if corner_radius_percent == 100 {
        ui.painter()
            .circle_filled(rect.center(), radius, editor_fill(theme, visible));
        ui.painter().circle_stroke(
            rect.center(),
            radius,
            egui::Stroke::new(1.25, editor_outline(theme, visible)),
        );
    } else {
        ui.painter()
            .rect_filled(rect, corner_radius, editor_fill(theme, visible));
        ui.painter().rect_stroke(
            rect,
            corner_radius,
            egui::Stroke::new(1.25, editor_outline(theme, visible)),
            egui::StrokeKind::Inside,
        );
    }
    paint_virtual_control_icon(ui.painter(), rect, icon, editor_icon_color(theme, visible));
    paint_selection(ui, rect, selected, corner_radius);
}

fn editor_fill(theme: &MaterialTheme, visible: bool) -> crate::Color32 {
    if visible {
        theme.surface_container_high
    } else {
        theme.surface_container_high.gamma_multiply(0.4)
    }
}

fn editor_outline(theme: &MaterialTheme, visible: bool) -> crate::Color32 {
    if visible {
        theme.outline
    } else {
        theme.outline.gamma_multiply(0.45)
    }
}

fn editor_icon_color(theme: &MaterialTheme, visible: bool) -> crate::Color32 {
    if visible {
        theme.on_surface
    } else {
        theme.on_surface.gamma_multiply(0.4)
    }
}

fn paint_selection(ui: &egui::Ui, rect: Rect, selected: bool, corner_radius: f32) {
    if selected {
        ui.painter().rect_stroke(
            rect.expand(3.0),
            corner_radius + 3.0,
            egui::Stroke::new(3.0, ui.visuals().selection.stroke.color),
            egui::StrokeKind::Inside,
        );
    }
}
