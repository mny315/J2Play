use super::{
    Category, EmuError, FrontendApp, ImeComposition, InputEvent, MAX_PLATFORM_EVENTS_PER_TICK,
    MAX_TEXT_INPUT_BYTES, PlatformTextInputEvent, SessionState,
};

impl FrontendApp {
    pub(super) fn process_platform_text_input(&mut self, ctx: &super::egui::Context) {
        let mut deleted_chars = 0;
        for _ in 0..MAX_PLATFORM_EVENTS_PER_TICK {
            let Some(result) = self.platform.poll_text_input() else {
                break;
            };
            let event = match result {
                Ok(event) => event,
                Err(error) => {
                    self.show_error("Text input failed", &error);
                    continue;
                }
            };
            if self.folder_name_editing(ctx) {
                self.library_folders.native_input(ctx, event);
                ctx.request_repaint();
                break;
            }
            if self.library_search.editing
                && self.library_search.open
                && matches!(self.screen, super::Screen::Library)
                && !self.overlay_active()
                && !self.platform_suspended
                && ctx.memory(super::egui::Memory::focused)
                    == Some(super::library_search::field_id())
            {
                self.library_search.native_input(ctx, event);
                // Apply each editor transaction before reading its cursor for
                // the next one (Android deletion offsets are UTF-16 units).
                ctx.request_repaint();
                break;
            }
            if !self.text_input_active
                || self.session.state() != SessionState::Running
                || self.overlay_active()
            {
                self.ime_composition = None;
                continue;
            }
            self.handle_platform_text_event(event, &mut deleted_chars);
        }
    }

    fn handle_platform_text_event(
        &mut self,
        event: PlatformTextInputEvent,
        deleted_chars: &mut usize,
    ) {
        match event {
            PlatformTextInputEvent::Key(action) => {
                let action = frontend_core::physical_input::PhysicalAction::Phone(action);
                if self.physical_input.action_active(action) {
                    return;
                }
                // One editor action is a complete keystroke. Its release
                // cannot be lost when the press closes the active editor.
                for pressed in [true, false] {
                    self.apply_physical_action(action, pressed);
                }
            }
            PlatformTextInputEvent::Composition {
                text,
                selection_start,
                selection_end,
            } => {
                if text.len() > MAX_TEXT_INPUT_BYTES {
                    self.show_error(
                        "Text input failed",
                        &EmuError::new(
                            Category::Platform,
                            "ime-composition-limit",
                            "IME composition exceeds 4096 bytes",
                        ),
                    );
                } else if text.is_empty() {
                    self.set_ime_composition(None);
                } else {
                    self.set_ime_composition(Some(ImeComposition {
                        text,
                        selection_start,
                        selection_end,
                    }));
                }
            }
            PlatformTextInputEvent::Commit(text) => {
                if text.is_empty() {
                    self.set_ime_composition(None);
                } else {
                    self.ime_composition = None;
                    self.submit_input(InputEvent::TextCommit { text });
                }
            }
            PlatformTextInputEvent::Selection { start, end } => {
                if let Some(mut composition) = self.ime_composition.clone() {
                    // Platform selection offsets are intentionally opaque:
                    // Android reports UTF-16 code units, not UTF-8 bytes.
                    composition.selection_start = start;
                    composition.selection_end = end;
                    self.set_ime_composition(Some(composition));
                }
            }
            PlatformTextInputEvent::Cancel => self.set_ime_composition(None),
            PlatformTextInputEvent::DeleteUtf16 { before, after } => {
                let before = before.min(u16::try_from(64 - *deleted_chars).unwrap_or(0));
                let after = after
                    .min(u16::try_from(64 - *deleted_chars - usize::from(before)).unwrap_or(0));
                *deleted_chars += usize::from(before + after);
                self.submit_input(InputEvent::DeleteUtf16 { before, after });
            }
            PlatformTextInputEvent::MoveCaret { offset } => {
                self.submit_input(InputEvent::MoveTextCaret { offset });
            }
            PlatformTextInputEvent::DeleteSurrounding {
                before_chars,
                after_chars,
            } => {
                self.submit_text_deletions(before_chars, after_chars, deleted_chars);
            }
        }
    }

    pub(super) fn set_ime_composition(&mut self, composition: Option<ImeComposition>) {
        if self.ime_composition == composition {
            return;
        }
        let (text, anchor, caret) = composition.as_ref().map_or_else(
            || (String::new(), 0, 0),
            |value| {
                let units = value.text.encode_utf16().count().min(MAX_TEXT_INPUT_BYTES);
                (
                    value.text.clone(),
                    value.selection_start.min(units),
                    value.selection_end.min(units),
                )
            },
        );
        self.submit_input(InputEvent::TextComposition {
            text,
            anchor: u16::try_from(anchor).unwrap_or(0),
            caret: u16::try_from(caret).unwrap_or(0),
        });
        self.ime_composition = composition;
    }

    pub(super) fn set_platform_text_input(&mut self, active: bool) {
        if self.text_input_active != active {
            // Switching between gameplay bindings and text editing changes
            // key ownership just like a remap: release before routing anew.
            self.release_all_input();
        }
        self.text_input_active = active;
        if !active {
            self.ime_composition = None;
        }
        self.sync_platform_text_input();
    }

    pub(super) fn sync_platform_text_input(&mut self) {
        let visible = (self.text_input_active && self.session.state() == SessionState::Running)
            || (self.library_folders.editing && self.folder_dialog_editable())
            || (self.library_search.editing
                && self.library_search.open
                && matches!(self.screen, super::Screen::Library)
                && !self.overlay_active()
                && !self.platform_suspended);
        if self.platform_text_input_visible == Some(visible) {
            return;
        }
        match self.platform.set_text_input_active(visible) {
            Ok(()) => self.platform_text_input_visible = Some(visible),
            Err(error) => self.show_error("Text input failed", &error),
        }
    }

    pub(super) fn folder_dialog_editable(&self) -> bool {
        matches!(
            self.library_folders.dialog,
            Some(super::library_folders::FolderDialog::Edit { .. })
        ) && !self.platform_suspended
            && self.display_error.is_none()
            && !self.exit_confirmation
            && self.data_action.is_none()
    }

    fn folder_name_editing(&self, ctx: &super::egui::Context) -> bool {
        self.library_folders.editing
            && self.folder_dialog_editable()
            && ctx.memory(super::egui::Memory::focused)
                == Some(super::library_folders::name_field_id())
    }
}
