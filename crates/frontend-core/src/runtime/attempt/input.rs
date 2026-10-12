//! Translate host keys, pointers and text edits into bounded guest callbacks.

use super::AttemptDriver;
use crate::runtime::{MAX_TEXT_COMMIT_BYTES, runtime_error};
use crate::{InputEvent, KeyState, PointerEvent, PointerPhase};
use diagnostics::EmuError;
use runtime::{key_call, pointer_call, text_input_call};
use std::collections::VecDeque;

impl AttemptDriver {
    pub(super) fn queue_input(&mut self, event: InputEvent) -> Result<(), EmuError> {
        match event {
            InputEvent::Key { action, state } => {
                if action == platform::HostAction::Quit {
                    self.request_close()?;
                    return Ok(());
                }
                let event = platform::InputEvent {
                    tick: 0,
                    kind: match state {
                        KeyState::Pressed => platform::KeyKind::Pressed,
                        KeyState::Released => platform::KeyKind::Released,
                    },
                    action,
                };
                if let Some(event) = self.input.apply(event) {
                    self.pending
                        .push_back(vm::DriverStep::Call(key_call(&event)));
                }
            }
            InputEvent::Pointer(event) => self.queue_pointer(event),
            InputEvent::TextCommit { text } => {
                if text.len() > MAX_TEXT_COMMIT_BYTES {
                    return Err(runtime_error(
                        "text-input-limit",
                        "committed text exceeds 4096 bytes",
                    ));
                }
                if std::mem::take(&mut self.composing) {
                    self.pending_text.push_back(text_edit_step(1, 0));
                }
                for event in platform::committed_text_events(0, &text) {
                    self.pending_text
                        .push_back(vm::DriverStep::Call(text_input_call(event)));
                }
            }
            InputEvent::TextComposition {
                text,
                anchor,
                caret,
            } => {
                if text.len() > MAX_TEXT_COMMIT_BYTES {
                    return Err(runtime_error(
                        "text-input-limit",
                        "composition exceeds 4096 bytes",
                    ));
                }
                self.composing = !text.is_empty();
                self.pending_text.push_back(text_edit_step(1, 0));
                self.pending_text
                    .extend(text.encode_utf16().map(|unit| text_edit_step(2, unit)));
                self.pending_text.push_back(text_edit_step(3, anchor));
                self.pending_text.push_back(text_edit_step(4, caret));
            }
            InputEvent::DeleteUtf16 { before, after } => {
                self.pending_text
                    .push_back(text_edit_step(5, before.min(64)));
                self.pending_text
                    .push_back(text_edit_step(6, after.min(64 - before.min(64))));
            }
            InputEvent::MoveTextCaret { offset } => {
                self.pending_text
                    .push_back(text_edit_step(7, u16::from_ne_bytes(offset.to_ne_bytes())));
            }
            InputEvent::DeleteBackward | InputEvent::DeleteForward => {
                self.pending_text
                    .push_back(vm::DriverStep::Call(text_input_call(
                        platform::TextInputEvent {
                            tick: 0,
                            kind: if matches!(event, InputEvent::DeleteForward) {
                                platform::TextInputKind::DeleteForward
                            } else {
                                platform::TextInputKind::DeleteBackward
                            },
                            code_unit: 0,
                        },
                    )));
            }
        }
        Ok(())
    }

    fn queue_pointer(&mut self, event: PointerEvent) {
        let dimensions = self.active_canvas_dimensions.get();
        let inside = event.canvas_x >= 0
            && event.canvas_y >= 0
            && u32::try_from(event.canvas_x).is_ok_and(|x| x < dimensions.0)
            && u32::try_from(event.canvas_y).is_ok_and(|y| y < dimensions.1);
        let kind = match event.phase {
            PointerPhase::Pressed if inside && self.captured_pointer.is_none() => {
                platform::PointerKind::PointerPressed
            }
            PointerPhase::Dragged if self.captured_pointer == Some(event.touch_id) => {
                platform::PointerKind::PointerDragged
            }
            PointerPhase::Released | PointerPhase::Cancelled
                if self.captured_pointer == Some(event.touch_id) =>
            {
                platform::PointerKind::PointerReleased
            }
            PointerPhase::Pressed
            | PointerPhase::Dragged
            | PointerPhase::Released
            | PointerPhase::Cancelled => return,
        };
        if !self.pointer.allows(kind) {
            return;
        }
        self.captured_pointer = match kind {
            platform::PointerKind::PointerPressed | platform::PointerKind::PointerDragged => {
                Some(event.touch_id)
            }
            platform::PointerKind::PointerReleased => None,
        };
        self.pointer_position = (event.canvas_x, event.canvas_y);
        self.pending
            .push_back(vm::DriverStep::Call(pointer_call(platform::PointerEvent {
                tick: 0,
                kind,
                x: event.canvas_x,
                y: event.canvas_y,
            })));
    }

    pub(super) fn queue_release_all(&mut self) {
        self.pending_text.clear();
        if std::mem::take(&mut self.composing) {
            self.pending.push_back(text_edit_step(1, 0));
        }
        self.pending.extend(
            self.input
                .focus_lost()
                .into_iter()
                .map(|event| key_call(&event))
                .map(vm::DriverStep::Call),
        );
        if self.captured_pointer.take().is_some() {
            self.pending
                .push_back(vm::DriverStep::Call(pointer_call(platform::PointerEvent {
                    tick: 0,
                    kind: platform::PointerKind::PointerReleased,
                    x: self.pointer_position.0,
                    y: self.pointer_position.1,
                })));
        }
    }
}

pub(in crate::runtime) fn retain_input_release_steps(pending: &mut VecDeque<vm::DriverStep>) {
    pending.retain(|step| {
        matches!(
            step,
            vm::DriverStep::Call(call)
                if matches!(
                    call.name.as_str(),
                    "__hostKeyReleased" | "__hostPointerReleased"
                ) || call.name == "__hostTextInput" && call.arguments == [vm::Value::Int(0x10000)]
        )
    });
}

// Internal LCDUI editor operations keep preedit separate from the public text.
// The low word is a UTF-16 unit/offset; ordinary committed input stays <= 0xffff.
pub(super) fn text_edit_step(operation: u8, value: u16) -> vm::DriverStep {
    vm::DriverStep::Call(text_edit_call(operation, value))
}

pub(super) fn text_edit_call(operation: u8, value: u16) -> vm::InstanceCall {
    let value = (i32::from(operation) << 16) | i32::from(value);
    vm::InstanceCall {
        target: vm::CallTarget::Static {
            class: "javax/microedition/lcdui/Display".to_owned(),
        },
        name: "__hostTextInput".to_owned(),
        descriptor: "(I)V".to_owned(),
        arguments: vec![vm::Value::Int(value)],
    }
}
