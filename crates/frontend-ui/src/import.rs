//! Import draft, descriptor selection, review, and library commit.

use super::{
    Category, DocumentKind, EmuError, FrontendApp, GameSettings, ImportInspection, ImportSource,
    PickedDocument, PreparedImport, RichText, Screen, SettingsScreen, SettingsTarget, Vec2, egui,
    inspect_import, material_card_frame, material_outlined_button, material_primary_button,
    material_supporting_text, material_text_button, material_tonal_button, profile_display_name,
};

pub(super) enum ImportFlow {
    NeedsJad(ImportSource),
    AwaitingJad(Box<Self>),
    ChooseMidlet(Box<ImportInspection>),
    ConfirmProfile(Box<PreparedImport>),
}

impl ImportFlow {
    fn from_source(source: ImportSource) -> Result<Self, EmuError> {
        // Keep the shared archive available if its MIDlet declaration needs a JAD.
        match inspect_import(source.clone())
            .and_then(|inspection| Self::from_inspection(inspection, None))
        {
            Err(error) if error.code() == "midlet-selection" => Ok(Self::NeedsJad(source)),
            result => result,
        }
    }

    fn from_inspection(
        inspection: ImportInspection,
        previous: Option<&Self>,
    ) -> Result<Self, EmuError> {
        let selected = match previous {
            Some(Self::ConfirmProfile(prepared)) => inspection.midlets().iter().find(|midlet| {
                midlet.index == prepared.midlet().index
                    && midlet.class_name == prepared.midlet().class_name
            }),
            _ => None,
        };
        let selected = selected
            .or_else(|| (inspection.midlets().len() == 1).then(|| &inspection.midlets()[0]));
        if let Some(index) = selected.map(|midlet| midlet.index) {
            inspection
                .select_midlet(index)
                .map(|prepared| Self::ConfirmProfile(Box::new(prepared)))
        } else {
            Ok(Self::ChooseMidlet(Box::new(inspection)))
        }
    }

    pub(super) fn review(&self) -> &Self {
        match self {
            Self::AwaitingJad(previous) => previous.review(),
            flow => flow,
        }
    }

    pub(super) fn source(&self) -> &ImportSource {
        match self {
            Self::NeedsJad(source) => source,
            Self::AwaitingJad(previous) => previous.source(),
            Self::ChooseMidlet(inspection) => inspection.source(),
            Self::ConfirmProfile(prepared) => prepared.source(),
        }
    }
}

impl FrontendApp {
    pub(super) fn restore_import_draft(&mut self) {
        match self.repository.restore_import() {
            Ok(Some(source)) => match ImportFlow::from_source(source) {
                Ok(flow) => {
                    // Android can recreate the frontend while its document picker
                    // is open. Accept that result while showing the restored review.
                    self.import_flow = Some(ImportFlow::AwaitingJad(Box::new(flow)));
                }
                Err(error) => {
                    self.discard_import_draft();
                    self.show_error("Import failed", &error);
                }
            },
            Ok(None) => {}
            Err(error) => {
                self.push_library_warning(&format!("Pending import: {}", error.message()));
            }
        }
    }

    pub(super) fn discard_import_draft(&mut self) {
        if let Err(error) = self.repository.discard_import_draft() {
            self.push_library_warning(&format!("Pending import cleanup: {}", error.message()));
        }
    }

    pub(super) fn cancelled_document(&mut self, kind: DocumentKind) {
        if kind == DocumentKind::Jad {
            self.finish_jad_picker();
        }
    }

    pub(super) fn finish_jad_picker(&mut self) {
        if let Some(flow) = self.import_flow.take() {
            self.import_flow = Some(match flow {
                ImportFlow::AwaitingJad(previous) => *previous,
                flow => flow,
            });
        }
    }

    pub(super) fn choose_import_jad(&mut self) {
        self.finish_jad_picker();
        if let Some(flow) = self.import_flow.take() {
            self.import_flow = Some(ImportFlow::AwaitingJad(Box::new(flow)));
            self.request_picker(DocumentKind::Jad);
        }
    }

    pub(super) fn accept_document(&mut self, document: PickedDocument) {
        let result = match document.kind {
            DocumentKind::Jar => ImportFlow::from_source(ImportSource::new(
                document.display_name,
                document.bytes,
                None,
            )),
            DocumentKind::Jad => {
                let Some(ImportFlow::AwaitingJad(previous)) = &self.import_flow else {
                    self.show_error(
                        "Import failed",
                        &EmuError::new(
                            Category::Platform,
                            "unexpected-document-result",
                            "received a JAD result with no pending JAD request",
                        ),
                    );
                    return;
                };
                let source = previous.source().with_jad(document.bytes);
                let result = inspect_import(source)
                    .and_then(|inspection| ImportFlow::from_inspection(inspection, Some(previous)));
                self.finish_jad_picker();
                result
            }
        };
        // Publish either replacement only after inspection and durable staging.
        // A failed JAR or JAD must keep the preceding review and stored draft.
        match result.and_then(|flow| {
            self.repository.stage_import(flow.source())?;
            Ok(flow)
        }) {
            Ok(flow) => self.import_flow = Some(flow),
            Err(error) => self.show_error("Import failed", &error),
        }
    }

    pub(super) fn select_midlet(&mut self, inspection: ImportInspection, index: u32) {
        match inspection.select_midlet(index) {
            Ok(prepared) => {
                self.import_flow = Some(ImportFlow::ConfirmProfile(Box::new(prepared)));
            }
            Err(error) => self.show_error("Import failed", &error),
        }
    }

    pub(super) fn commit_import(
        &mut self,
        prepared: &PreparedImport,
        settings: GameSettings,
    ) -> bool {
        match self.repository.commit_import(prepared, settings) {
            Ok(entry) => {
                self.library_search.close();
                self.library_search.folder = super::library_folders::FolderFilter::All;
                self.library_motion
                    .forget_item(super::library_view::launch_id(entry.id()));
                self.library_added = Some(super::library_search::AddedGames::new([entry
                    .id()
                    .to_owned()]));
                self.icons.remove(entry.id());
                self.library_assets.invalidate();
                self.entries
                    .retain(|candidate| candidate.id() != entry.id());
                self.entries.push(entry);
                self.sort_entries();
                self.discard_import_draft();
                true
            }
            Err(error) => {
                self.show_error("Could not save game", &error);
                false
            }
        }
    }

    pub(super) fn settings_for_import(&self, prepared: &PreparedImport) -> GameSettings {
        self.entries
            .iter()
            .find(|entry| {
                entry.jar_sha256() == prepared.jar_sha256()
                    && entry.midlet_index() == prepared.midlet().index
            })
            .map_or_else(GameSettings::default, |entry| entry.settings().clone())
    }

    pub(super) fn request_picker(&mut self, kind: DocumentKind) {
        match self.platform.request_document(kind) {
            Ok(()) => self.picker_pending = true,
            Err(error) => {
                if kind == DocumentKind::Jad {
                    self.finish_jad_picker();
                }
                self.show_error("Import failed", &error);
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn draw_import_flow(&mut self, ctx: &egui::Context) {
        let tr = crate::i18n::Translator::from_context(ctx);
        let Some(flow) = self.import_flow.as_ref().map(ImportFlow::review) else {
            return;
        };
        let mut choose_jad = false;
        let mut selected_midlet = None;
        let mut confirm_profile = false;
        let mut choose_profile = false;
        let mut cancel = false;

        let title = if matches!(flow, ImportFlow::ChooseMidlet(_)) {
            tr.text("Choose MIDlet")
        } else {
            tr.text("Import game")
        };
        let style = ctx.global_style();
        let margin = egui::Frame::window(&style).total_margin().sum().x;
        let content_width = (self.safe_content_rect.width() - margin).clamp(80.0, 440.0);
        super::dialogs::dialog_window_with_width(ctx, title, content_width + margin)
            .id(egui::Id::new("import-game"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .constrain_to(self.safe_content_rect)
            .show(ctx, |ui| {
                ui.set_width(content_width);
                ui.add_enabled_ui(!self.picker_pending, |ui| {
                    super::scrolling::vertical()
                        .id_salt("import-review")
                        .max_height((self.safe_content_rect.height() - 220.0).clamp(80.0, 480.0))
                        .show(ui, |ui| {
                            if let ImportFlow::ConfirmProfile(prepared) = flow {
                                let summary = prepared.automatic_profile_summary();
                                let profile_name = profile_display_name(
                                    &self.profile_options,
                                    &summary.target_profile_id,
                                )
                                .unwrap_or("Unavailable device profile");
                                ui.label(RichText::new(prepared.title()).strong());
                                ui.add_space(8.0);
                                ui.label(RichText::new(tr.text("Detected device profile")).strong());
                                material_card_frame(&self.material_theme).show(ui, |ui| {
                                    ui.label(RichText::new(tr.text("Profile")).small().strong());
                                    ui.label(tr.profile_name(profile_name));
                                    ui.add_space(4.0);
                                    ui.label(RichText::new(tr.text("Screen")).small().strong());
                                    ui.label(format!(
                                        "{}×{}",
                                        summary.canvas_dimensions.0, summary.canvas_dimensions.1
                                    ));
                                    ui.add_space(4.0);
                                    ui.label(RichText::new(tr.text("Reason")).small().strong());
                                    ui.label(tr.profile_reason(&summary.reason));
                                });
                            } else if let ImportFlow::ChooseMidlet(inspection) = flow {
                                ui.label(tr.text("This suite contains more than one MIDlet."));
                                for midlet in inspection.midlets() {
                                    if ui
                                        .add_sized(
                                            [ui.available_width(), 48.0],
                                            material_outlined_button(
                                                &self.material_theme,
                                                &midlet.name,
                                            ),
                                        )
                                        .on_hover_text(&midlet.class_name)
                                        .clicked()
                                    {
                                        selected_midlet = Some(midlet.index);
                                    }
                                }
                            } else {
                                ui.label(tr.text("This JAR does not declare a MIDlet. Add its JAD descriptor to continue."));
                            }
                            ui.add_space(12.0);
                            ui.label(RichText::new(if matches!(flow, ImportFlow::NeedsJad(_)) {
                                tr.text("JAD descriptor")
                            } else {
                                tr.text("JAD descriptor (optional)")
                            }).small().strong());
                            let has_jad = flow.source().has_jad();
                            ui.label(material_supporting_text(
                                &self.material_theme,
                                if has_jad {
                                    tr.text("JAD added. Its settings are included above.")
                                } else {
                                    tr.text("If you have a separate JAD file, you can add it here.")
                                },
                            ));
                            choose_jad = ui
                                .add(material_tonal_button(
                                    &self.material_theme,
                                    if has_jad { tr.text("Change JAD") } else { tr.text("Choose JAD") },
                                ))
                                .clicked();
                        });
                });
                ui.add_space(10.0);
                if self.picker_pending {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr.text("Waiting for the system document picker…"));
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if matches!(flow, ImportFlow::ConfirmProfile(_)) {
                        confirm_profile = ui
                            .add_enabled(
                                !self.picker_pending,
                                material_primary_button(&self.material_theme, tr.text("Continue")),
                            )
                            .clicked();
                        choose_profile = ui
                            .add_enabled(
                                !self.picker_pending,
                                material_tonal_button(
                                    &self.material_theme,
                                    tr.text("Choose profile"),
                                ),
                            )
                            .clicked();
                    }
                    cancel = ui
                        .add(material_text_button(&self.material_theme, tr.text("Cancel")))
                        .clicked();
                });
            });

        if cancel {
            self.import_flow = None;
            self.discard_import_draft();
            return;
        }
        if choose_jad {
            self.choose_import_jad();
            return;
        }
        if selected_midlet.is_some() || confirm_profile || choose_profile {
            self.finish_jad_picker();
        }
        if let Some(index) = selected_midlet {
            if let Some(ImportFlow::ChooseMidlet(inspection)) = self.import_flow.take() {
                self.select_midlet(*inspection, index);
            }
        } else if confirm_profile {
            if let Some(ImportFlow::ConfirmProfile(prepared)) = self.import_flow.take() {
                let settings = self.settings_for_import(&prepared);
                if !self.commit_import(&prepared, settings) {
                    self.import_flow = Some(ImportFlow::ConfirmProfile(prepared));
                }
            }
        } else if choose_profile
            && let Some(ImportFlow::ConfirmProfile(prepared)) = self.import_flow.take()
        {
            let draft = self.settings_for_import(&prepared);
            self.screen = Screen::Settings(Box::new(SettingsScreen {
                target: SettingsTarget::PendingImport(prepared),
                draft,
                focus_profile: true,
                show_all_profiles: false,
                editor_request: None,
                control_editor: None,
            }));
        }
    }
}
