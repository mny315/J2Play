//! Transient library filtering and feedback for imported or moved games.

use super::{FrontendApp, LibraryEntry, egui};

pub(super) mod ime;

#[derive(Default)]
enum SearchAppearance {
    #[default]
    Hidden,
    Fading(f64),
    Visible,
}

#[derive(Default)]
// Visibility, pending focus/scroll, native editing and cache validity are independent.
#[allow(clippy::struct_excessive_bools)]
pub(super) struct LibrarySearch {
    pub folder: super::library_folders::FolderFilter,
    pub has_filed_games: bool,
    pub open: bool,
    pub query: String,
    pub focus: bool,
    pub reset_scroll: bool,
    pub indices: Vec<usize>,
    pub editing: bool,
    pub composition: Option<String>,
    current: bool,
    appearance: SearchAppearance,
}

impl LibrarySearch {
    pub fn invalidate(&mut self) {
        self.current = false;
    }

    pub fn refresh(&mut self, entries: &[LibraryEntry], folders: &frontend_core::LibraryFolders) {
        if self.current {
            return;
        }
        let query = self.query.trim().to_lowercase();
        self.has_filed_games = false;
        self.indices.clear();
        for (index, entry) in entries.iter().enumerate() {
            let folder = folders.folder_for(entry);
            self.has_filed_games |= folder.is_some();
            if self.folder.matches(folder)
                && (query.is_empty() || entry.title().to_lowercase().contains(&query))
            {
                self.indices.push(index);
            }
        }
        if !self.has_filed_games && self.folder == super::library_folders::FolderFilter::Unfiled {
            self.folder = super::library_folders::FolderFilter::All;
        }
        self.current = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.focus = false;
        self.editing = false;
        self.composition = None;
        self.appearance = SearchAppearance::Hidden;
        self.reset_scroll = true;
        self.invalidate();
    }

    pub fn finish_animation(&mut self) {
        if self.open {
            self.appearance = SearchAppearance::Visible;
        }
    }

    fn opacity(&mut self, now: f64) -> f32 {
        if matches!(self.appearance, SearchAppearance::Hidden) {
            self.appearance = SearchAppearance::Fading(now);
        }
        let SearchAppearance::Fading(started) = self.appearance else {
            return 1.0;
        };
        #[allow(clippy::cast_possible_truncation)]
        let t = ((now - started) / f64::from(super::ui_motion::UI_MOTION_SECONDS)).clamp(0.0, 1.0)
            as f32;
        if t >= 1.0 {
            self.appearance = SearchAppearance::Visible;
        }
        0.05 + 0.95 * t * t * (3.0 - 2.0 * t)
    }
}

pub(super) struct AddedGames {
    pub ids: std::collections::BTreeSet<String>,
    pub reveal: bool,
    started: Option<f64>,
}

impl AddedGames {
    pub fn new(ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            ids: ids.into_iter().collect(),
            reveal: true,
            started: None,
        }
    }

    pub fn strength(&mut self, now: f64) -> f32 {
        let start = *self.started.get_or_insert(now);
        #[allow(clippy::cast_possible_truncation)]
        let progress = ((now - start) / 10.0).clamp(0.0, 1.0) as f32;
        1.0 - progress * progress * (3.0 - 2.0 * progress)
    }
}

pub(super) fn field_id() -> egui::Id {
    egui::Id::new("library-search-field")
}

impl FrontendApp {
    pub(super) fn draw_library_search(&mut self, ui: &mut egui::Ui) {
        if !self.library_search.open {
            return;
        }
        let tr = crate::i18n::Translator::from_context(ui.ctx());
        let opacity = self.library_search.opacity(ui.input(|input| input.time));
        if opacity < 1.0 {
            ui.ctx().request_repaint();
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.multiply_opacity(opacity);
            let width = (ui.available_width() - 48.0 - ui.spacing().item_spacing.x).max(0.0);
            let response = ui.add_sized(
                [width, 48.0],
                egui::TextEdit::singleline(&mut self.library_search.query)
                    .id(field_id())
                    .hint_text(tr.text("Search games"))
                    .char_limit(256)
                    .vertical_align(egui::Align::Center),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::TextEdit,
                    ui.is_enabled(),
                    tr.text("Search games"),
                )
            });
            super::focus_navigation::register(ui, &response);
            if ui.is_enabled() && std::mem::take(&mut self.library_search.focus) {
                response.request_focus();
            }
            if response.changed() {
                self.library_search.invalidate();
                self.library_search.reset_scroll = true;
                self.library_motion.settle();
            }
            let close = ui
                .add_sized(
                    [48.0, 48.0],
                    super::material_tonal_button(&self.material_theme, "×"),
                )
                .on_hover_text(tr.text("Close search"));
            close.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    tr.text("Close search"),
                )
            });
            if close.clicked() {
                self.library_search.close();
                response.surrender_focus();
            }
        });
    }

    pub(super) fn library_added_strength(&mut self, ui: &egui::Ui, index: usize) -> f32 {
        let Some(added) = &mut self.library_added else {
            return 0.0;
        };
        if !added.ids.contains(self.entries[index].id()) {
            return 0.0;
        }
        let strength = added.strength(ui.input(|input| input.time));
        if strength > 0.0 {
            ui.ctx().request_repaint();
        } else {
            self.library_added = None;
        }
        strength
    }
}

pub(super) fn highlight_frame(
    frame: egui::Frame,
    theme: &super::MaterialTheme,
    strength: f32,
) -> egui::Frame {
    if strength <= 0.0 {
        return frame;
    }
    frame
        .fill(frame.fill.lerp_to_gamma(theme.primary_container, strength))
        .stroke(egui::Stroke::new(
            frame.stroke.width,
            frame.stroke.color.lerp_to_gamma(theme.primary, strength),
        ))
}
