use super::painting::{paint_editor_geometry, paint_editor_grid};
use super::selection::{
    interact_with_editor_geometry, selected_transform, selected_transform_mut, selection_bounds,
};
use crate::controls::{bottom_controls_rect, gameplay_control_geometry};
use crate::layout::{game_canvas_rect, gameplay_layout};
use crate::{
    ControlEditorState, GameScale, Pos2, Rect, Vec2, VirtualControlGeometry, VirtualControlLayout,
    egui,
};

const EDITOR_PREVIEW_MAX_HEIGHT: f32 = 360.0;
const LANDSCAPE_PREVIEW_SIZE: Vec2 = Vec2::new(960.0, 432.0);

pub(super) struct ControlPreview {
    pub rect: Rect,
    pub controls: Rect,
    pub canvas: Option<Rect>,
    pub geometry: Option<(VirtualControlGeometry, VirtualControlGeometry)>,
    host_size: Option<Vec2>,
    source_controls: Rect,
    source_canvas: Option<Rect>,
}

pub(super) fn landscape_available(host_size: Vec2) -> bool {
    host_size.is_finite() && host_size.x > host_size.y && host_size.y > 0.0
}

pub(super) fn orientation_selector(
    ui: &mut egui::Ui,
    theme: &crate::MaterialTheme,
    host_size: Vec2,
    orientation_control: crate::OrientationControl,
    editor: &mut ControlEditorState,
) {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let landscape_available =
        orientation_control == crate::OrientationControl::Layout || landscape_available(host_size);
    if landscape_available && editor.landscape_requested {
        editor.select_orientation(true);
        editor.landscape_requested = false;
    }
    if !landscape_available {
        editor.select_orientation(false);
    }
    let page_top = ui.min_rect().min;
    ui.horizontal(|ui| {
        let width = (ui.available_width() - ui.spacing().item_spacing.x) * 0.5;
        for (label, landscape) in [("Portrait", false), ("Landscape", true)] {
            let response = ui.add_sized(
                [width, crate::SETTINGS_TOUCH_TARGET_HEIGHT],
                crate::material_choice_button(tr.text(label), editor.landscape == landscape),
            );
            if !landscape {
                crate::focus_ring::track_page_start(
                    ui,
                    &response,
                    24.0,
                    page_top,
                    &mut editor.navigation.request_focus,
                );
            }
            if response.clicked() {
                editor.landscape_requested = landscape && !landscape_available;
                if !editor.landscape_requested {
                    editor.select_orientation(landscape);
                }
            }
        }
    });
    ui.add_space(8.0);
    if editor.landscape_requested {
        let id = egui::Id::new("control-editor-rotate");
        let response = egui::Modal::new(id)
            .area(egui::Modal::default_area(id).fade_in(true))
            .frame(crate::material_card_frame(theme))
            .show(ui.ctx(), |ui| {
                ui.set_width((host_size.x - 64.0).clamp(160.0, 320.0));
                ui.label(
                    crate::RichText::new(tr.text("Rotate your phone"))
                        .strong()
                        .size(20.0),
                );
                ui.add_space(8.0);
                ui.label(tr.text("Turn your phone sideways to open Landscape controls."));
                ui.add_space(16.0);
                ui.add_sized(
                    [ui.available_width(), crate::SETTINGS_TOUCH_TARGET_HEIGHT],
                    crate::material_text_button(theme, tr.text("Cancel")),
                )
                .clicked()
            });
        if response.inner || response.should_close() {
            editor.landscape_requested = false;
        }
    }
}

impl ControlPreview {
    pub fn allocate(
        ui: &mut egui::Ui,
        host_size: Vec2,
        orientation_control: crate::OrientationControl,
        editor: &ControlEditorState,
    ) -> Self {
        if !editor.landscape {
            let host_size = Vec2::new(host_size.min_elem().max(1.0), host_size.max_elem().max(1.0));
            let controls = portrait_controls_rect(host_size, editor);
            let scale = (ui.available_width() / controls.width())
                .min(EDITOR_PREVIEW_MAX_HEIGHT / controls.height().max(1.0));
            let (slot, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), controls.height() * scale),
                egui::Sense::hover(),
            );
            let rect = Rect::from_center_size(slot.center(), controls.size() * scale);
            let transform = egui::emath::RectTransform::from_to(controls, rect);
            let geometry = |layout: &VirtualControlLayout| {
                gameplay_control_geometry(controls, layout, None)
                    .map(|geometry| transform_geometry(geometry, transform))
            };
            return Self {
                rect,
                controls: rect,
                canvas: None,
                host_size: None,
                source_controls: controls,
                source_canvas: None,
                geometry: geometry(&editor.layout).zip(geometry(&VirtualControlLayout::default())),
            };
        }
        let host_size = match orientation_control {
            crate::OrientationControl::HostWindow
                if host_size.is_finite() && host_size.min_elem() > 0.0 =>
            {
                Vec2::new(host_size.max_elem(), host_size.min_elem())
            }
            // A desktop can select Landscape while its window is portrait or
            // square. Preview a landscape viewport, otherwise gameplay geometry
            // falls back to the bottom keypad despite the selected orientation.
            crate::OrientationControl::Layout | crate::OrientationControl::HostWindow => {
                LANDSCAPE_PREVIEW_SIZE
            }
        };
        let width_scale = ui.available_width() / host_size.x;
        let scale = match orientation_control {
            crate::OrientationControl::HostWindow => {
                width_scale.min(EDITOR_PREVIEW_MAX_HEIGHT / host_size.y)
            }
            // Use the desktop editor width; its scroll area handles the height.
            crate::OrientationControl::Layout => width_scale,
        };
        let (slot, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), host_size.y * scale),
            egui::Sense::hover(),
        );
        let rect = Rect::from_center_size(slot.center(), host_size * scale);
        Self::landscape(rect, host_size, editor)
    }

    fn landscape(rect: Rect, host_size: Vec2, editor: &ControlEditorState) -> Self {
        let viewport = Rect::from_min_size(Pos2::ZERO, host_size);
        let layout = gameplay_layout(host_size, editor.canvas_dimensions, false, true);
        let controls = Rect::from_min_max(
            Pos2::new(0.0, host_size.y - layout.controls_height),
            viewport.max,
        );
        let controls = if layout.controls_overlay {
            controls
        } else {
            bottom_controls_rect(controls)
        };
        let canvas = game_canvas_rect(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(host_size.x, layout.game_height)),
            editor.canvas_dimensions,
            GameScale::AutomaticFit,
        );
        let transform = egui::emath::RectTransform::from_to(viewport, rect);
        let geometry = |control_layout: &VirtualControlLayout| {
            let geometry = gameplay_control_geometry(
                controls,
                control_layout,
                layout.controls_overlay.then_some(canvas),
            )?;
            Some(transform_geometry(geometry, transform))
        };
        Self {
            rect,
            controls: transform.transform_rect(controls),
            canvas: Some(transform.transform_rect(canvas)),
            host_size: Some(host_size),
            source_controls: controls,
            source_canvas: layout.controls_overlay.then_some(canvas),
            geometry: geometry(&editor.layout).zip(geometry(&VirtualControlLayout::default())),
        }
    }

    pub fn show(
        &self,
        ui: &mut egui::Ui,
        theme: &crate::MaterialTheme,
        editor: &mut ControlEditorState,
    ) {
        ui.painter()
            .rect_filled(self.rect, 18.0, theme.surface_container);
        ui.painter().rect_stroke(
            self.rect,
            18.0,
            egui::Stroke::new(1.0, theme.outline_variant),
            egui::StrokeKind::Inside,
        );
        if editor.grid_enabled {
            paint_editor_grid(ui, theme, self.controls);
        }
        if let Some(canvas) = self.canvas {
            ui.painter().rect_filled(canvas, 0.0, egui::Color32::BLACK);
            ui.painter().rect_stroke(
                canvas,
                0.0,
                egui::Stroke::new(2.0, theme.primary),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                canvas.center(),
                egui::Align2::CENTER_CENTER,
                crate::i18n::Translator::from_context(ui.ctx()).text("GAME SCREEN"),
                egui::FontId::proportional(18.0),
                egui::Color32::WHITE,
            );
        }
        if let Some((geometry, defaults)) = self.geometry {
            paint_editor_geometry(ui, theme, geometry, &editor.layout, editor.selected);
            interact_with_editor_geometry(ui, self.controls, (geometry, defaults), editor);
        } else {
            editor.navigation.focused = None;
            editor.navigation.moving = None;
            ui.painter().text(
                self.rect.center(),
                egui::Align2::CENTER_CENTER,
                crate::i18n::Translator::from_context(ui.ctx())
                    .text("More space is needed to edit controls"),
                egui::FontId::proportional(14.0),
                theme.on_surface_variant,
            );
        }
    }

    pub fn reject_screen_overlap(
        &self,
        editor: &mut ControlEditorState,
        previous: &VirtualControlLayout,
    ) -> bool {
        let Some(host_size) = self.host_size else {
            return false;
        };
        let selected = editor.selected;
        let transform = selected_transform(&editor.layout, selected);
        let old = selected_transform(previous, selected);
        if transform == old || !transform.visible {
            return false;
        }
        let current = Self::landscape(self.rect, host_size, editor);
        let Some((geometry, _)) = current.geometry else {
            return false;
        };
        let Some(canvas) = current.canvas else {
            return false;
        };
        let bounds = selection_bounds(geometry, selected);
        if bounds.intersects(canvas.expand(2.0)) {
            *selected_transform_mut(&mut editor.layout, selected) = old;
            return true;
        }
        false
    }

    pub fn selection_moved(
        &self,
        editor: &ControlEditorState,
        previous: &VirtualControlLayout,
    ) -> bool {
        let selected = editor.selected;
        let before = selected_transform(previous, selected);
        let after = selected_transform(&editor.layout, selected);
        if (before.offset_x, before.offset_y) == (after.offset_x, after.offset_y) {
            return false;
        }
        let Some((old, _)) = self.geometry else {
            return false;
        };
        let Some(current) =
            gameplay_control_geometry(self.source_controls, &editor.layout, self.source_canvas)
        else {
            return false;
        };
        let transform = egui::emath::RectTransform::from_to(self.source_controls, self.controls);
        let current = transform_geometry(current, transform);
        // Bounds may clamp several saved offsets to the same visible position.
        // Feedback follows the actual control after Canvas protection is applied.
        selection_bounds(old, selected)
            .center()
            .distance_sq(selection_bounds(current, selected).center())
            > 0.01
    }
}

fn portrait_controls_rect(host_size: Vec2, editor: &ControlEditorState) -> Rect {
    let available = Rect::from_min_size(Pos2::ZERO, host_size);
    let layout = crate::gameplay_portrait::portrait_layout(
        available,
        editor.canvas_dimensions,
        editor.game_scale,
        editor.portrait_frame_percent,
    )
    .unwrap_or_else(|| gameplay_layout(host_size, editor.canvas_dimensions, false, false));
    bottom_controls_rect(Rect::from_min_size(
        Pos2::new(0.0, host_size.y - layout.controls_height),
        Vec2::new(host_size.x, layout.controls_height),
    ))
}

fn transform_geometry(
    mut geometry: VirtualControlGeometry,
    transform: egui::emath::RectTransform,
) -> VirtualControlGeometry {
    let scale = transform.scale().x;
    geometry.direction_pad_center = transform * geometry.direction_pad_center;
    geometry.direction_button_size *= scale;
    geometry.fire_center = transform * geometry.fire_center;
    geometry.fire_size *= scale;
    geometry.left_soft_key_center = transform * geometry.left_soft_key_center;
    geometry.left_soft_key_size *= scale;
    geometry.right_soft_key_center = transform * geometry.right_soft_key_center;
    geometry.right_soft_key_size *= scale;
    for (center, size) in &mut geometry.number_keys {
        *center = transform * *center;
        *size *= scale;
    }
    geometry
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/control_editor/preview/mod.rs"]
mod tests;
