//! Route the shell editor to egui's product text field, without guest input.

use super::{LibrarySearch, egui, field_id};
use crate::{HostAction, PlatformTextInputEvent as Input};

impl LibrarySearch {
    pub(crate) fn native_input(&mut self, ctx: &egui::Context, input: Input) {
        native_input(ctx, field_id(), &self.query, &mut self.composition, input);
    }
}

pub(crate) fn native_input(
    ctx: &egui::Context,
    id: egui::Id,
    text: &str,
    composition: &mut Option<String>,
    input: Input,
) {
    let event = match input {
        Input::Composition {
            text,
            selection_start,
            selection_end,
        } => {
            let range = count_units(text.chars(), selection_start)
                ..count_units(text.chars(), selection_end);
            *composition = Some(text.clone());
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text,
                active_range_chars: Some(range),
            })
        }
        Input::Commit(text) => {
            *composition = None;
            egui::Event::Ime(egui::ImeEvent::Commit(text))
        }
        Input::Cancel => {
            *composition = None;
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: String::new(),
                active_range_chars: None,
            })
        }
        Input::Selection { start, end } => {
            let Some(text) = composition.clone() else {
                return;
            };
            let range = count_units(text.chars(), start)..count_units(text.chars(), end);
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text,
                active_range_chars: Some(range),
            })
        }
        Input::DeleteSurrounding {
            before_chars,
            after_chars,
        } => egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
            before_chars: before_chars.min(256),
            after_chars: after_chars.min(256),
        }),
        Input::DeleteUtf16 { before, after } => {
            let chars: Vec<_> = text.chars().collect();
            let range = egui::TextEdit::load_state(ctx, id)
                .and_then(|state| state.cursor.char_range())
                .map_or(chars.len()..chars.len(), |range| {
                    let range = range.as_sorted_char_range();
                    range.start.0..range.end.0
                });
            let start = range.start.min(chars.len());
            let end = range.end.min(chars.len());
            egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
                before_chars: count_units(
                    chars[..start].iter().copied().rev(),
                    usize::from(before),
                ),
                after_chars: count_units(chars[end..].iter().copied(), usize::from(after)),
            })
        }
        Input::MoveCaret { offset } => {
            move_native_caret(ctx, id, text, offset);
            return;
        }
        Input::Key(action) => {
            let key = match action {
                HostAction::Left => egui::Key::ArrowLeft,
                HostAction::Right => egui::Key::ArrowRight,
                HostAction::Up => egui::Key::ArrowUp,
                HostAction::Down => egui::Key::ArrowDown,
                HostAction::Fire => egui::Key::Enter,
                _ => return,
            };
            push_key(ctx, key);
            return;
        }
    };
    ctx.input_mut(|input| input.events.push(event));
}

fn move_native_caret(ctx: &egui::Context, id: egui::Id, text: &str, offset: i16) {
    let chars: Vec<_> = text.chars().collect();
    let caret = egui::TextEdit::load_state(ctx, id)
        .and_then(|state| state.cursor.char_range())
        .map_or(chars.len(), |range| range.primary.index.0.min(chars.len()));
    let units = usize::from(offset.unsigned_abs());
    let steps = if offset < 0 {
        count_units(chars[..caret].iter().copied().rev(), units)
    } else {
        count_units(chars[caret..].iter().copied(), units)
    };
    let key = if offset < 0 {
        egui::Key::ArrowLeft
    } else {
        egui::Key::ArrowRight
    };
    for _ in 0..steps {
        push_key(ctx, key);
    }
}

fn push_key(ctx: &egui::Context, key: egui::Key) {
    ctx.input_mut(|input| {
        for pressed in [true, false] {
            input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
    });
}

fn count_units(chars: impl Iterator<Item = char>, units: usize) -> usize {
    let mut consumed = 0;
    chars
        .take_while(|ch| {
            if consumed >= units {
                return false;
            }
            consumed += ch.len_utf16();
            true
        })
        .count()
}
