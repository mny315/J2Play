use super::{DocumentKind, FrontendApp, egui, material_card_frame, material_tonal_button};

impl DocumentKind {
    /// Shared title for native and in-window pickers in the current UI language.
    #[must_use]
    pub fn picker_title(self, ctx: &egui::Context) -> String {
        crate::i18n::Translator::from_context(ctx).text(match self {
            Self::Jar => "Choose a JAR",
            Self::Jad => "Choose a JAD",
        })
    }
}

/// Display-only file selection state. Paths and file access remain in the shell.
#[derive(Debug)]
pub struct DocumentBrowser {
    pub kind: DocumentKind,
    pub folder: String,
    pub revision: u64,
    pub parent: Option<u64>,
    pub shortcuts: Vec<DocumentBrowserEntry>,
    pub entries: Vec<DocumentBrowserEntry>,
    pub busy: bool,
    pub truncated: bool,
    pub error: Option<String>,
}

/// An opaque shell identifier, valid only for the current directory listing.
#[derive(Debug)]
pub struct DocumentBrowserEntry {
    pub id: u64,
    pub name: String,
    pub directory: bool,
}

impl FrontendApp {
    pub(super) fn draw_document_browser(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        let Some(browser) = self.platform.document_browser() else {
            return;
        };
        let mut selected = None;
        let mut cancel = false;
        let id = egui::Id::new("document-browser");
        let response = egui::Modal::new(id)
            .area(egui::Modal::default_area(id).constrain_to(self.safe_content_rect))
            .backdrop_color(egui::Color32::TRANSPARENT)
            .frame(material_card_frame(&self.material_theme))
            .show(ctx, |ui| {
                ui.set_width((self.safe_content_rect.width() - 64.0).clamp(160.0, 700.0));
                ui.heading(browser.kind.picker_title(ctx));
                ui.add(egui::Label::new(egui::RichText::new(&browser.folder).strong()).truncate())
                    .on_hover_text(&browser.folder);
                let height = (self.safe_content_rect.height() - 220.0).clamp(80.0, 600.0);
                let revision_id = id.with("revision");
                let changed = ctx.data_mut(|data| {
                    let changed = data.get_temp::<u64>(revision_id) != Some(browser.revision);
                    data.insert_temp(revision_id, browser.revision);
                    changed
                });
                let mut scroll = super::scrolling::vertical()
                    .id_salt("document-folders")
                    .max_height(height);
                if changed {
                    scroll = scroll.vertical_scroll_offset(0.0);
                }
                scroll.show(ui, |ui| {
                    ui.add_enabled_ui(!browser.busy, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if let Some(parent) = browser.parent
                                && ui
                                    .add(material_tonal_button(&self.material_theme, tr.text("Up")))
                                    .clicked()
                            {
                                selected = Some(parent);
                            }
                            for entry in &browser.shortcuts {
                                if ui
                                    .add(material_tonal_button(
                                        &self.material_theme,
                                        tr.text(&entry.name),
                                    ))
                                    .clicked()
                                {
                                    selected = Some(entry.id);
                                }
                            }
                        });
                    });
                    if let Some(error) = &browser.error {
                        ui.colored_label(self.material_theme.error, tr.error(error));
                    }
                    if browser.truncated {
                        ui.label(
                            tr.text("This folder has too many items. Some items are not shown."),
                        );
                    }
                    if browser.busy {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(tr.text("Loading…"));
                        });
                    } else if browser.entries.is_empty() && browser.error.is_none() {
                        ui.label(tr.text("No matching files or folders."));
                    }
                    ui.add_enabled_ui(!browser.busy, |ui| {
                        for entry in &browser.entries {
                            let response = entry_button(ui, entry, self.material_theme.on_surface);
                            super::focus_ring::track(ui, &response, 12.0);
                            if response.clicked() {
                                selected = Some(entry.id);
                            }
                        }
                    });
                });
                ui.add_space(8.0);
                cancel = ui
                    .add(material_tonal_button(
                        &self.material_theme,
                        tr.text("Cancel"),
                    ))
                    .clicked();
            });
        if cancel || response.should_close() {
            self.platform.cancel_document_browser();
        } else if let Some(selected) = selected
            && let Err(error) = self.platform.activate_document_entry(selected)
        {
            self.show_error("Could not open selection", &error);
        }
    }
}

fn entry_button(
    ui: &mut egui::Ui,
    entry: &DocumentBrowserEntry,
    color: egui::Color32,
) -> egui::Response {
    ui.push_id(entry.id, |ui| {
        let icon_id = ui.id().with("folder");
        let mut button = egui::Button::new(&entry.name)
            .truncate()
            .min_size(egui::vec2(ui.available_width(), 48.0));
        if entry.directory {
            button = button.right_text(egui::Atom::custom(icon_id, egui::vec2(24.0, 24.0)));
        }
        let response = button.atom_ui(ui);
        if let Some(rect) = response.rect(icon_id) {
            paint_folder(ui, rect, color);
        }
        response.response
    })
    .inner
}

// Paint the folder directly: the bundled font need not contain a folder or
// triangle glyph, and the reserved atom keeps long names clear of the icon.
fn paint_folder(ui: &egui::Ui, rect: egui::Rect, color: egui::Color32) {
    let origin = rect.center() - egui::vec2(10.0, 8.0);
    let points = [
        (0.0, 0.0),
        (8.0, 0.0),
        (11.0, 3.0),
        (20.0, 3.0),
        (20.0, 16.0),
        (0.0, 16.0),
    ]
    .map(|(x, y)| origin + egui::vec2(x, y));
    ui.painter().add(egui::Shape::closed_line(
        points.to_vec(),
        egui::Stroke::new(1.8, color),
    ));
}
