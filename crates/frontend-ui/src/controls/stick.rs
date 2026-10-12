//! Fixed touch stick: presentation and bounded conversion to the keypad's keys.

use crate::{
    DIRECTION_PAD_BUTTONS, FrontendApp, HostAction, MaterialTheme, Pos2, Rect, TouchOwner, Vec2,
    VirtualControlGeometry, egui,
};

const DEAD_ZONE_ENTER: f32 = 0.22;
const DEAD_ZONE_EXIT: f32 = 0.16;
const KNOB_TRAVEL: f32 = 0.60;

impl TouchOwner {
    pub(crate) fn direction_key(action: Option<HostAction>, two_key_diagonals: bool) -> Self {
        use HostAction::{Num1, Num2, Num3, Num4, Num6, Num7, Num8, Num9};
        match (action, two_key_diagonals) {
            (Some(Num1), true) => Self::VirtualKeyPair(Num2, Num4),
            (Some(Num3), true) => Self::VirtualKeyPair(Num2, Num6),
            (Some(Num7), true) => Self::VirtualKeyPair(Num8, Num4),
            (Some(Num9), true) => Self::VirtualKeyPair(Num8, Num6),
            _ => Self::VirtualKey(action),
        }
    }

    pub(crate) fn virtual_actions(self) -> [Option<HostAction>; 2] {
        match self {
            Self::VirtualKey(action) => [action, None],
            Self::VirtualKeyPair(first, second) => [Some(first), Some(second)],
            Self::VirtualStick {
                action,
                two_key_diagonals,
                ..
            } => Self::direction_key(action, two_key_diagonals).virtual_actions(),
            Self::Canvas { .. } => [None, None],
        }
    }
}

pub(crate) fn contains(bounds: Rect, position: Pos2) -> bool {
    let radius = bounds.width().min(bounds.height()) * 0.5;
    radius > 0.0 && position.distance_sq(bounds.center()) <= radius * radius
}

pub(crate) fn touch_owner(
    bounds: Rect,
    position: Pos2,
    previous: Option<HostAction>,
    two_key_diagonals: bool,
) -> TouchOwner {
    let travel = bounds.width().min(bounds.height()) * 0.5 * KNOB_TRAVEL;
    let delta = position - bounds.center();
    if !travel.is_finite() || travel <= 0.0 || !delta.is_finite() {
        return TouchOwner::VirtualStick {
            action: None,
            offset: Vec2::ZERO,
            two_key_diagonals,
        };
    }
    let offset = delta / travel;
    let length = offset.length();
    if !length.is_finite() {
        return TouchOwner::VirtualStick {
            action: None,
            offset: Vec2::ZERO,
            two_key_diagonals,
        };
    }
    let offset = offset / length.max(1.0);
    let threshold = if previous.is_some() {
        DEAD_ZONE_EXIT
    } else {
        DEAD_ZONE_ENTER
    };
    let action = (length > threshold)
        .then(|| {
            let direction = offset.normalized();
            // Retain the current sector through small finger jitter at its edge.
            // Thirty degrees gives each 45-degree sector 7.5 degrees of hysteresis.
            let retained = DIRECTION_PAD_BUTTONS.iter().find(|(_, action, x, y)| {
                Some(*action) == previous
                    && direction.dot(Vec2::new(*x, *y).normalized()) >= 30_f32.to_radians().cos()
            });
            retained
                .or_else(|| {
                    DIRECTION_PAD_BUTTONS.iter().max_by(|a, b| {
                        direction
                            .dot(Vec2::new(a.2, a.3).normalized())
                            .total_cmp(&direction.dot(Vec2::new(b.2, b.3).normalized()))
                    })
                })
                .map(|(_, action, _, _)| *action)
        })
        .flatten();
    TouchOwner::VirtualStick {
        action,
        offset,
        two_key_diagonals,
    }
}

pub(crate) fn paint(
    painter: &egui::Painter,
    theme: &MaterialTheme,
    bounds: Rect,
    offset: Vec2,
    active: bool,
    visible: bool,
) {
    let radius = bounds.width().min(bounds.height()) * 0.5;
    let center = bounds.center();
    let opacity = if visible { 1.0 } else { 0.4 };
    let fill = theme.surface_container_high.gamma_multiply(opacity);
    let outline =
        super::virtual_control_outline(theme.mode, theme.outline, fill).gamma_multiply(opacity);
    painter.circle(
        center,
        (radius - 0.75).max(0.0),
        fill,
        egui::Stroke::new(1.25, outline),
    );
    painter.circle_stroke(
        center,
        radius * KNOB_TRAVEL,
        egui::Stroke::new(1.0, outline.gamma_multiply(0.55)),
    );
    for delta in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
        painter.line_segment(
            [
                center + delta * radius * 0.81,
                center + delta * radius * 0.9,
            ],
            egui::Stroke::new(2.0, outline),
        );
    }
    let knob = center + offset * radius * KNOB_TRAVEL;
    painter.circle(
        knob,
        radius * 0.32,
        if active {
            theme.primary_container
        } else {
            theme.surface_container_highest
        }
        .gamma_multiply(opacity),
        egui::Stroke::new(
            1.5,
            if active {
                theme.primary
            } else {
                theme.on_surface_variant
            }
            .gamma_multiply(opacity),
        ),
    );
    painter.circle_filled(
        knob,
        radius * 0.055,
        theme.on_surface_variant.gamma_multiply(opacity),
    );
}

impl FrontendApp {
    pub(crate) fn draw_virtual_stick(
        &mut self,
        ui: &mut egui::Ui,
        geometry: VirtualControlGeometry,
        two_key_diagonals: bool,
    ) {
        let bounds = Rect::from_center_size(
            geometry.direction_pad_center,
            Vec2::splat(geometry.direction_button_size * 3.0),
        );
        self.stick_region = Some((bounds, two_key_diagonals));
        let offset = self.touch_owners.values().find_map(|owner| match owner {
            TouchOwner::VirtualStick { offset, .. } => Some(*offset),
            _ => None,
        });
        let response = ui.interact(bounds, ui.id().with("virtual-stick"), egui::Sense::drag());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Other,
                ui.is_enabled(),
                crate::i18n::Translator::from_context(ui.ctx()).text("Virtual stick"),
            )
        });
        paint(
            ui.painter(),
            &self.material_theme,
            bounds,
            offset.unwrap_or_default(),
            offset.is_some(),
            true,
        );
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/controls/stick.rs"]
mod tests;
