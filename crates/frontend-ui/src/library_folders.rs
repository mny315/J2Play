//! Shared library folder selection and explicit organization actions.

use super::{FrontendApp, egui};
use frontend_core::LibraryFolders;

pub(super) const FOLDER_FOCUS: &str = "library-folder-focus";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum FolderFilter {
    #[default]
    All,
    Unfiled,
    Folder(u64),
}

impl FolderFilter {
    pub fn matches(self, folder: Option<u64>) -> bool {
        match self {
            Self::All => true,
            Self::Unfiled => folder.is_none(),
            Self::Folder(id) => folder == Some(id),
        }
    }
}

pub(super) enum FolderDialog {
    Edit {
        id: Option<u64>,
        name: String,
    },
    Delete {
        id: u64,
        name: String,
    },
    Move {
        entry_ids: Vec<String>,
        folder: Option<u64>,
    },
}

pub(super) struct LibraryFoldersUi {
    pub catalog: LibraryFolders,
    pub available: bool,
    pub dialog: Option<FolderDialog>,
    pub editing: bool,
    pub composition: Option<String>,
    focus_name: bool,
    dialog_fade: FolderFade,
    pub content_fade: FolderFade,
}

/// A finite fade begins when painted and never delays folder actions or moves hit targets.
#[derive(Default)]
pub(super) struct FolderFade {
    pending: bool,
    started: Option<f64>,
}

impl FolderFade {
    pub fn request(&mut self) {
        self.pending = true;
        self.started = None;
    }

    pub fn settle(&mut self) {
        self.pending = false;
        self.started = None;
    }

    pub fn opacity(&mut self, ctx: &egui::Context) -> f32 {
        let now = ctx.input(|input| input.time);
        if std::mem::take(&mut self.pending) {
            self.started = Some(now);
        }
        let Some(started) = self.started else {
            return 1.0;
        };
        #[allow(clippy::cast_possible_truncation)]
        let progress = ((now - started) / f64::from(super::ui_motion::UI_MOTION_SECONDS))
            .clamp(0.0, 1.0) as f32;
        if progress >= 1.0 {
            self.settle();
        } else {
            ctx.request_repaint();
        }
        0.05 + 0.95 * progress * progress * (3.0 - 2.0 * progress)
    }
}

impl LibraryFoldersUi {
    pub fn new(catalog: Option<LibraryFolders>) -> Self {
        Self {
            available: catalog.is_some(),
            catalog: catalog.unwrap_or_default(),
            dialog: None,
            editing: false,
            composition: None,
            focus_name: false,
            dialog_fade: FolderFade::default(),
            content_fade: FolderFade::default(),
        }
    }

    pub fn finish_editing(&mut self) {
        self.editing = false;
        self.composition = None;
        self.focus_name = false;
        self.dialog_fade.settle();
    }

    pub fn edit(&mut self, id: Option<u64>, name: String) {
        self.finish_editing();
        self.dialog = Some(FolderDialog::Edit { id, name });
        self.focus_name = true;
        self.dialog_fade.request();
    }

    pub fn native_input(&mut self, ctx: &egui::Context, input: super::PlatformTextInputEvent) {
        if let Some(FolderDialog::Edit { name, .. }) = &self.dialog {
            super::library_search::ime::native_input(
                ctx,
                name_field_id(),
                name,
                &mut self.composition,
                input,
            );
        }
    }
}

pub(super) fn name_field_id() -> egui::Id {
    egui::Id::new("library-folder-name")
}

impl FrontendApp {
    pub(super) fn select_library_folder(&mut self, folder: FolderFilter) {
        self.library_selection.clear();
        self.library_search.folder = folder;
        self.library_search.invalidate();
        self.library_search.reset_scroll = true;
        self.library_motion.settle();
        self.library_folders.content_fade.request();
    }

    pub(super) fn draw_library_folder_bar(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        ui.add_space(8.0);
        if self.library_selection.active {
            self.draw_library_selection_bar(ui);
            return;
        }
        let mut selected = self.library_search.folder;
        let mut actions = Vec::new();
        ui.horizontal(|ui| {
            super::apply_settings_style(ui);
            let buttons = if matches!(selected, FolderFilter::Folder(_)) {
                3.0
            } else {
                2.0
            };
            let width =
                (ui.available_width() - buttons * (48.0 + ui.spacing().item_spacing.x)).max(48.0);
            let combo = folder_filter(
                ui,
                &self.library_folders.catalog,
                self.library_search.has_filed_games,
                &mut selected,
                width,
            );
            actions.push(combo.id);
            combo.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::ComboBox,
                    ui.is_enabled(),
                    tr.text("Folder"),
                )
            });
            let select = super::library_selection::selection_button(
                ui,
                &self.material_theme,
                false,
                &tr.text("Select games"),
                !self.entries.is_empty(),
            );
            if select.enabled() {
                actions.push(select.id);
            }
            if select.clicked() {
                self.library_selection.active = true;
            }
            let add = ui
                .add_enabled(
                    self.library_folders.available
                        && self.library_folders.catalog.folders().len()
                            < frontend_core::MAX_LIBRARY_FOLDERS,
                    super::material_tonal_button(&self.material_theme, "+")
                        .min_size(egui::vec2(48.0, 48.0)),
                )
                .on_hover_text(tr.text("New folder"));
            add.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    add.enabled(),
                    tr.text("New folder"),
                )
            });
            if add.enabled() {
                actions.push(add.id);
            }
            if add.clicked() {
                self.library_folders.edit(None, String::new());
            }
            if let FolderFilter::Folder(id) = selected {
                actions.push(self.draw_folder_actions(ui, id));
            }
        });
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new(FOLDER_FOCUS), actions));
        if selected != self.library_search.folder {
            self.select_library_folder(selected);
        }
    }

    fn draw_folder_actions(&mut self, ui: &mut egui::Ui, id: u64) -> egui::Id {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let menu = ui.menu_button("…", |ui| {
            if ui.button(tr.text("Rename folder")).clicked() {
                if let Some(folder) = self.library_folders.catalog.get(id) {
                    self.library_folders
                        .edit(Some(id), folder.name().to_owned());
                }
                ui.close();
            }
            if ui.button(tr.text("Delete folder")).clicked() {
                if let Some(folder) = self.library_folders.catalog.get(id) {
                    self.library_folders.dialog = Some(FolderDialog::Delete {
                        id,
                        name: folder.name().to_owned(),
                    });
                }
                ui.close();
            }
        });
        menu.response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                tr.text("Folder actions"),
            )
        });
        menu.response.id
    }

    pub(super) fn draw_folder_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.library_folders.dialog else {
            return;
        };
        let tr = crate::i18n::Translator::from_context(ctx);
        let title = match dialog {
            FolderDialog::Edit { id: None, .. } => "New folder",
            FolderDialog::Edit { .. } => "Rename folder",
            FolderDialog::Delete { .. } => "Delete folder?",
            FolderDialog::Move { .. } => "Move to folder",
        };
        let mut confirm = false;
        let mut cancel = false;
        let animate_name_dialog = matches!(dialog, FolderDialog::Edit { .. });
        let mut opacity = 1.0;
        let window =
            super::dialogs::dialog_window(ctx, tr.text(title), self.safe_content_rect, 440.0)
                .id(egui::Id::new(("library-folder-dialog", title)))
                .fade_in(!animate_name_dialog)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .constrain_to(self.safe_content_rect)
                .show(ctx, |ui| {
                    if animate_name_dialog && ui.is_visible() && !ui.is_sizing_pass() {
                        opacity = self.library_folders.dialog_fade.opacity(ctx);
                    }
                    super::apply_settings_style(ui);
                    let valid = draw_dialog_contents(
                        ui,
                        dialog,
                        &self.library_folders.catalog,
                        &mut self.library_folders.focus_name,
                        &self.entries,
                    );
                    ui.add_space(12.0);
                    ui.horizontal_wrapped(|ui| {
                        cancel = ui
                            .add(super::material_text_button(
                                &self.material_theme,
                                tr.text("Cancel"),
                            ))
                            .clicked();
                        let label = match dialog {
                            FolderDialog::Delete { .. } => "Delete folder",
                            FolderDialog::Move { .. } => "Move",
                            FolderDialog::Edit { .. } => "Save",
                        };
                        let button = if matches!(dialog, FolderDialog::Delete { .. }) {
                            super::material_danger_button(&self.material_theme, tr.text(label))
                        } else {
                            super::material_primary_button(&self.material_theme, tr.text(label))
                        };
                        confirm = ui.add_enabled(valid, button).clicked();
                    });
                });
        if let Some(window) = window {
            fade_dialog_layer(ctx, window.response.layer_id, opacity);
        }
        if cancel {
            self.library_folders.dialog = None;
            self.library_folders.finish_editing();
        } else if confirm {
            self.confirm_folder_dialog();
        }
    }

    pub(super) fn confirm_folder_dialog(&mut self) {
        let Some(mut dialog) = self.library_folders.dialog.take() else {
            return;
        };
        self.library_folders.finish_editing();
        let result = match &mut dialog {
            FolderDialog::Edit { id: None, name } => {
                self.repository.create_folder(name).map(|(catalog, id)| {
                    self.library_folders.catalog = catalog;
                    self.select_library_folder(FolderFilter::Folder(id));
                })
            }
            FolderDialog::Edit { id: Some(id), name } => self
                .repository
                .rename_folder(*id, name)
                .map(|catalog| self.library_folders.catalog = catalog),
            FolderDialog::Delete { id, .. } => self.repository.delete_folder(*id).map(|catalog| {
                self.library_folders.catalog = catalog;
                self.select_library_folder(FolderFilter::Unfiled);
            }),
            FolderDialog::Move { entry_ids, folder } => {
                let result = entry_ids.iter().try_for_each(|id| {
                    let entry = self
                        .entries
                        .iter_mut()
                        .find(|entry| entry.id() == id)
                        .ok_or_else(|| {
                            super::EmuError::new(
                                diagnostics::Category::Platform,
                                "library-entry-missing",
                                "selected library entry no longer exists",
                            )
                        })?;
                    self.repository.move_to_folder(entry, *folder)?;
                    self.library_selection.ids.remove(id);
                    Ok(())
                });
                if result.is_err() {
                    entry_ids.retain(|id| self.library_selection.ids.contains(id));
                }
                result
            }
        };
        self.library_search.invalidate();
        if let Err(error) = result {
            self.library_folders.dialog = Some(dialog);
            self.show_error("Could not update folders", &error);
        } else if let FolderDialog::Move { entry_ids, folder } = dialog {
            self.library_search.close();
            self.select_library_folder(folder.map_or(FolderFilter::Unfiled, FolderFilter::Folder));
            self.library_added = Some(super::library_search::AddedGames::new(entry_ids));
        }
    }

    pub(super) fn open_move_to_folder(&mut self) {
        let entry_ids: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| self.library_selection.ids.contains(entry.id()))
            .map(|entry| entry.id().to_owned())
            .collect();
        if !entry_ids.is_empty() {
            self.library_folders.finish_editing();
            self.library_folders.dialog = Some(FolderDialog::Move {
                entry_ids,
                folder: match self.library_search.folder {
                    FolderFilter::Folder(id) => Some(id),
                    _ => None,
                },
            });
        }
    }

    pub(super) fn settle_folder_motion(&mut self) {
        self.library_folders.content_fade.settle();
        self.library_folders.dialog_fade.settle();
    }
}

fn fade_dialog_layer(ctx: &egui::Context, layer_id: egui::LayerId, opacity: f32) {
    if opacity >= 1.0 {
        return;
    }
    // The window paints its frame and title outside the content closure. Fade
    // their layer together, leaving layout, focus and hit targets untouched.
    ctx.graphics_mut(|graphics| {
        if let Some(layer) = graphics.get_mut(layer_id) {
            for index in 0..layer.next_idx().0 {
                layer.mutate_shape(egui::layers::ShapeIdx(index), |shape| {
                    egui::epaint::shape_transform::adjust_colors(&mut shape.shape, move |color| {
                        if *color != egui::Color32::PLACEHOLDER {
                            *color = color.gamma_multiply(opacity);
                        }
                    });
                });
            }
        }
    });
}

fn draw_folder_choice(ui: &mut egui::Ui, folders: &LibraryFolders, selected: &mut Option<u64>) {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let label = selected.and_then(|id| folders.get(id)).map_or_else(
        || tr.text("Without folder"),
        |folder| folder.name().to_owned(),
    );
    let combo_id = ui.make_persistent_id(egui::IdSalt::new("move-folder"));
    egui::ComboBox::from_id_salt("move-folder")
        .selected_text(label)
        .width(ui.available_width())
        .height(240.0)
        .truncate()
        .show_ui(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            let mut menu = super::focus_navigation::ComboMenu::begin(ui, combo_id);
            menu.selectable_value(ui, selected, None, &tr.text("Without folder"));
            for folder in folders.folders() {
                menu.selectable_value(ui, selected, Some(folder.id()), folder.name());
            }
            menu.finish(ui);
        });
}

fn draw_dialog_contents(
    ui: &mut egui::Ui,
    dialog: &mut FolderDialog,
    catalog: &LibraryFolders,
    focus_name: &mut bool,
    entries: &[frontend_core::LibraryEntry],
) -> bool {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let mut valid = true;
    match dialog {
        FolderDialog::Edit { id, name } => {
            ui.label(tr.text("Folder name"));
            let response = ui.add_sized(
                [ui.available_width(), 48.0],
                egui::TextEdit::singleline(name)
                    .id(name_field_id())
                    .char_limit(frontend_core::MAX_FOLDER_NAME_CHARS)
                    .vertical_align(egui::Align::Center),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::TextEdit,
                    ui.is_enabled(),
                    tr.text("Folder name"),
                )
            });
            super::focus_navigation::register(ui, &response);
            if ui.is_visible() && !ui.is_sizing_pass() && std::mem::take(focus_name) {
                response.request_focus();
            }
            if let Err(error) = catalog.validate_name(name, *id) {
                valid = false;
                if !name.is_empty() {
                    ui.label(tr.text(if error.code() == "library-folder-duplicate" {
                        "A folder with this name already exists"
                    } else {
                        "Enter a valid folder name"
                    }));
                }
            }
        }
        FolderDialog::Delete { name, .. } => {
            ui.label(name.as_str());
            ui.label(
                tr.text("Games will be moved to Without folder. Games and saves will be kept."),
            );
        }
        FolderDialog::Move { entry_ids, folder } => {
            if entry_ids.len() == 1 {
                if let Some(entry) = entries.iter().find(|entry| entry.id() == entry_ids[0]) {
                    ui.label(entry.title());
                }
            } else {
                ui.label(tr.format(
                    "Selected: {count}",
                    &[("count", &entry_ids.len().to_string())],
                ));
            }
            draw_folder_choice(ui, catalog, folder);
        }
    }
    valid
}

fn folder_filter(
    ui: &mut egui::Ui,
    folders: &LibraryFolders,
    show_unfiled: bool,
    selected: &mut FolderFilter,
    width: f32,
) -> egui::Response {
    let tr = crate::i18n::Translator::from_context(ui.ctx());
    let label = match *selected {
        FolderFilter::All => tr.text("All games"),
        FolderFilter::Unfiled => tr.text("Without folder"),
        FolderFilter::Folder(id) => folders.get(id).map_or_else(
            || tr.text("Without folder"),
            |folder| folder.name().to_owned(),
        ),
    };
    ui.allocate_ui_with_layout(
        egui::vec2(width, 48.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            let combo_id = ui.make_persistent_id(egui::IdSalt::new("library-folder"));
            egui::ComboBox::from_id_salt("library-folder")
                .selected_text(label)
                .width(width)
                .height(300.0)
                .truncate()
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                    let mut menu = super::focus_navigation::ComboMenu::begin(ui, combo_id);
                    menu.selectable_value(ui, selected, FolderFilter::All, &tr.text("All games"));
                    if show_unfiled {
                        menu.selectable_value(
                            ui,
                            selected,
                            FolderFilter::Unfiled,
                            &tr.text("Without folder"),
                        );
                    }
                    for folder in folders.folders() {
                        menu.selectable_value(
                            ui,
                            selected,
                            FolderFilter::Folder(folder.id()),
                            folder.name(),
                        );
                    }
                    menu.finish(ui);
                })
        },
    )
    .inner
    .response
}
