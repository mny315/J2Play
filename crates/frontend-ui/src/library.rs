//! Virtualized library presentation, navigation, and lazy assets.

use super::library_view::{self, LibraryLayout};
use super::{
    DocumentKind, FrontendApp, MAX_LIBRARY_WARNINGS, MAX_UI_TECHNICAL_ERROR_BYTES, RichText,
    Screen, Vec2, bounded_ui_text, draw_library_icon, draw_library_text, egui, library_game_info,
    library_launch_rect, library_profile_presentation, material_app_bar_frame, material_card_frame,
    material_library_card_frame, material_primary_button, material_supporting_text,
};

mod navigation;

const LIBRARY_FOCUS_ROWS: &str = "library-focus-rows";
const LIBRARY_FOCUS_FIRST: &str = "library-focus-first";
const LIBRARY_FOCUS_ACTIONS: &str = "library-focus-actions";
type LibraryFocusRows = Vec<(usize, [egui::Id; 2])>;

#[derive(Clone)]
struct LibraryFocusGrid {
    layout: LibraryLayout,
    items: LibraryFocusRows,
}

impl LibraryFocusGrid {
    fn focused_cell(&self, focused: Option<egui::Id>) -> Option<(usize, usize)> {
        let focused = focused?;
        self.items.iter().enumerate().find_map(|(row, (_, ids))| {
            ids.iter()
                .position(|id| *id == focused)
                .map(|column| (row, column))
        })
    }
}

impl FrontendApp {
    pub(super) fn sort_entries(&mut self) {
        self.entries
            .sort_by_cached_key(|entry| (entry.title().to_lowercase(), entry.id().to_owned()));
        self.library_motion.invalidate_entries();
        self.library_search.invalidate();
    }

    pub(super) fn push_library_warning(&mut self, warning: &str) {
        if self.library_warnings.len() >= MAX_LIBRARY_WARNINGS {
            self.library_warnings.remove(0);
        }
        self.library_warnings
            .push(bounded_ui_text(warning, MAX_UI_TECHNICAL_ERROR_BYTES));
    }

    pub(super) fn draw_library(&mut self, ui: &mut egui::Ui) {
        self.library_selection.begin(ui);
        self.library_search
            .refresh(&self.entries, &self.library_folders.catalog);
        if ui.is_enabled()
            && self.focus_ring.controller_connected()
            && ui.memory(egui::Memory::focused).is_none()
        {
            self.move_library_focus(ui.ctx(), egui::FocusDirection::None);
        }
        self.draw_library_app_bar(ui);
        self.draw_library_folder_bar(ui);
        self.draw_library_search(ui);
        self.library_search
            .refresh(&self.entries, &self.library_folders.catalog);
        ui.add_space(8.0);
        ui.multiply_opacity(self.library_folders.content_fade.opacity(ui.ctx()));
        self.draw_library_warnings(ui);
        if self.entries.is_empty()
            && self.library_search.folder == super::library_folders::FolderFilter::All
        {
            self.library_motion = super::library_motion::LibraryMotion::default();
            ui.ctx().data_mut(|data| {
                data.remove::<LibraryFocusGrid>(egui::Id::new(LIBRARY_FOCUS_ROWS));
            });
            self.draw_empty_library(ui);
            return;
        }
        if self.library_search.indices.is_empty() {
            ui.ctx().data_mut(|data| {
                data.remove::<LibraryFocusGrid>(egui::Id::new(LIBRARY_FOCUS_ROWS));
            });
            let tr = crate::i18n::Translator::from_context(ui.ctx());
            ui.label(material_supporting_text(
                &self.material_theme,
                tr.text(
                    if self.library_search.query.trim().is_empty()
                        && self.library_search.folder != super::library_folders::FolderFilter::All
                    {
                        "This folder is empty"
                    } else {
                        "No games found"
                    },
                ),
            ));
            return;
        }
        self.draw_library_entries(ui);
        self.flush_library_selection_haptic(ui);
    }

    // Library length and egui viewport coordinates are bounded; these casts only
    // select the visible rows plus one neighbor for directional focus.
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn draw_library_entries(&mut self, ui: &mut egui::Ui) {
        let focus_first = ui.is_enabled()
            && ui.ctx().data_mut(|data| {
                data.remove_temp::<bool>(egui::Id::new(LIBRARY_FOCUS_FIRST))
                    .unwrap_or_default()
            });
        let layout = LibraryLayout::new(
            self.app_settings.library_view,
            (ui.available_width() - ui.spacing().scroll.content_margin.sum().x).max(0.0),
            ui.spacing().item_spacing.x,
        );
        let focused = ui.memory(egui::Memory::focused);
        self.library_motion.begin(
            self.entries
                .iter()
                .map(|entry| library_view::launch_id(entry.id())),
            layout,
        );
        let (reflow_focus, mut focus_rows) = ui.ctx().data_mut(|data| {
            let previous =
                data.get_temp_mut_or_insert_with(egui::Id::new(LIBRARY_FOCUS_ROWS), || {
                    LibraryFocusGrid {
                        layout,
                        items: LibraryFocusRows::new(),
                    }
                });
            let reflow_focus = if previous.layout == layout {
                None
            } else {
                previous
                    .focused_cell(focused)
                    .map(|(row, _)| previous.items[row].0)
            };
            (reflow_focus, std::mem::take(&mut previous.items))
        });
        let reveal = self
            .library_added
            .as_mut()
            .filter(|added| added.reveal && ui.is_enabled())
            .and_then(|added| {
                added.reveal = false;
                self.library_search
                    .indices
                    .iter()
                    .position(|&index| added.ids.contains(self.entries[index].id()))
            });
        let mut scroll = super::scrolling::vertical().id_salt("game-library");
        let reset_scroll = std::mem::take(&mut self.library_search.reset_scroll);
        if let Some(index) = reveal {
            scroll = scroll.vertical_scroll_offset(
                (index / layout.columns) as f32
                    * (layout.item_size.y + ui.spacing().item_spacing.y),
            );
        } else if focus_first || reset_scroll {
            scroll = scroll.vertical_scroll_offset(0.0);
        } else if let Some(index) = reflow_focus {
            scroll = scroll.vertical_scroll_offset(
                (index / layout.columns) as f32
                    * (layout.item_size.y + ui.spacing().item_spacing.y),
            );
        }
        focus_rows.clear();
        scroll.show_viewport(ui, |ui, viewport| {
            let stride = layout.item_size.y + ui.spacing().item_spacing.y;
            let row_count = self.library_search.indices.len().div_ceil(layout.columns);
            ui.set_height((row_count as f32 * stride - ui.spacing().item_spacing.y).max(0.0));
            let visible = layout.visible_entries(
                viewport,
                ui.spacing().item_spacing.y,
                self.library_search.indices.len(),
            );
            let first = visible.start / layout.columns;
            let origin = ui.max_rect().min;
            self.prepare_library_icons(ui.ctx(), visible.len(), layout.tiles);
            for index in visible {
                let entry_index = self.library_search.indices[index];
                let (rect, opacity) = self.animated_library_rect(ui, index, first, layout);
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .id_salt(("library-item", self.entries[entry_index].id()))
                        .max_rect(rect.translate(origin.to_vec2())),
                    |ui| {
                        ui.multiply_opacity(opacity);
                        let ids = if layout.tiles {
                            self.draw_library_tile(ui, entry_index, layout.item_size.x)
                        } else {
                            self.draw_library_row(ui, entry_index, layout.item_size.x)
                        };
                        if reveal == Some(index) || (reveal.is_none() && focus_first && index == 0)
                        {
                            ui.memory_mut(|memory| memory.request_focus(ids[0]));
                            ui.ctx().request_repaint();
                        } else if reflow_focus == Some(index)
                            && let Some(response) =
                                focused.and_then(|id| ui.ctx().read_response(id))
                        {
                            // Stable action IDs do not retrigger the focus ring's
                            // normal scroll request when only geometry changes.
                            ui.scroll_to_rect(
                                response.rect.expand(f32::from(super::focus_ring::OUTSET)),
                                None,
                            );
                        }
                        focus_rows.push((index, ids));
                    },
                );
            }
        });
        self.library_motion.finish();
        ui.ctx().data_mut(|data| {
            let grid = data.get_temp_mut_or_insert_with(egui::Id::new(LIBRARY_FOCUS_ROWS), || {
                LibraryFocusGrid {
                    layout,
                    items: LibraryFocusRows::new(),
                }
            });
            grid.layout = layout;
            grid.items = focus_rows;
        });
    }

    // Library indices are bounded; all resulting coordinates are host-only UI points.
    #[allow(clippy::cast_precision_loss)]
    fn animated_library_rect(
        &mut self,
        ui: &egui::Ui,
        index: usize,
        first_row: usize,
        layout: LibraryLayout,
    ) -> (egui::Rect, f32) {
        let stride = layout.item_size + ui.spacing().item_spacing;
        let target = egui::Rect::from_min_size(
            egui::pos2(
                (index % layout.columns) as f32 * stride.x,
                (index / layout.columns) as f32 * stride.y,
            ),
            layout.item_size,
        );
        let (rect, opacity, animating) = self.library_motion.item(
            library_view::launch_id(self.entries[self.library_search.indices[index]].id()),
            target,
            index - first_row * layout.columns,
            ui.input(|input| input.time),
        );
        if animating {
            ui.ctx().request_repaint();
        }
        (rect, opacity)
    }

    pub(super) fn draw_library_app_bar(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        material_app_bar_frame(&self.material_theme).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let compact = ui.available_width() < 460.0;
            let import_label = tr.text(if compact { "+ Import" } else { "+ Import game" });
            let actions_width = library_view::settings_button_width(ui, &import_label)
                + 96.0
                + ui.spacing().item_spacing.x * 2.0;
            let brand_width = 36.0
                + ui.spacing().item_spacing.x * 2.0
                + ui.painter()
                    .layout_no_wrap(
                        "J2Play".to_owned(),
                        egui::FontId::proportional(20.0),
                        self.material_theme.on_surface,
                    )
                    .size()
                    .x;
            let stacked = ui.available_width() < actions_width + brand_width;
            ui.horizontal(|ui| {
                let (mark, _) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                ui.painter().circle_filled(
                    mark.center(),
                    18.0,
                    self.material_theme.primary_container,
                );
                ui.painter().text(
                    mark.center(),
                    egui::Align2::CENTER_CENTER,
                    "J2",
                    egui::FontId::proportional(17.0),
                    self.material_theme.on_surface,
                );
                ui.vertical(|ui| {
                    ui.add_space(1.0);
                    ui.label(RichText::new("J2Play").size(20.0).strong());
                    if !compact {
                        ui.label(material_supporting_text(
                            &self.material_theme,
                            tr.text("Java ME library"),
                        ));
                    }
                });
                if !stacked {
                    self.draw_library_app_actions(ui, compact);
                }
            });
            if stacked {
                ui.add_space(8.0);
                self.draw_library_app_actions(ui, compact);
            }
        });
    }

    fn draw_library_app_actions(&mut self, ui: &mut egui::Ui, compact: bool) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let mut actions = [egui::Id::NULL; 3];
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = tr.text(if compact { "+ Import" } else { "+ Import game" });
            let label = if ui.available_width()
                < library_view::settings_button_width(ui, &label)
                    + 96.0
                    + ui.spacing().item_spacing.x * 2.0
            {
                "+".to_owned()
            } else {
                label
            };
            let import = ui
                .add_sized(
                    [48.0, 48.0],
                    material_primary_button(&self.material_theme, label),
                )
                .on_hover_text(tr.text("Import a Java ME JAR from this device"));
            import.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    tr.text("Import game"),
                )
            });
            actions[2] = import.id;
            if import.clicked() {
                self.library_selection.clear();
                self.request_picker(DocumentKind::Jar);
            }
            let search = ui
                .add_sized(
                    [48.0, 48.0],
                    super::material_tonal_button(&self.material_theme, ""),
                )
                .on_hover_text(tr.text("Search games"));
            actions[1] = search.id;
            search.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    tr.text("Search games"),
                )
            });
            let center = search.rect.center() - egui::vec2(2.0, 2.0);
            let stroke = egui::Stroke::new(2.0, self.material_theme.on_surface);
            ui.painter().circle_stroke(center, 7.0, stroke);
            ui.painter().line_segment(
                [
                    center + egui::vec2(5.0, 5.0),
                    center + egui::vec2(12.0, 12.0),
                ],
                stroke,
            );
            if search.clicked() {
                if self.library_search.open {
                    self.library_search.close();
                } else {
                    self.library_search.open = true;
                    self.library_search.focus = true;
                }
            }
            let response = ui
                .add_sized(
                    [48.0, 48.0],
                    super::material_tonal_button(&self.material_theme, "⚙"),
                )
                .on_hover_text(tr.text("App settings"));
            actions[0] = response.id;
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    tr.text("App settings"),
                )
            });
            if response.clicked() {
                self.library_selection.clear();
                self.screen = Screen::AppSettings(self.app_settings.clone().into());
            }
        });
        ui.ctx().data_mut(|data| {
            data.insert_temp(egui::Id::new(LIBRARY_FOCUS_ACTIONS), actions);
        });
    }

    pub(super) fn draw_library_warnings(&self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        if !self.library_warnings.is_empty() {
            material_card_frame(&self.material_theme)
                .fill(self.material_theme.warning_container)
                .stroke(egui::Stroke::new(1.0, self.material_theme.warning))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(tr.text("Library warnings"))
                            .size(18.0)
                            .strong()
                            .color(self.material_theme.on_warning_container),
                    );
                    ui.label(material_supporting_text(
                        &self.material_theme,
                        tr.format(
                            "Warnings: {count}. Valid games remain available.",
                            &[("count", &self.library_warnings.len().to_string())],
                        ),
                    ));
                    ui.collapsing(tr.text("Technical details"), |ui| {
                        for warning in &self.library_warnings {
                            ui.monospace(warning);
                        }
                    });
                });
            ui.add_space(8.0);
        }
    }

    pub(super) fn draw_empty_library(&mut self, ui: &mut egui::Ui) {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        ui.vertical_centered(|ui| {
            ui.add_space(36.0);
            material_card_frame(&self.material_theme).show(ui, |ui| {
                ui.set_max_width(420.0);
                ui.vertical_centered(|ui| {
                    let (mark, _) = ui.allocate_exact_size(Vec2::splat(76.0), egui::Sense::hover());
                    ui.painter().circle_filled(
                        mark.center(),
                        38.0,
                        self.material_theme.primary_container,
                    );
                    ui.painter().text(
                        mark.center(),
                        egui::Align2::CENTER_CENTER,
                        "J2",
                        egui::FontId::proportional(25.0),
                        self.material_theme.on_surface,
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(tr.text("Your game library is empty"))
                            .size(22.0)
                            .strong(),
                    );
                    ui.label(material_supporting_text(
                        &self.material_theme,
                        tr.text("Import an original Java ME JAR to get started."),
                    ));
                    ui.add_space(8.0);
                    if ui
                        .add(material_primary_button(
                            &self.material_theme,
                            tr.text("Import game"),
                        ))
                        .clicked()
                    {
                        self.request_picker(DocumentKind::Jar);
                    }
                });
            });
        });
    }

    pub(super) fn draw_library_row(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        row_width: f32,
    ) -> [egui::Id; 2] {
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let icon = self.load_library_assets(ui.ctx(), index);
        let strength = self
            .library_added_strength(ui, index)
            .max(self.library_selection_strength(ui, self.entries[index].id()));
        let id = self.entries[index].id();
        let title = self.entries[index].title();
        let game_info = library_game_info(&self.entries[index]);
        let profile = library_profile_presentation(
            &self.entries[index],
            &self.profile_options,
            &self.catalog_fingerprint,
            tr,
        );
        let description = profile.launch_description(title, &game_info, tr);
        let mut focus_ids = [egui::Id::NULL; 2];
        let mut launch_response = None;
        let mut settings_requested = false;
        let selecting = self.library_selection.active;
        let selected = self.library_selection.ids.contains(id);
        ui.push_id(id, |ui| {
            super::library_search::highlight_frame(
                material_library_card_frame(&self.material_theme),
                &self.material_theme,
                strength,
            )
            .show(ui, |ui| {
                ui.set_min_width((row_width - 26.0).max(0.0));
                ui.horizontal(|ui| {
                    let icon_rect = draw_library_icon(ui, icon.as_ref(), &self.material_theme);
                    let mut text_rect = None;
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let settings = library_view::row_action(
                            ui,
                            id,
                            title,
                            &self.material_theme,
                            selecting.then_some(selected),
                        );
                        settings_requested = settings.clicked();
                        focus_ids[1] = settings.id;

                        let text_width = ui.available_width().max(0.0);
                        text_rect = draw_library_text(
                            ui,
                            title,
                            &game_info,
                            &profile,
                            text_width,
                            &self.material_theme,
                        );
                    });
                    let launch_rect = library_launch_rect(icon_rect, text_rect);
                    let launch = ui
                        .interact(
                            launch_rect,
                            library_view::launch_id(id),
                            egui::Sense::click(),
                        )
                        .on_hover_text(if selecting { title } else { &description });
                    let enabled = ui.is_enabled();
                    launch.widget_info(|| {
                        if selecting {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Checkbox,
                                enabled,
                                selected,
                                title,
                            )
                        } else {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                enabled,
                                &description,
                            )
                        }
                    });
                    library_view::track_launch(ui, &launch, &self.material_theme);
                    focus_ids[0] = launch.id;
                    launch_response = Some(launch);
                });
            });
        });
        let open = launch_response
            .is_some_and(|launch| self.library_selection.interact(&launch, id, true));
        if open || settings_requested {
            let id = id.to_owned();
            if open {
                self.open_game(&id);
            }
            if settings_requested {
                if selecting {
                    self.library_selection.toggle(&id);
                } else {
                    self.open_settings(&id, false);
                }
            }
        }
        focus_ids
    }
}
