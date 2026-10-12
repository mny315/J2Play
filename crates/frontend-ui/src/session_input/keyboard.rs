use super::{Event, FrontendApp, HostAction, Key, egui};
use frontend_core::physical_input::PhysicalAction;

impl FrontendApp {
    pub(super) fn process_game_key(
        &mut self,
        ctx: &egui::Context,
        event: &Event,
        deleted: &mut usize,
    ) {
        let Event::Key { key, .. } = *event else {
            return;
        };
        let Some(event) = crate::physical_controls::keyboard_input_event(event) else {
            return;
        };
        if !self.text_input_active {
            self.process_physical_event(ctx, event, true);
            return;
        }

        let enabled = !self.platform_suspended && ctx.input(|input| input.focused);
        let action = enabled
            .then(|| match key {
                Key::F10 => Some(PhysicalAction::DebugOverlay),
                Key::F11 => Some(PhysicalAction::Fullscreen),
                _ => text_edit_action_for_key(key).map(PhysicalAction::Phone),
            })
            .flatten();
        // Editor mappings replace gameplay bindings, while all input sources
        // retain the same first-press/last-release and neutral-after-barrier rules.
        for change in self
            .physical_input
            .update(event, self.physical_bindings.dead_zone_percent)
        {
            if enabled && change.pressed && matches!(key, Key::Backspace | Key::Delete) {
                self.submit_text_deletions(
                    usize::from(key == Key::Backspace),
                    usize::from(key == Key::Delete),
                    deleted,
                );
            }
            if let Some((action, pressed)) = self.physical_input.resolve_action(change, action) {
                self.apply_physical_action(action, pressed);
            }
        }
    }
}

fn text_edit_action_for_key(key: Key) -> Option<HostAction> {
    Some(match key {
        Key::ArrowUp => HostAction::Up,
        Key::ArrowDown => HostAction::Down,
        Key::ArrowLeft => HostAction::Left,
        Key::ArrowRight => HostAction::Right,
        Key::Enter => HostAction::Fire,
        Key::F1 => HostAction::SoftLeft,
        Key::F2 => HostAction::SoftRight,
        Key::F3 => HostAction::Back,
        _ => return None,
    })
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/session_input/keyboard.rs"]
mod tests;
