use super::controls::gameplay_control_geometry;
use super::gameplay_toolbar::GameplayToolbarRequest;
use super::layout::game_canvas_rect;
use super::{
    Color32, DIRECTION_PAD_BUTTONS, DirectionIcon, FrontendApp, HostAction, NUMBER_ROW_KEYS,
    NUMBER_ROW_OUTLINE_WIDTH, Pos2, Rect, Screen, SessionState, VIRTUAL_BUTTON_CORNER_RADIUS, Vec2,
    VirtualControlGeometry, VirtualControlIcon, button_rect, control_corner_radius, egui,
    number_button_size, paint_direction_icon, paint_runtime_debug_hud, paint_virtual_control_icon,
    runtime_debug_lines, virtual_control_outline, virtual_controls_visible,
};

impl FrontendApp {
    pub(super) fn draw_gameplay(&mut self, ui: &mut egui::Ui) {
        let Screen::Gameplay(gameplay) = &self.screen else {
            return;
        };
        let state = self.session.state();
        let landscape = ui.available_width() > ui.available_height();
        let control_layout = if landscape {
            gameplay.landscape_control_layout.clone()
        } else {
            gameplay.control_layout.clone()
        };
        let canvas_dimensions = gameplay.canvas_dimensions;
        let mut game_scale = gameplay.game_scale;
        let portrait_frame_percent = gameplay.portrait_frame_percent;
        let fast_forward = gameplay.fast_forward;
        let show_virtual_controls =
            virtual_controls_visible(gameplay.pointer_events, &control_layout);
        self.begin_gameplay_transition(ui, gameplay.fullscreen);
        let toolbar = self.draw_animated_gameplay_toolbar(ui, state);
        match toolbar {
            Some(GameplayToolbarRequest::ToggleFastForward) => self.toggle_fast_forward(),
            Some(GameplayToolbarRequest::ToggleRuntimeDebug) => self.toggle_runtime_debug(),
            Some(GameplayToolbarRequest::ToggleFullscreen) => self.toggle_gameplay_fullscreen(),
            Some(GameplayToolbarRequest::Stop | GameplayToolbarRequest::RotateScreen) | None => {}
        }

        let frame_dimensions = self
            .latest_frame
            .as_ref()
            .map_or(canvas_dimensions, |frame| (frame.width, frame.height));
        let portrait_layout = (show_virtual_controls && !landscape)
            .then(|| {
                super::gameplay_portrait::portrait_layout(
                    ui.available_rect_before_wrap(),
                    frame_dimensions,
                    game_scale,
                    portrait_frame_percent,
                )
            })
            .flatten();
        let layout = portrait_layout.unwrap_or_else(|| {
            super::gameplay_layout(
                ui.available_size(),
                frame_dimensions,
                !show_virtual_controls,
                landscape,
            )
        });
        if portrait_layout.is_some() {
            game_scale = super::GameScale::AutomaticFit;
        }
        let game_area = layout.allocate_game_area(ui);
        self.paint_game_frame(ui, game_area, frame_dimensions, game_scale);
        self.configure_window_ime(ui, game_area);
        if self.runtime_debug.enabled {
            let lines = runtime_debug_lines(
                &self.runtime_debug,
                state,
                canvas_dimensions,
                fast_forward,
                self.runtime_diagnostics.len(),
            );
            paint_runtime_debug_hud(ui, game_area, &lines);
        }
        self.control_regions.clear();
        self.stick_region = None;
        if show_virtual_controls {
            let controls = if layout.controls_overlay {
                Rect::from_min_max(
                    Pos2::new(
                        game_area.left(),
                        game_area.bottom() - layout.controls_height,
                    ),
                    game_area.right_bottom(),
                )
            } else {
                let controls = Rect::from_min_size(
                    game_area.left_bottom() + Vec2::new(0.0, layout.gap),
                    Vec2::new(ui.available_width(), layout.controls_height),
                );
                ui.allocate_rect(controls, egui::Sense::hover()).rect
            };
            self.draw_virtual_controls(
                ui,
                controls,
                &control_layout,
                layout.controls_overlay.then(|| {
                    game_canvas_rect(game_area, frame_dimensions, super::GameScale::AutomaticFit)
                }),
            );
        }
        // Complete the clicked frame before an action invalidates its geometry
        // or closes the screen. Returning after painting only the toolbar made
        // Stop flash an empty panel between the game and the library.
        match toolbar {
            Some(GameplayToolbarRequest::Stop) => self.stop_active_session(),
            Some(GameplayToolbarRequest::RotateScreen) => self.rotate_gameplay_screen(landscape),
            _ => {}
        }
    }

    fn configure_window_ime(&self, ui: &egui::Ui, game_area: Rect) {
        if self.platform.uses_window_ime()
            && self.text_input_active
            && self.session.state() == SessionState::Running
            && !self.overlay_active()
            && !self.platform_suspended
        {
            ui.output_mut(|output| {
                output.ime = Some(egui::output::IMEOutput {
                    purpose: egui::IMEPurpose::Normal,
                    rect: game_area,
                    cursor_rect: game_area,
                    should_interrupt_composition: false,
                });
                output.mutable_text_under_cursor = true;
            });
        }
    }

    // Framebuffer dimensions are bounded by the resolved profile; conversion
    // to egui's f32 coordinate space is the presentation boundary.
    #[allow(clippy::cast_precision_loss)]
    pub(super) fn paint_game_frame(
        &mut self,
        ui: &mut egui::Ui,
        available: Rect,
        dimensions: (u32, u32),
        scale: super::GameScale,
    ) {
        ui.painter()
            .rect_filled(available, 0.0, self.material_theme.background);
        let (width, height) = dimensions;
        if width == 0 || height == 0 {
            return;
        }
        let canvas =
            self.gameplay_transition
                .canvas(game_canvas_rect(available, (width, height), scale));
        self.canvas_rect = Some(canvas);
        let appearance = egui::Id::new("game-frame-appearance");
        let (Some(_), Some(texture)) = (&self.latest_frame, &self.game_texture) else {
            ui.ctx().animate_bool_with_time(appearance, false, 0.0);
            ui.put(
                Rect::from_center_size(canvas.center() - Vec2::new(0.0, 18.0), Vec2::splat(24.0)),
                egui::Spinner::new()
                    .size(24.0)
                    .color(self.material_theme.primary),
            );
            ui.painter().text(
                canvas.center() + Vec2::new(0.0, 18.0),
                egui::Align2::CENTER_CENTER,
                crate::i18n::Translator::from_context(ui.ctx()).text("Starting game…"),
                egui::TextStyle::Body.resolve(ui.style()),
                self.material_theme.on_surface_variant,
            );
            return;
        };
        let opacity = ui.ctx().animate_bool_with_time(
            appearance,
            true,
            if self.platform_suspended {
                0.0
            } else {
                super::ui_motion::UI_MOTION_SECONDS
            },
        );
        let mut painter = ui.painter().clone();
        painter.multiply_opacity(opacity);
        painter.rect_filled(canvas, 0.0, Color32::BLACK);
        painter.image(
            texture.id(),
            canvas,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    pub(super) fn draw_virtual_controls(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        layout: &super::VirtualControlLayout,
        canvas: Option<Rect>,
    ) {
        if let Some(panel) = self
            .gameplay_transition
            .panel(canvas.is_none().then_some(rect))
        {
            ui.painter()
                .rect_filled(panel, 18.0, self.material_theme.surface_container);
            ui.painter().rect_stroke(
                panel,
                18.0,
                egui::Stroke::new(1.0, self.material_theme.outline_variant),
                egui::StrokeKind::Inside,
            );
        }
        let Some(geometry) = gameplay_control_geometry(rect, layout, canvas) else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                crate::i18n::Translator::from_context(ui.ctx())
                    .text("Virtual controls need more space"),
                egui::FontId::proportional(14.0),
                self.material_theme.on_surface_variant,
            );
            return;
        };
        let geometry = self.gameplay_transition.controls(geometry);
        if layout.direction_pad.visible {
            if layout.stick_enabled {
                self.draw_virtual_stick(ui, geometry, layout.two_key_diagonals);
            } else {
                self.draw_direction_pad(ui, geometry, layout.two_key_diagonals);
            }
        }
        self.draw_number_row(ui, geometry, layout);
        self.draw_secondary_controls(ui, geometry, layout);
    }

    pub(super) fn draw_direction_pad(
        &mut self,
        ui: &mut egui::Ui,
        geometry: VirtualControlGeometry,
        two_key_diagonals: bool,
    ) {
        let button_size = geometry.direction_button_size;
        let center = geometry.direction_pad_center;
        let bounds = Rect::from_center_size(center, Vec2::splat(button_size * 3.0));
        ui.painter().rect_filled(
            bounds,
            control_corner_radius(button_size * 0.62, geometry.corner_radius_percent),
            self.material_theme.surface_container_high,
        );
        ui.painter().rect_stroke(
            bounds,
            control_corner_radius(button_size * 0.62, geometry.corner_radius_percent),
            egui::Stroke::new(
                1.25,
                virtual_control_outline(
                    self.material_theme.mode,
                    self.material_theme.outline,
                    self.material_theme.surface_container_high,
                ),
            ),
            egui::StrokeKind::Inside,
        );
        for (direction, action, column, row) in DIRECTION_PAD_BUTTONS {
            self.virtual_direction_button(
                ui,
                button_rect(
                    center.x + column * button_size,
                    center.y + row * button_size,
                    button_size,
                ),
                direction,
                super::TouchOwner::direction_key(Some(action), two_key_diagonals),
                geometry.corner_radius_percent,
            );
        }
    }

    pub(super) fn draw_number_row(
        &mut self,
        ui: &mut egui::Ui,
        geometry: VirtualControlGeometry,
        layout: &super::VirtualControlLayout,
    ) {
        for (label, action, index) in NUMBER_ROW_KEYS {
            if !layout.number_keys[usize::from(index)].visible {
                continue;
            }
            let (center, button_size) = geometry.number_keys[usize::from(index)];
            self.virtual_number_button(
                ui,
                button_rect(center.x, center.y, button_size),
                label,
                action,
                geometry.corner_radius_percent,
            );
        }
    }

    pub(super) fn draw_secondary_controls(
        &mut self,
        ui: &mut egui::Ui,
        geometry: VirtualControlGeometry,
        layout: &super::VirtualControlLayout,
    ) {
        if layout.fire.visible {
            self.virtual_icon_button(
                ui,
                button_rect(
                    geometry.fire_center.x,
                    geometry.fire_center.y,
                    geometry.fire_size,
                ),
                "Fire key",
                VirtualControlIcon::Fire,
                HostAction::Fire,
                geometry.corner_radius_percent,
            );
        }
        if layout.left_soft_key.visible {
            self.virtual_icon_button(
                ui,
                button_rect(
                    geometry.left_soft_key_center.x,
                    geometry.left_soft_key_center.y,
                    geometry.left_soft_key_size,
                ),
                "Left soft key",
                VirtualControlIcon::SoftLeft,
                HostAction::SoftLeft,
                geometry.corner_radius_percent,
            );
        }

        if layout.right_soft_key.visible {
            self.virtual_icon_button(
                ui,
                button_rect(
                    geometry.right_soft_key_center.x,
                    geometry.right_soft_key_center.y,
                    geometry.right_soft_key_size,
                ),
                "Right soft key",
                VirtualControlIcon::SoftRight,
                HostAction::SoftRight,
                geometry.corner_radius_percent,
            );
        }
    }

    pub(super) fn virtual_number_button(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        label: &'static str,
        action: HostAction,
        corner_radius_percent: u8,
    ) {
        self.control_regions
            .push((rect, super::TouchOwner::VirtualKey(Some(action))));
        let response = ui.put(rect, egui::Button::new("").frame(false));
        let enabled = ui.is_enabled();
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        response.widget_info(move || {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                enabled,
                tr.format("{key} key", &[("key", label)]),
            )
        });

        let hovered = response.hovered() || response.has_focus();
        let visuals = ui.style().interact(&response);
        let fill = if hovered {
            visuals.weak_bg_fill
        } else {
            self.material_theme.surface_container_high
        };
        let outline = virtual_control_outline(
            self.material_theme.mode,
            self.material_theme.outline,
            self.material_theme.surface_container_high,
        );
        let color = if hovered {
            visuals.fg_stroke.color
        } else {
            self.material_theme.on_surface
        };
        let visual_rect =
            Rect::from_center_size(rect.center(), Vec2::splat(number_button_size(rect.width())));
        let corner_radius = control_corner_radius(
            f32::from(VIRTUAL_BUTTON_CORNER_RADIUS),
            corner_radius_percent,
        );
        ui.painter().rect_filled(visual_rect, corner_radius, fill);
        ui.painter().rect_stroke(
            visual_rect,
            corner_radius,
            egui::Stroke::new(NUMBER_ROW_OUTLINE_WIDTH, outline),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            visual_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional((rect.width() * 0.48).clamp(11.0, 18.0)),
            color,
        );
    }

    pub(super) fn virtual_direction_button(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        direction: DirectionIcon,
        action: super::TouchOwner,
        corner_radius_percent: u8,
    ) {
        self.control_regions.push((rect, action));
        let response = ui.put(rect, egui::Button::new("").frame(false));
        let enabled = ui.is_enabled();
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                enabled,
                crate::i18n::Translator::from_context(ui.ctx()).text(
                    direction.accessibility_label(matches!(
                        action,
                        super::TouchOwner::VirtualKeyPair(..)
                    )),
                ),
            )
        });
        if response.hovered() || response.has_focus() {
            ui.painter().rect_filled(
                rect.shrink(2.0),
                control_corner_radius(
                    rect.width().min(rect.height()) * 0.36,
                    corner_radius_percent,
                ),
                self.material_theme.primary.gamma_multiply_u8(24),
            );
        }
        let color = ui.style().interact(&response).fg_stroke.color;
        paint_direction_icon(ui.painter(), rect, direction, color);
    }

    pub(super) fn virtual_icon_button(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        accessibility_label: &'static str,
        icon: VirtualControlIcon,
        action: HostAction,
        corner_radius_percent: u8,
    ) {
        self.control_regions
            .push((rect, super::TouchOwner::VirtualKey(Some(action))));
        let response = ui.put(rect, egui::Button::new("").frame(false));
        let enabled = ui.is_enabled();
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                enabled,
                crate::i18n::Translator::from_context(ui.ctx()).text(accessibility_label),
            )
        });
        let hovered = response.hovered() || response.has_focus();
        let visuals = ui.style().interact(&response);
        let fill = if hovered {
            visuals.weak_bg_fill
        } else {
            self.material_theme.surface_container_high
        };
        let stroke = if hovered {
            visuals.bg_stroke
        } else {
            egui::Stroke::new(
                1.25,
                virtual_control_outline(
                    self.material_theme.mode,
                    self.material_theme.outline,
                    self.material_theme.surface_container_high,
                ),
            )
        };
        let color = if hovered {
            visuals.fg_stroke.color
        } else {
            self.material_theme.on_surface
        };
        let radius = rect.width().min(rect.height()) / 2.0;
        let radius = (radius - 0.75).max(0.0);
        if corner_radius_percent == 100 {
            ui.painter().circle_filled(rect.center(), radius, fill);
            ui.painter().circle_stroke(rect.center(), radius, stroke);
        } else {
            let visual_rect = Rect::from_center_size(rect.center(), Vec2::splat(radius * 2.0));
            let corner_radius = control_corner_radius(radius, corner_radius_percent);
            ui.painter().rect_filled(visual_rect, corner_radius, fill);
            ui.painter()
                .rect_stroke(visual_rect, corner_radius, stroke, egui::StrokeKind::Middle);
        }
        paint_virtual_control_icon(ui.painter(), rect, icon, color);
    }
}
