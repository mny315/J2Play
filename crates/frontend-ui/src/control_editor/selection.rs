//! Control selection, hit regions and normalized drag transforms.

use super::CONTROL_GRID_UNITS;
use crate::{
    CONTROL_POSITION_UNITS, ControlEditorState, ControlSelection, ControlTransform,
    NUMBER_ROW_KEYS, Rect, Vec2, VirtualControlGeometry, VirtualControlLayout, egui,
};

pub(super) fn interact_with_editor_geometry(
    ui: &mut egui::Ui,
    preview: Rect,
    (geometry, default_geometry): (VirtualControlGeometry, VirtualControlGeometry),
    editor: &mut ControlEditorState,
) {
    let previous_selection = editor.selected;
    let previous_moving = editor.navigation.moving;
    if !ui.is_enabled()
        || !ui.input(|input| input.focused && !input.pointer.any_down())
        || editor.navigation.moving != ui.memory(egui::Memory::focused)
    {
        editor.navigation.moving = None;
    }
    editor.navigation.focused = None;
    let controls = [
        ControlSelection::DirectionPad,
        ControlSelection::Fire,
        ControlSelection::LeftSoftKey,
        ControlSelection::RightSoftKey,
    ]
    .into_iter()
    .chain(
        NUMBER_ROW_KEYS
            .into_iter()
            .map(|(_, _, index)| ControlSelection::NumberKey(index)),
    );

    for selection in controls {
        let rect = selection_bounds(geometry, selection);
        let response = ui
            .interact(
                rect,
                ui.make_persistent_id(("control-editor", selection_id(selection))),
                egui::Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Grab);
        let label = crate::i18n::Translator::from_context(ui.ctx())
            .control(selection_label(&editor.layout, selection));
        let enabled = ui.is_enabled();
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        if response.clicked() || response.drag_started() {
            editor.selected = selection;
            response.request_focus();
            editor.navigation.moving = if response.clicked()
                && !response.clicked_by(egui::PointerButton::Primary)
                && editor.navigation.moving != Some(response.id)
            {
                Some(response.id)
            } else {
                None
            };
            editor.drag_origin = None;
        }
        if response.has_focus() {
            editor.navigation.focused = Some(response.id);
            editor.selected = selection;
            let moving = editor.navigation.moving == Some(response.id);
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        horizontal_arrows: moving,
                        vertical_arrows: moving,
                        escape: moving,
                        ..Default::default()
                    },
                );
            });
            if moving {
                super::navigation::move_selected(ui, editor, preview, geometry, default_geometry);
            }
        }
        crate::focus_ring::track(ui, &response, rect.width().min(rect.height()) * 0.2);
        if response.drag_started() {
            editor.drag_origin = Some((
                selected_transform(&editor.layout, selection),
                selection_center(geometry, selection),
            ));
        }
        if response.dragged()
            && editor.selected == selection
            && let Some((origin, origin_center)) = editor.drag_origin
        {
            // `drag_delta` is only the latest frame; using it here makes a
            // slowly dragged control jump back toward its gesture origin.
            let moved = transform_after_drag(
                origin,
                origin_center,
                selection_center(default_geometry, selection),
                response.total_drag_delta().unwrap_or_default(),
                preview,
                editor.grid_enabled,
            );
            *selected_transform_mut(&mut editor.layout, selection) = moved;
        }
        if response.drag_stopped() && editor.selected == selection {
            editor.drag_origin = None;
        }
    }
    if editor.navigation.moving != editor.navigation.focused {
        editor.navigation.moving = None;
    }
    if editor.selected != previous_selection || editor.navigation.moving != previous_moving {
        ui.ctx().request_repaint();
    }
}

#[allow(clippy::cast_possible_truncation)]
pub(super) fn transform_after_drag(
    origin: ControlTransform,
    origin_center: egui::Pos2,
    default_center: egui::Pos2,
    delta: Vec2,
    bounds: Rect,
    snap_to_grid: bool,
) -> ControlTransform {
    let moved_center = origin_center + delta;
    let center = if snap_to_grid {
        snap_center_to_grid(moved_center, bounds)
    } else {
        moved_center
    };
    let horizontal = if bounds.width() > 0.0 {
        ((center.x - default_center.x) * f32::from(CONTROL_POSITION_UNITS) / bounds.width()).round()
            as i32
    } else {
        0
    };
    let vertical = if bounds.height() > 0.0 {
        ((center.y - default_center.y) * f32::from(CONTROL_POSITION_UNITS) / bounds.height())
            .round() as i32
    } else {
        0
    };
    ControlTransform {
        offset_x: bounded_position(horizontal),
        offset_y: bounded_position(vertical),
        ..origin
    }
}

fn snap_center_to_grid(center: egui::Pos2, bounds: Rect) -> egui::Pos2 {
    let snap_axis = |coordinate: f32, minimum: f32, extent: f32| {
        if extent <= 0.0 {
            return minimum;
        }
        let units = (coordinate - minimum) * f32::from(CONTROL_POSITION_UNITS) / extent;
        let grid = f32::from(CONTROL_GRID_UNITS);
        minimum + (units / grid).round() * grid * extent / f32::from(CONTROL_POSITION_UNITS)
    };
    egui::Pos2::new(
        snap_axis(center.x, bounds.left(), bounds.width()),
        snap_axis(center.y, bounds.top(), bounds.height()),
    )
}

fn selection_geometry(
    geometry: VirtualControlGeometry,
    selection: ControlSelection,
) -> (egui::Pos2, f32) {
    match selection {
        ControlSelection::DirectionPad => (
            geometry.direction_pad_center,
            geometry.direction_button_size * 3.0,
        ),
        ControlSelection::Fire => (geometry.fire_center, geometry.fire_size),
        ControlSelection::LeftSoftKey => {
            (geometry.left_soft_key_center, geometry.left_soft_key_size)
        }
        ControlSelection::RightSoftKey => {
            (geometry.right_soft_key_center, geometry.right_soft_key_size)
        }
        ControlSelection::NumberKey(index) => geometry.number_keys[usize::from(index)],
    }
}

fn selection_center(geometry: VirtualControlGeometry, selection: ControlSelection) -> egui::Pos2 {
    selection_geometry(geometry, selection).0
}

pub(super) fn selection_bounds(
    geometry: VirtualControlGeometry,
    selection: ControlSelection,
) -> Rect {
    let (center, size) = selection_geometry(geometry, selection);
    Rect::from_center_size(center, Vec2::splat(size))
}

fn bounded_position(value: i32) -> i16 {
    let bound = i32::from(CONTROL_POSITION_UNITS);
    i16::try_from(value.clamp(-bound, bound)).unwrap_or(0)
}

pub(super) fn selected_transform(
    layout: &VirtualControlLayout,
    selection: ControlSelection,
) -> ControlTransform {
    match selection {
        ControlSelection::DirectionPad => layout.direction_pad,
        ControlSelection::Fire => layout.fire,
        ControlSelection::LeftSoftKey => layout.left_soft_key,
        ControlSelection::RightSoftKey => layout.right_soft_key,
        ControlSelection::NumberKey(index) => layout.number_keys[usize::from(index)],
    }
}

pub(super) fn selected_transform_mut(
    layout: &mut VirtualControlLayout,
    selection: ControlSelection,
) -> &mut ControlTransform {
    match selection {
        ControlSelection::DirectionPad => &mut layout.direction_pad,
        ControlSelection::Fire => &mut layout.fire,
        ControlSelection::LeftSoftKey => &mut layout.left_soft_key,
        ControlSelection::RightSoftKey => &mut layout.right_soft_key,
        ControlSelection::NumberKey(index) => &mut layout.number_keys[usize::from(index)],
    }
}

pub(super) fn selection_label(
    layout: &VirtualControlLayout,
    selection: ControlSelection,
) -> &'static str {
    match selection {
        ControlSelection::DirectionPad if layout.stick_enabled => "Virtual stick",
        ControlSelection::DirectionPad => "Direction pad",
        ControlSelection::Fire => "Fire key",
        ControlSelection::LeftSoftKey => "Left soft key",
        ControlSelection::RightSoftKey => "Right soft key",
        ControlSelection::NumberKey(0) => "1 key",
        ControlSelection::NumberKey(1) => "2 key",
        ControlSelection::NumberKey(2) => "3 key",
        ControlSelection::NumberKey(3) => "4 key",
        ControlSelection::NumberKey(4) => "5 key",
        ControlSelection::NumberKey(5) => "6 key",
        ControlSelection::NumberKey(6) => "7 key",
        ControlSelection::NumberKey(7) => "8 key",
        ControlSelection::NumberKey(8) => "9 key",
        ControlSelection::NumberKey(9) => "Star key",
        ControlSelection::NumberKey(10) => "0 key",
        ControlSelection::NumberKey(11) => "Pound key",
        ControlSelection::NumberKey(_) => "Number key",
    }
}

const fn selection_id(selection: ControlSelection) -> u8 {
    match selection {
        ControlSelection::DirectionPad => 0,
        ControlSelection::Fire => 1,
        ControlSelection::LeftSoftKey => 2,
        ControlSelection::RightSoftKey => 3,
        ControlSelection::NumberKey(index) => 4_u8.saturating_add(index),
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/control_editor/selection.rs"]
mod tests;
