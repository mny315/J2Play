//! Directional library focus and keyboard navigation.

use super::{
    FrontendApp, LIBRARY_FOCUS_ACTIONS, LIBRARY_FOCUS_FIRST, LIBRARY_FOCUS_ROWS, LibraryFocusGrid,
    egui,
};
use crate::Screen;

impl FrontendApp {
    pub(crate) fn move_library_focus(
        &self,
        ctx: &egui::Context,
        direction: egui::FocusDirection,
    ) -> bool {
        if !matches!(self.screen, Screen::Library) || self.overlay_active() {
            return false;
        }
        let focused = ctx.memory(egui::Memory::focused);
        if focused.is_none() && !self.library_search.indices.is_empty() {
            ctx.data_mut(|data| data.insert_temp(egui::Id::new(LIBRARY_FOCUS_FIRST), true));
            ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
            return true;
        }
        let grid =
            ctx.data(|data| data.get_temp::<LibraryFocusGrid>(egui::Id::new(LIBRARY_FOCUS_ROWS)));
        let first_row = grid.as_ref().is_some_and(|grid| {
            grid.focused_cell(focused).is_some_and(|(row, action)| {
                grid.items[row].0 < grid.layout.columns && (!grid.layout.tiles || action == 0)
            })
        });
        if self.move_library_folder_focus(ctx, direction, first_row) {
            return true;
        }
        if self.move_library_app_bar_focus(ctx, direction, first_row) {
            return true;
        }
        if focused == Some(crate::library_search::field_id())
            && direction == egui::FocusDirection::Down
            && !self.library_search.indices.is_empty()
        {
            ctx.data_mut(|data| data.insert_temp(egui::Id::new(LIBRARY_FOCUS_FIRST), true));
            ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
            return true;
        }
        let Some(grid) = grid else {
            return false;
        };
        let rows = &grid.items;
        let Some((row, column)) = grid.focused_cell(focused) else {
            return false;
        };
        // The preceding row can be above the viewport, geometrically closer to
        // the app bar. Follow library order so Up still reaches that row.
        let target = if grid.layout.tiles {
            let index = rows[row].0;
            let columns = grid.layout.columns;
            let next = match direction {
                egui::FocusDirection::Up if column > 0 => Some((index, 0)),
                egui::FocusDirection::Up if index < columns => return false,
                egui::FocusDirection::Up => Some((index - columns, 1)),
                egui::FocusDirection::Down if column == 0 => Some((index, 1)),
                egui::FocusDirection::Down => {
                    let next_row = (index / columns + 1) * columns;
                    (next_row < self.library_search.indices.len()).then(|| {
                        (
                            (index + columns).min(self.library_search.indices.len() - 1),
                            0,
                        )
                    })
                }
                egui::FocusDirection::Left => (index % columns > 0).then(|| (index - 1, column)),
                egui::FocusDirection::Right => (index % columns + 1 < columns
                    && index + 1 < self.library_search.indices.len())
                .then_some((index + 1, column)),
                _ => return false,
            };
            next.and_then(|(index, action)| {
                rows.iter()
                    .find(|(item, _)| *item == index)
                    .map(|(_, ids)| ids[action])
            })
        } else {
            match direction {
                egui::FocusDirection::Up if rows[row].0 == 0 => return false,
                egui::FocusDirection::Up => row.checked_sub(1).map(|row| rows[row].1[column]),
                egui::FocusDirection::Down => rows.get(row + 1).map(|(_, ids)| ids[column]),
                egui::FocusDirection::Left => Some(rows[row].1[column.saturating_sub(1)]),
                egui::FocusDirection::Right => Some(rows[row].1[(column + 1).min(1)]),
                _ => return false,
            }
        };
        ctx.memory_mut(|memory| {
            memory.move_focus(egui::FocusDirection::None);
            if let Some(target) = target {
                memory.request_focus(target);
            }
        });
        true
    }

    fn move_library_app_bar_focus(
        &self,
        ctx: &egui::Context,
        direction: egui::FocusDirection,
        first_row: bool,
    ) -> bool {
        let Some(actions) =
            ctx.data(|data| data.get_temp::<[egui::Id; 3]>(egui::Id::new(LIBRARY_FOCUS_ACTIONS)))
        else {
            return false;
        };
        let focused = ctx.memory(egui::Memory::focused);
        let target = if first_row && direction == egui::FocusDirection::Up {
            // App actions can be outside egui's directional search cone,
            // especially above the leftmost tile on a wide screen.
            Some(actions[0])
        } else if let Some(column) = actions.iter().position(|id| Some(*id) == focused) {
            match direction {
                egui::FocusDirection::Left => Some(actions[column.saturating_sub(1)]),
                egui::FocusDirection::Right => Some(actions[(column + 1).min(actions.len() - 1)]),
                egui::FocusDirection::Up => Some(actions[column]),
                egui::FocusDirection::Down if !self.library_search.indices.is_empty() => {
                    ctx.data_mut(|data| {
                        data.insert_temp(egui::Id::new(LIBRARY_FOCUS_FIRST), true);
                    });
                    None
                }
                _ => return false,
            }
        } else {
            return false;
        };
        ctx.memory_mut(|memory| {
            memory.move_focus(egui::FocusDirection::None);
            if let Some(target) = target {
                memory.request_focus(target);
            }
        });
        true
    }

    fn move_library_folder_focus(
        &self,
        ctx: &egui::Context,
        direction: egui::FocusDirection,
        first_row: bool,
    ) -> bool {
        let folders = ctx
            .data(|data| {
                data.get_temp::<Vec<egui::Id>>(egui::Id::new(crate::library_folders::FOLDER_FOCUS))
            })
            .unwrap_or_default();
        let Some(&first) = folders.first() else {
            return false;
        };
        let actions =
            ctx.data(|data| data.get_temp::<[egui::Id; 3]>(egui::Id::new(LIBRARY_FOCUS_ACTIONS)));
        let focused = ctx.memory(egui::Memory::focused);
        let target = if (first_row && direction == egui::FocusDirection::Up)
            || (direction == egui::FocusDirection::Down
                && actions.is_some_and(|actions| actions.iter().any(|id| Some(*id) == focused)))
        {
            Some(first)
        } else if let Some(index) = folders.iter().position(|id| Some(*id) == focused) {
            match direction {
                egui::FocusDirection::Up => actions.map(|actions| actions[0]),
                egui::FocusDirection::Left => Some(folders[index.saturating_sub(1)]),
                egui::FocusDirection::Right => Some(folders[(index + 1).min(folders.len() - 1)]),
                egui::FocusDirection::Down if !self.library_search.indices.is_empty() => {
                    ctx.data_mut(|data| data.insert_temp(egui::Id::new(LIBRARY_FOCUS_FIRST), true));
                    None
                }
                _ => return false,
            }
        } else {
            return false;
        };
        ctx.memory_mut(|memory| {
            memory.move_focus(egui::FocusDirection::None);
            if let Some(target) = target {
                memory.request_focus(target);
            }
        });
        true
    }

    pub(crate) fn process_library_keyboard(&mut self, ctx: &egui::Context) {
        if !matches!(self.screen, Screen::Library) || self.overlay_active() {
            return;
        }
        if ctx.text_edit_focused() {
            if ctx.memory(egui::Memory::focused) == Some(crate::library_search::field_id())
                && !self.library_search.indices.is_empty()
                && ctx.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                })
            {
                self.move_library_focus(ctx, egui::FocusDirection::Down);
            }
            return;
        }
        let keys: Vec<_> = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.is_none() => Some(*key),
                    _ => None,
                })
                .collect()
        });
        for key in keys {
            let direction = match key {
                egui::Key::ArrowUp => egui::FocusDirection::Up,
                egui::Key::ArrowDown => egui::FocusDirection::Down,
                egui::Key::ArrowLeft => egui::FocusDirection::Left,
                egui::Key::ArrowRight => egui::FocusDirection::Right,
                egui::Key::Tab | egui::Key::Enter | egui::Key::Space => egui::FocusDirection::None,
                _ => continue,
            };
            if self.focus_ring.navigate() && !self.entries.is_empty() {
                ctx.memory_mut(|memory| {
                    if let Some(id) = memory.focused() {
                        memory.surrender_focus(id);
                    }
                });
            }
            if self.move_library_focus(ctx, direction) {
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key));
                ctx.request_repaint();
            }
        }
    }
}
