use super::labels::{action_label, control_label};
use super::selection::{BindingTarget, Picker};
use super::{HostAction, PhysicalAction, PhysicalBindings, PhysicalControl, egui};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(super) enum KeyGroup {
    #[default]
    Letters,
    Symbols,
    Navigation,
    Numpad,
    Function,
    Modifiers,
    Assigned,
}

impl KeyGroup {
    pub(super) const ALL: [Self; 7] = [
        Self::Letters,
        Self::Symbols,
        Self::Navigation,
        Self::Numpad,
        Self::Function,
        Self::Modifiers,
        Self::Assigned,
    ];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Letters => "Letters",
            Self::Symbols => "Numbers & symbols",
            Self::Navigation => "Navigation & editing",
            Self::Numpad => "Number pad",
            Self::Function => "Function keys",
            Self::Modifiers => "Modifiers",
            Self::Assigned => "Assigned keys",
        }
    }

    pub(super) fn usages(self, bindings: &PhysicalBindings) -> Vec<u16> {
        match self {
            Self::Letters => (4..=29).collect(),
            Self::Symbols => (30..=39).chain(45..=49).chain(51..=56).collect(),
            Self::Navigation => (40..=44).chain([57]).chain(70..=82).collect(),
            Self::Numpad => (83..=99).collect(),
            Self::Function => (58..=69).collect(),
            Self::Modifiers => (224..=231).collect(),
            Self::Assigned => {
                let mut usages: Vec<_> = bindings
                    .bindings
                    .iter()
                    .filter_map(|binding| match binding.control {
                        PhysicalControl::Keyboard { usage } => Some(usage),
                        _ => None,
                    })
                    .collect();
                usages.sort_unstable();
                usages
            }
        }
    }
}

// Names cover the keyboard usages already normalized by the Android shell,
// including keys that egui does not represent (modifiers and the number pad).
pub(super) fn key_label(usage: u16) -> String {
    match usage {
        4..=29 => {
            return char::from_u32(u32::from(usage) + u32::from(b'A') - 4)
                .unwrap_or('?')
                .to_string();
        }
        30..=38 => return (usage - 29).to_string(),
        39 => "0",
        40 => "Enter",
        41 => "Esc",
        42 => "Backspace",
        43 => "Tab",
        44 => "Space",
        45 => "-",
        46 => "=",
        47 => "[",
        48 => "]",
        49 => "\\",
        51 => ";",
        52 => "'",
        53 => "`",
        54 => ",",
        55 => ".",
        56 => "/",
        57 => "Caps Lock",
        58..=69 => return format!("F{}", usage - 57),
        70 => "Print Screen",
        71 => "Scroll Lock",
        72 => "Pause",
        73 => "Insert",
        74 => "Home",
        75 => "Page Up",
        76 => "Delete",
        77 => "End",
        78 => "Page Down",
        79 => "Right",
        80 => "Left",
        81 => "Down",
        82 => "Up",
        83 => "Num Lock",
        84 => "Num /",
        85 => "Num *",
        86 => "Num -",
        87 => "Num +",
        88 => "Num Enter",
        89..=97 => return format!("Num {}", usage - 88),
        98 => "Num 0",
        99 => "Num .",
        224 => "Left Ctrl",
        225 => "Left Shift",
        226 => "Left Alt",
        227 => "Left Meta",
        228 => "Right Ctrl",
        229 => "Right Shift",
        230 => "Right Alt",
        231 => "Right Meta",
        _ => return format!("Key {usage}"),
    }
    .to_owned()
}

fn keycap_label(usage: u16) -> String {
    match usage {
        42 => "Bksp",
        57 => "Caps",
        70 => "PrtSc",
        71 => "ScrLk",
        75 => "PgUp",
        78 => "PgDn",
        83 => "NumLk",
        88 => "Num Ent",
        224 => "L Ctrl",
        225 => "L Shift",
        226 => "L Alt",
        227 => "L Meta",
        228 => "R Ctrl",
        229 => "R Shift",
        230 => "R Alt",
        231 => "R Meta",
        _ => return key_label(usage),
    }
    .to_owned()
}

fn assignment_label(action: Option<PhysicalAction>) -> &'static str {
    match action {
        None => "×",
        Some(PhysicalAction::Menu) => "App menu",
        Some(PhysicalAction::Fullscreen) => "Fullscreen",
        Some(PhysicalAction::FastForward) => "Fast-fwd",
        Some(PhysicalAction::DebugOverlay) => "Debug",
        Some(PhysicalAction::Phone(HostAction::SoftLeft)) => "L soft",
        Some(PhysicalAction::Phone(HostAction::SoftRight)) => "R soft",
        Some(PhysicalAction::Phone(HostAction::Back)) => "Back",
        Some(action) => action_label(action),
    }
}

fn paint_label(ui: &egui::Ui, rect: egui::Rect, label: &str, size: f32, color: egui::Color32) {
    let mut job = egui::text::LayoutJob::simple(
        label.to_owned(),
        egui::FontId::proportional(size),
        color,
        rect.width(),
    );
    job.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(job);
    let pos = rect.center() - galley.size() * 0.5;
    ui.painter().galley(pos, galley, color);
}

/// Real egui buttons retain keyboard/gamepad activation and accessibility.
/// Each tile shows both the physical key and its current assignment.
pub(super) fn draw_grid(
    ui: &mut egui::Ui,
    theme: &crate::MaterialTheme,
    bindings: &PhysicalBindings,
    controls: &[PhysicalControl],
) -> Option<Picker> {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let gap = 6.0;
    let columns = (1..=8_u8)
        .rev()
        .find(|columns| ui.available_width() + gap >= f32::from(*columns) * (64.0 + gap))
        .unwrap_or(1);
    let width = (ui.available_width() - gap * f32::from(columns - 1)) / f32::from(columns);
    let mut selected = None;
    let grid_id = ui.make_persistent_id("physical-key-grid");
    // Rows use the current width immediately; egui::Grid retains column sizes
    // from the previous orientation. Explicit IDs keep focus on the same key
    // when the number of columns or the Assigned filter changes.
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
        for row in controls.chunks(usize::from(columns)) {
            ui.horizontal(|ui| {
                for control in row.iter().copied() {
                    ui.scope_builder(egui::UiBuilder::new().id(grid_id.with(control)), |ui| {
                        let action = bindings.resolve(control);
                        let target = BindingTarget::from_control(control);
                        let label = match control {
                            PhysicalControl::Keyboard { usage } => keycap_label(usage),
                            PhysicalControl::AndroidKey { code: 24 } => "Vol +".to_owned(),
                            PhysicalControl::AndroidKey { code: 25 } => "Vol -".to_owned(),
                            PhysicalControl::Gamepad {
                                button: super::GamepadButton::Extra(number),
                            } => format!("Extra {number}"),
                            _ => control_label(control),
                        };
                        let response = ui.add_sized(
                            [width, 64.0],
                            egui::Button::new("")
                                .fill(if action.is_some() {
                                    theme.secondary_container
                                } else {
                                    theme.surface_container
                                })
                                .stroke(egui::Stroke::new(1.0, theme.outline_variant))
                                .corner_radius(12.0),
                        );
                        let current = format!(
                            "{} - {}",
                            tr.control(&target.label()),
                            tr.control(&target.assignment(bindings))
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                &current,
                            )
                        });
                        let rect = response.rect.shrink(5.0);
                        let color = if action.is_some() {
                            theme.on_secondary_container
                        } else {
                            theme.on_surface
                        };
                        paint_label(
                            ui,
                            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 28.0)),
                            &if matches!(control, PhysicalControl::Keyboard { .. }) {
                                label
                            } else {
                                tr.control(&label)
                            },
                            16.0,
                            color,
                        );
                        paint_label(
                            ui,
                            egui::Rect::from_min_max(
                                egui::pos2(rect.left(), rect.top() + 30.0),
                                rect.max,
                            ),
                            &tr.text(assignment_label(action)),
                            if action.is_some() { 11.0 } else { 16.0 },
                            color,
                        );
                        crate::focus_ring::track(ui, &response, 12.0);
                        if response.clicked() {
                            selected = Some(Picker::Binding(target));
                        }
                    });
                }
            });
        }
    });
    selected
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    theme: &crate::MaterialTheme,
    bindings: &PhysicalBindings,
    group: &mut KeyGroup,
) -> Option<Picker> {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    // A short category menu leaves room for touch-sized keys in portrait.
    let combo_id = ui.make_persistent_id(egui::IdSalt::new("keyboard-group"));
    let combo = egui::ComboBox::from_id_salt("keyboard-group")
        .selected_text(tr.text(group.label()))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let mut menu = crate::focus_navigation::ComboMenu::begin(ui, combo_id);
            for choice in KeyGroup::ALL {
                let selected = *group == choice;
                let response = ui.add_sized(
                    [ui.available_width(), crate::SETTINGS_TOUCH_TARGET_HEIGHT],
                    crate::material_choice_button(tr.text(choice.label()), *group == choice),
                );
                menu.item(ui, &response, selected);
                if response.clicked() {
                    *group = choice;
                    ui.close();
                }
            }
            menu.finish(ui);
        });
    crate::focus_ring::track(ui, &combo.response, 12.0);
    ui.add_space(8.0);
    let controls: Vec<_> = group
        .usages(bindings)
        .into_iter()
        .map(|usage| PhysicalControl::Keyboard { usage })
        .collect();
    if controls.is_empty() {
        ui.label(tr.text("No keyboard keys assigned."));
    }
    let selected = draw_grid(ui, theme, bindings, &controls);
    ui.add_space(8.0);
    selected
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/physical_controls/keyboard.rs"]
mod tests;
