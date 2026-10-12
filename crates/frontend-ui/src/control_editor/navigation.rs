//! Directional focus and grid movement inside the virtual control editor.

use super::CONTROL_GRID_UNITS;
use super::selection::{
    selected_transform, selected_transform_mut, selection_bounds, transform_after_drag,
};
use crate::{ControlEditorState, FrontendApp, Rect, Vec2, VirtualControlGeometry, egui};

#[derive(Clone, Debug)]
pub(crate) struct ControlNavigation {
    pub focused: Option<egui::Id>,
    pub moving: Option<egui::Id>,
    pub request_focus: bool,
}

impl Default for ControlNavigation {
    fn default() -> Self {
        Self {
            focused: None,
            moving: None,
            request_focus: true,
        }
    }
}

const DIRECTIONS: [(egui::Key, egui::FocusDirection, Vec2); 4] = [
    (
        egui::Key::ArrowUp,
        egui::FocusDirection::Up,
        Vec2::new(0.0, -1.0),
    ),
    (
        egui::Key::ArrowDown,
        egui::FocusDirection::Down,
        Vec2::new(0.0, 1.0),
    ),
    (
        egui::Key::ArrowLeft,
        egui::FocusDirection::Left,
        Vec2::new(-1.0, 0.0),
    ),
    (
        egui::Key::ArrowRight,
        egui::FocusDirection::Right,
        Vec2::new(1.0, 0.0),
    ),
];

fn owns_focus(ctx: &egui::Context, editor: &ControlEditorState) -> bool {
    editor.navigation.focused.is_some_and(|id| {
        ctx.memory(|memory| memory.has_focus(id))
            && ctx.read_response(id).is_some_and(|response| {
                response.enabled()
                    && ctx.memory(|memory| memory.allows_interaction(response.layer_id))
            })
    }) && ctx.input(|input| input.focused && !input.pointer.any_down())
}

impl FrontendApp {
    pub(crate) fn control_editor_keyboard(&mut self, ctx: &egui::Context) -> bool {
        let enabled = !self.platform_suspended && !self.overlay_active();
        let Some(editor) = self.screen.control_editor_mut() else {
            return false;
        };
        if !enabled || !owns_focus(ctx, editor) {
            editor.navigation.moving = None;
            return false;
        }
        if editor.navigation.moving.is_none() {
            // These drag targets select controls; Left/Right must navigate them
            // instead of being reserved for slider adjustment.
            for (key, direction, _) in DIRECTIONS {
                if ctx.input(|input| input.key_pressed(key) && input.modifiers.is_none())
                    && crate::focus_navigation::move_focus(ctx, direction)
                {
                    ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key));
                }
            }
        }
        true
    }

    pub(crate) fn control_editor_move_direction(
        &mut self,
        ctx: &egui::Context,
        direction: egui::FocusDirection,
    ) -> bool {
        if self.platform_suspended || self.overlay_active() {
            return false;
        }
        let Some(editor) = self.screen.control_editor_mut() else {
            return false;
        };
        if editor.navigation.moving.is_none() || !owns_focus(ctx, editor) {
            editor.navigation.moving = None;
            return false;
        }
        let Some((key, _, _)) = DIRECTIONS
            .iter()
            .find(|(_, candidate, _)| *candidate == direction)
        else {
            return false;
        };
        ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        crate::physical_controls::press_ui_key(ctx, *key);
        true
    }
}

pub(super) fn move_selected(
    ui: &egui::Ui,
    editor: &mut ControlEditorState,
    bounds: Rect,
    geometry: VirtualControlGeometry,
    defaults: VirtualControlGeometry,
) {
    for (key, _, direction) in DIRECTIONS {
        if !ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
            continue;
        }
        let selection = editor.selected;
        let origin = selected_transform(&editor.layout, selection);
        let delta = direction * bounds.size() * f32::from(CONTROL_GRID_UNITS)
            / f32::from(crate::CONTROL_POSITION_UNITS);
        let mut moved = transform_after_drag(
            origin,
            selection_bounds(geometry, selection).center(),
            selection_bounds(defaults, selection).center(),
            delta,
            bounds,
            editor.grid_enabled,
        );
        // Snapping one axis must not shift the other, even when its default
        // center is between grid lines or a saved offset is clamped at an edge.
        if direction.x == 0.0 {
            moved.offset_x = origin.offset_x;
        } else {
            moved.offset_y = origin.offset_y;
        }
        *selected_transform_mut(&mut editor.layout, selection) = moved;
    }
}
