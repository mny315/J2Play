use super::{
    Color32, FrontendApp, GameplayScreen, Pos2, Rect, RichText, SessionState, Vec2, egui,
    material_gameplay_app_bar_frame,
};

pub(super) const GAMEPLAY_TOOLBAR_HEIGHT: f32 = 36.0;
const GAMEPLAY_TOOLBAR_ACTION_WIDTH: f32 = 44.0;
const GAMEPLAY_TOOLBAR_TITLE_SIZE: f32 = 18.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GameplayToolbarRequest {
    Stop,
    RotateScreen,
    ToggleFastForward,
    ToggleRuntimeDebug,
    ToggleFullscreen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EmulatorIcon {
    Fullscreen,
    FastForward,
    Debug,
    Rotate,
    Stop,
}

impl FrontendApp {
    pub(super) fn draw_animated_gameplay_toolbar(
        &self,
        ui: &mut egui::Ui,
        state: SessionState,
    ) -> Option<GameplayToolbarRequest> {
        let super::Screen::Gameplay(gameplay) = &self.screen else {
            return None;
        };
        let fraction = self.gameplay_transition.toolbar();
        if fraction <= 0.0 {
            return None;
        }
        if fraction >= 1.0 {
            let request = self.draw_gameplay_toolbar(ui, gameplay, state);
            ui.add_space(8.0);
            return request;
        }
        let frame = material_gameplay_app_bar_frame(&self.material_theme);
        let height = GAMEPLAY_TOOLBAR_HEIGHT + frame.total_margin().sum().y;
        let origin = ui.next_widget_position();
        let size = Vec2::new(ui.available_width(), height);
        let rect = Rect::from_min_size(origin - Vec2::new(0.0, height * (1.0 - fraction)), size);
        let mut toolbar_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        toolbar_ui.set_clip_rect(
            ui.clip_rect()
                .intersect(Rect::from_min_size(origin, size * Vec2::new(1.0, fraction))),
        );
        toolbar_ui.multiply_opacity(fraction);
        let request = self.draw_gameplay_toolbar(&mut toolbar_ui, gameplay, state);
        ui.add_space((height + ui.spacing().item_spacing.y + 8.0) * fraction);
        request
    }

    pub(super) fn draw_gameplay_toolbar(
        &self,
        ui: &mut egui::Ui,
        gameplay: &GameplayScreen,
        state: SessionState,
    ) -> Option<GameplayToolbarRequest> {
        let frame = material_gameplay_app_bar_frame(&self.material_theme);
        frame
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().interact_size =
                    Vec2::new(GAMEPLAY_TOOLBAR_ACTION_WIDTH, GAMEPLAY_TOOLBAR_HEIGHT);
                ui.spacing_mut().item_spacing.x = gameplay_toolbar_spacing(
                    ui.available_width(),
                    self.platform.orientation_control() == super::OrientationControl::HostWindow,
                );
                ui.style_mut().visuals.widgets.hovered.expansion = 0.0;
                let outline = egui::Stroke::new(1.0, self.material_theme.outline);
                ui.visuals_mut().widgets.inactive.bg_stroke = outline;
                ui.visuals_mut().widgets.noninteractive.bg_stroke = outline;
                let (row, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), GAMEPLAY_TOOLBAR_HEIGHT),
                    egui::Sense::hover(),
                );
                self.draw_gameplay_toolbar_row(ui, row, gameplay, state)
            })
            .inner
    }

    pub(super) fn draw_gameplay_toolbar_row(
        &self,
        ui: &mut egui::Ui,
        row: Rect,
        gameplay: &GameplayScreen,
        state: SessionState,
    ) -> Option<GameplayToolbarRequest> {
        let actions = gameplay_toolbar_geometry(
            row,
            ui.spacing().item_spacing.x,
            self.platform.orientation_control() == super::OrientationControl::HostWindow,
        );
        paint_gameplay_title(ui, actions.title, &gameplay.title);
        let mut request = gameplay_icon_button(
            ui,
            actions.fullscreen,
            "Enter fullscreen (F11; Back exits)",
            true,
            false,
            EmulatorIcon::Fullscreen,
        )
        .then_some(GameplayToolbarRequest::ToggleFullscreen);

        let fast_forward_enabled = state == SessionState::Running;
        let fast_forward_label = if fast_forward_enabled {
            "Fast-forward"
        } else {
            "Fast-forward is available while the game is running"
        };
        if gameplay_icon_button(
            ui,
            actions.fast_forward,
            fast_forward_label,
            fast_forward_enabled,
            gameplay.fast_forward,
            EmulatorIcon::FastForward,
        ) {
            request = Some(GameplayToolbarRequest::ToggleFastForward);
        }

        if gameplay_icon_button(
            ui,
            actions.debug,
            "Toggle runtime debug HUD (F10)",
            true,
            self.runtime_debug.enabled,
            EmulatorIcon::Debug,
        ) {
            request = Some(GameplayToolbarRequest::ToggleRuntimeDebug);
        }

        if let Some(rotate) = actions.rotate
            && gameplay_icon_button(
                ui,
                rotate,
                "Rotate screen",
                true,
                false,
                EmulatorIcon::Rotate,
            )
        {
            request = Some(GameplayToolbarRequest::RotateScreen);
        }

        if gameplay_icon_button(
            ui,
            actions.stop,
            "Stop game and return to the library",
            true,
            false,
            EmulatorIcon::Stop,
        ) {
            request = Some(GameplayToolbarRequest::Stop);
        }
        request
    }
}

fn gameplay_toolbar_action_count(rotate_screen: bool) -> f32 {
    if rotate_screen { 5.0 } else { 4.0 }
}

fn gameplay_toolbar_title_width(
    available_width: f32,
    item_spacing: f32,
    rotate_screen: bool,
) -> f32 {
    let action_count = gameplay_toolbar_action_count(rotate_screen);
    (available_width - GAMEPLAY_TOOLBAR_ACTION_WIDTH * action_count - item_spacing * action_count)
        .max(0.0)
}

pub(super) fn gameplay_toolbar_spacing(available_width: f32, rotate_screen: bool) -> f32 {
    let action_count = gameplay_toolbar_action_count(rotate_screen);
    ((available_width - GAMEPLAY_TOOLBAR_ACTION_WIDTH * action_count) / action_count)
        .clamp(0.0, 4.0)
}

pub(super) struct GameplayToolbarGeometry {
    pub title: Rect,
    pub fullscreen: Rect,
    pub fast_forward: Rect,
    pub debug: Rect,
    pub rotate: Option<Rect>,
    pub stop: Rect,
}

pub(super) fn gameplay_toolbar_geometry(
    row: Rect,
    spacing: f32,
    rotate_screen: bool,
) -> GameplayToolbarGeometry {
    let action_step = GAMEPLAY_TOOLBAR_ACTION_WIDTH + spacing;
    let right_center = row.right() - GAMEPLAY_TOOLBAR_ACTION_WIDTH / 2.0;
    let action = |from_right| {
        Rect::from_center_size(
            Pos2::new(right_center - from_right * action_step, row.center().y),
            Vec2::new(GAMEPLAY_TOOLBAR_ACTION_WIDTH, GAMEPLAY_TOOLBAR_HEIGHT),
        )
    };
    let title = Rect::from_min_size(
        row.left_top(),
        Vec2::new(
            gameplay_toolbar_title_width(row.width(), spacing, rotate_screen),
            row.height(),
        ),
    );
    let rotation_slot = if rotate_screen { 1.0 } else { 0.0 };
    GameplayToolbarGeometry {
        title,
        fullscreen: action(3.0 + rotation_slot),
        fast_forward: action(2.0 + rotation_slot),
        debug: action(1.0 + rotation_slot),
        rotate: rotate_screen.then(|| action(1.0)),
        stop: action(0.0),
    }
}

fn paint_gameplay_title(ui: &mut egui::Ui, rect: Rect, title: &str) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.shrink_clip_rect(rect);
            ui.add(
                egui::Label::new(RichText::new(title).size(GAMEPLAY_TOOLBAR_TITLE_SIZE))
                    .truncate()
                    .halign(egui::Align::Min),
            )
            .on_hover_text(title);
        },
    );
}

fn gameplay_icon_button(
    ui: &mut egui::Ui,
    rect: Rect,
    accessibility_label: &'static str,
    enabled: bool,
    selected: bool,
    icon: EmulatorIcon,
) -> bool {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let mut builder = egui::UiBuilder::new().max_rect(rect);
    if !enabled {
        builder = builder.disabled();
    }
    let response = ui
        .scope_builder(builder, |ui| {
            let response = ui.put(
                rect,
                egui::Button::new("")
                    .selected(selected)
                    .corner_radius(GAMEPLAY_TOOLBAR_HEIGHT / 2.0),
            );
            let enabled = ui.is_enabled();
            response.widget_info(move || {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    enabled,
                    selected,
                    tr.text(accessibility_label),
                )
            });
            response
        })
        .inner;
    let color = ui.style().interact(&response).fg_stroke.color;
    paint_emulator_icon(ui.painter(), response.rect, icon, color);
    let response = response.on_hover_text(tr.text(accessibility_label));
    // egui expands pointer hit targets beyond their clip rect. A sliding
    // toolbar must not accept clicks in the part that has already disappeared.
    response.clicked()
        && (!response.clicked_by(egui::PointerButton::Primary)
            || response
                .interact_pointer_pos()
                .is_some_and(|position| ui.clip_rect().contains(position)))
}

fn paint_emulator_icon(painter: &egui::Painter, rect: Rect, icon: EmulatorIcon, color: Color32) {
    let shortest = rect.width().min(rect.height());
    let stroke = egui::Stroke::new((shortest * 0.045).clamp(1.5, 2.0), color);
    let center = rect.center();
    match icon {
        EmulatorIcon::Fullscreen => {
            for segment in fullscreen_icon_segments(rect) {
                painter.line_segment(segment, stroke);
            }
        }
        EmulatorIcon::FastForward => {
            let radius = emulator_icon_radius(rect);
            painter.circle_stroke(center, radius, stroke);
            for segment in speedometer_segments(rect) {
                painter.line_segment(segment, stroke);
            }
            painter.circle_filled(center, stroke.width, color);
        }
        EmulatorIcon::Debug => {
            // Antennae reach 0.23 above the body, legs only 0.12 below it.
            // Center the complete silhouette inside the toolbar button.
            let rect = rect.translate(Vec2::new(0.0, shortest * 0.055));
            let center = rect.center();
            painter.circle_filled(center, shortest * 0.13, color);
            painter.circle_filled(
                center - Vec2::new(0.0, shortest * 0.13),
                shortest * 0.075,
                color,
            );
            for segment in debug_icon_segments(rect) {
                painter.line_segment(segment, stroke);
            }
        }
        EmulatorIcon::Rotate => {
            for segment in rotate_icon_segments(rect) {
                painter.line_segment(segment, stroke);
            }
        }
        EmulatorIcon::Stop => {
            painter.rect_filled(stop_icon_rect(rect), 1.0, color);
        }
    }
}

fn fullscreen_icon_segments(rect: Rect) -> [[Pos2; 2]; 8] {
    let half = rect.width().min(rect.height()) * 0.22;
    let arm = half * 0.62;
    let left = rect.center().x - half;
    let right = rect.center().x + half;
    let top = rect.center().y - half;
    let bottom = rect.center().y + half;
    [
        [Pos2::new(left, top + arm), Pos2::new(left, top)],
        [Pos2::new(left, top), Pos2::new(left + arm, top)],
        [Pos2::new(right - arm, top), Pos2::new(right, top)],
        [Pos2::new(right, top), Pos2::new(right, top + arm)],
        [Pos2::new(right, bottom - arm), Pos2::new(right, bottom)],
        [Pos2::new(right, bottom), Pos2::new(right - arm, bottom)],
        [Pos2::new(left + arm, bottom), Pos2::new(left, bottom)],
        [Pos2::new(left, bottom), Pos2::new(left, bottom - arm)],
    ]
}

fn emulator_icon_radius(rect: Rect) -> f32 {
    rect.width().min(rect.height()) * 0.22
}

fn speedometer_segments(rect: Rect) -> [[Pos2; 2]; 4] {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    let center = rect.center();
    let radius = emulator_icon_radius(rect);
    let tick = |angle: f32| {
        let direction = Vec2::new(angle.cos(), angle.sin());
        [
            center + direction * (radius * 0.62),
            center + direction * (radius * 0.86),
        ]
    };
    let needle_direction = Vec2::new(FRAC_PI_4.cos(), -FRAC_PI_4.sin());
    [
        tick(PI),
        tick(-FRAC_PI_2),
        tick(0.0),
        [center, center + needle_direction * (radius * 0.72)],
    ]
}

fn debug_icon_segments(rect: Rect) -> [[Pos2; 2]; 8] {
    let shortest = rect.width().min(rect.height());
    let center = rect.center();
    let point = |x: f32, y: f32| center + Vec2::new(shortest * x, shortest * y);
    [
        [point(-0.05, -0.16), point(-0.12, -0.23)],
        [point(0.05, -0.16), point(0.12, -0.23)],
        [point(-0.11, -0.07), point(-0.21, -0.12)],
        [point(-0.13, 0.00), point(-0.23, 0.00)],
        [point(-0.11, 0.07), point(-0.21, 0.12)],
        [point(0.11, -0.07), point(0.21, -0.12)],
        [point(0.13, 0.00), point(0.23, 0.00)],
        [point(0.11, 0.07), point(0.21, 0.12)],
    ]
}

fn rotate_icon_segments(rect: Rect) -> [[Pos2; 2]; 12] {
    let shortest = rect.width().min(rect.height());
    let point = |x: f32, y: f32| rect.center() + Vec2::new(shortest * x, shortest * y);
    [
        // Tilted phone, with opposing arrows showing the quarter turn.
        [point(-0.16, -0.04), point(-0.04, -0.16)],
        [point(-0.04, -0.16), point(0.16, 0.04)],
        [point(0.16, 0.04), point(0.04, 0.16)],
        [point(0.04, 0.16), point(-0.16, -0.04)],
        [point(-0.23, -0.08), point(-0.23, -0.23)],
        [point(-0.23, -0.23), point(0.02, -0.23)],
        [point(0.02, -0.23), point(-0.04, -0.29)],
        [point(0.02, -0.23), point(-0.04, -0.17)],
        [point(0.23, 0.08), point(0.23, 0.23)],
        [point(0.23, 0.23), point(-0.02, 0.23)],
        [point(-0.02, 0.23), point(0.04, 0.17)],
        [point(-0.02, 0.23), point(0.04, 0.29)],
    ]
}

fn stop_icon_rect(rect: Rect) -> Rect {
    Rect::from_center_size(
        rect.center(),
        Vec2::splat(rect.width().min(rect.height()) * 0.30),
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/gameplay_toolbar.rs"]
mod tests;
