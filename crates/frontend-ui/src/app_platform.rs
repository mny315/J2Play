use super::{
    DocumentOutcome, EmuError, FrontendApp, FullscreenExitGesture, FullscreenHelpState, HashMap,
    Instant, LibraryRepository, MAX_LIBRARY_WARNINGS, MAX_PLATFORM_EVENTS_PER_TICK,
    MAX_UI_TECHNICAL_ERROR_BYTES, MaterialTheme, Path, PauseReason, PlatformBridge, PlatformInsets,
    PlatformLifecycleEvent, PlatformTheme, PlatformThemeMode, ProfileOption, Rect,
    RuntimeDebugState, RuntimeWorker, Screen, SessionController, SliderHapticState, StartupSplash,
    VecDeque, bounded_ui_text, catalog_fingerprint, egui, slider_haptic_due,
};

impl FrontendApp {
    /// Loads app-private library state and creates the shared UI.
    ///
    /// # Errors
    /// Returns a controlled profile-catalog or app-private storage diagnostic.
    pub fn new(
        app_data_root: impl AsRef<Path>,
        platform: Box<dyn PlatformBridge>,
    ) -> Result<Self, EmuError> {
        Self::with_repository(
            LibraryRepository::open(app_data_root.as_ref().join("library"))?,
            platform,
        )
    }

    /// Creates the shared app using a repository whose roots the shell selected.
    /// A repository-owned instance lock also covers the worker's lifetime.
    ///
    /// # Errors
    /// Returns a controlled catalog, storage, worker or platform diagnostic.
    #[allow(clippy::too_many_lines)] // Explicit initialization of frontend-owned state.
    pub fn with_repository(
        repository: LibraryRepository,
        mut platform: Box<dyn PlatformBridge>,
    ) -> Result<Self, EmuError> {
        let profiles = launch::builtin_device_profiles()?;
        let mut profile_options = profiles
            .iter()
            .map(ProfileOption::from_profile)
            .collect::<Vec<_>>();
        profile_options.sort_by(|left, right| {
            left.display_name
                .cmp(&right.display_name)
                .then_with(|| left.profile_id.cmp(&right.profile_id))
        });
        let catalog_fingerprint = catalog_fingerprint(profiles);
        let loaded = repository.load()?;
        let warning_count = loaded.warnings.len();
        let retained_warnings = if warning_count > MAX_LIBRARY_WARNINGS {
            MAX_LIBRARY_WARNINGS.saturating_sub(1)
        } else {
            warning_count
        };
        let mut library_warnings = loaded
            .warnings
            .into_iter()
            .take(retained_warnings)
            .map(|warning| format!("{} [{}]: {}", warning.record, warning.code, warning.detail))
            .collect::<Vec<_>>();
        if warning_count > MAX_LIBRARY_WARNINGS {
            library_warnings.push(format!(
                "<library> [warning-capacity]: {} additional warning(s) were coalesced",
                warning_count - retained_warnings
            ));
        }
        let fullscreen_help = match repository.fullscreen_help_acknowledged() {
            Ok(true) => FullscreenHelpState::Acknowledged,
            Ok(false) => FullscreenHelpState::Pending,
            Err(error) => {
                if library_warnings.len() == MAX_LIBRARY_WARNINGS {
                    library_warnings.remove(0);
                }
                library_warnings.push(format!(
                    "<frontend-state> [{}]: {}",
                    error.code(),
                    bounded_ui_text(error.message(), MAX_UI_TECHNICAL_ERROR_BYTES)
                ));
                FullscreenHelpState::Pending
            }
        };
        let lifecycle = platform.lifecycle_signal();
        let app_settings = repository.load_app_settings();
        let folders = repository.load_folders();
        let runtime = RuntimeWorker::spawn_with_lifecycle(repository.clone(), lifecycle)?;
        let library_assets = super::LibraryAssetLoader::spawn(repository.clone())?;
        platform.bind_runtime_effects(runtime.audio().clone(), runtime.vibration().clone())?;
        let mut app = Self {
            app_settings: app_settings
                .as_ref()
                .cloned()
                .unwrap_or_else(|_| repository.default_app_settings()),
            repository,
            entries: loaded.entries,
            library_warnings,
            profile_options,
            catalog_fingerprint,
            platform,
            runtime,
            runtime_waker_bound: false,
            session: SessionController::default(),
            import_flow: None,
            screen: Screen::Library,
            display_error: None,
            exit_confirmation: false,
            exit_requested: false,
            icons: HashMap::new(),
            library_icon_edge: super::ICON_EDGE,
            library_assets,
            editor_preparation: super::control_editor::EditorPreparation::default(),
            latest_frame: None,
            game_texture: None,
            canvas_rect: None,
            control_regions: Vec::new(),
            stick_region: None,
            touch_owners: HashMap::new(),
            physical_input: frontend_core::physical_input::PhysicalInputState::default(),
            #[cfg(test)]
            observed_input: None,
            slider_repeat: None,
            navigation_repeat: None,
            physical_editor: None,
            physical_bindings: frontend_core::physical_input::PhysicalBindings::default(),
            host_request: None,
            heap_recovery: None,
            data_action: None,
            ime_composition: None,
            last_native_input_time: None,
            text_input_active: false,
            platform_text_input_visible: Some(false),
            external_after_stop: None,
            runtime_diagnostics: VecDeque::new(),
            runtime_debug: RuntimeDebugState::default(),
            fullscreen_help,
            fullscreen_policy: None,
            fullscreen_exit_gesture: FullscreenExitGesture::default(),
            host_orientation: super::PlatformOrientation::Automatic,
            platform_fullscreen: None,
            gameplay_transition: super::gameplay_transition::GameplayTransition::default(),
            ui_motion: super::ui_motion::UiMotion::default(),
            library_motion: super::library_motion::LibraryMotion::default(),
            library_search: super::library_search::LibrarySearch::default(),
            library_selection: super::library_selection::LibrarySelection::default(),
            library_folders: super::library_folders::LibraryFoldersUi::new(
                folders.as_ref().ok().cloned(),
            ),
            library_added: None,
            focus_ring: super::focus_ring::FocusRing::default(),
            platform_insets: PlatformInsets::default(),
            material_theme: MaterialTheme::fallback(PlatformThemeMode::Dark),
            platform_theme: None,
            platform_theme_mode: None,
            platform_suspended: false,
            safe_content_rect: Rect::NOTHING,
            picker_pending: false,
            startup_splash: StartupSplash::load().ok(),
            slider_haptic: SliderHapticState::default(),
            game_haptic_disabled: false,
        };
        if let Err(error) = app_settings {
            app.show_error("Could not load app settings", &error);
        }
        if let Err(error) = folders {
            app.push_library_warning(&format!("Folders [{}]: {}", error.code(), error.message()));
        }
        app.restore_import_draft();
        Ok(app)
    }

    pub(super) fn process_platform_events(&mut self) {
        if matches!(self.screen, Screen::Library)
            && !self.platform_suspended
            && !self.picker_pending
            && !self.exit_requested
            && !self.exit_confirmation
            && self.import_flow.is_none()
            && self.display_error.is_none()
            && self.data_action.is_none()
            && self.library_folders.dialog.is_none()
            && let Some(result) = self.platform.poll_external_document()
        {
            match result {
                Ok(DocumentOutcome::Selected(document)) => self.accept_document(document),
                Ok(DocumentOutcome::Cancelled { .. }) => {}
                Err(error) => self.show_error("Import failed", &error),
            }
        }
        for _ in 0..MAX_PLATFORM_EVENTS_PER_TICK {
            let Some(result) = self.platform.poll_document() else {
                break;
            };
            self.picker_pending = false;
            match result {
                Ok(DocumentOutcome::Selected(document)) => self.accept_document(document),
                Ok(DocumentOutcome::Cancelled { kind }) => self.cancelled_document(kind),
                Err(error) => {
                    self.finish_jad_picker();
                    self.show_error("Import failed", &error);
                }
            }
        }
    }

    pub(super) fn process_lifecycle_events(&mut self) {
        for _ in 0..MAX_PLATFORM_EVENTS_PER_TICK {
            let Some(event) = self.platform.poll_lifecycle() else {
                break;
            };
            match event {
                PlatformLifecycleEvent::Suspended => {
                    self.ui_motion.settle();
                    self.library_motion.settle();
                    self.library_search.finish_animation();
                    self.library_selection.cancel_hold();
                    self.settle_folder_motion();
                    self.gameplay_transition =
                        super::gameplay_transition::GameplayTransition::default();
                    self.platform_suspended = true;
                    self.release_all_input();
                    self.set_lifecycle_paused(true);
                }
                PlatformLifecycleEvent::Resumed => {
                    self.platform_suspended = false;
                    self.set_lifecycle_paused(false);
                }
                PlatformLifecycleEvent::FocusLost => self.release_all_input(),
                PlatformLifecycleEvent::Destroyed => {
                    self.platform_suspended = true;
                    self.stop_active_session();
                }
            }
        }
    }

    pub(super) fn process_platform_insets(&mut self) {
        for _ in 0..MAX_PLATFORM_EVENTS_PER_TICK {
            let Some(result) = self.platform.poll_insets() else {
                break;
            };
            match result {
                Ok(insets) => {
                    if self.platform_insets != insets {
                        // Input geometry is rebuilt during this frame. Do not
                        // let an event arriving with new system insets use the
                        // previous frame's Canvas/control hit regions.
                        self.clear_gameplay_input_geometry();
                        self.platform_insets = insets;
                    }
                }
                Err(error) => self.show_error("Display insets failed", &error),
            }
        }
    }

    pub(super) fn process_platform_theme(&mut self, ctx: &egui::Context) {
        for _ in 0..MAX_PLATFORM_EVENTS_PER_TICK {
            let Some(result) = self.platform.poll_theme() else {
                break;
            };
            match result {
                Ok(platform_theme) => {
                    self.platform_theme = Some(platform_theme);
                }
                Err(error) => self.show_error("System theme failed", &error),
            }
        }

        let platform = self.platform_theme.unwrap_or_else(|| {
            let mode = match ctx.system_theme().unwrap_or(egui::Theme::Dark) {
                egui::Theme::Light => PlatformThemeMode::Light,
                egui::Theme::Dark => PlatformThemeMode::Dark,
            };
            PlatformTheme {
                mode,
                light_colors: None,
                dark_colors: None,
            }
        });
        let theme = MaterialTheme::resolve(
            platform,
            self.app_settings.theme,
            self.app_settings.accent_color,
        );
        if self.material_theme != theme {
            self.material_theme = theme;
            ctx.request_repaint();
        }
        if self.platform_theme_mode != Some(theme.mode) {
            self.platform_theme_mode = Some(theme.mode);
            if let Err(error) = self.platform.set_theme_mode(theme.mode) {
                self.show_error("App appearance failed", &error);
            }
        }
    }

    pub(super) fn set_lifecycle_paused(&mut self, paused: bool) {
        if self.session.active_ids().is_none() {
            return;
        }
        let command = match self.session.set_paused(PauseReason::Lifecycle, paused) {
            Ok(command) => command,
            Err(error) => {
                self.show_error("Lifecycle error", &error);
                return;
            }
        };
        if let Err(error) = self.runtime.submit(command) {
            self.show_error("Lifecycle error", &error);
        }
        self.sync_platform_text_input();
    }

    pub(super) fn request_slider_haptic(&mut self) {
        if self.slider_haptic.disabled {
            return;
        }
        let now = Instant::now();
        if !slider_haptic_due(self.slider_haptic.last_tick, now) {
            return;
        }
        self.slider_haptic.last_tick = Some(now);
        match self.platform.request_selection_haptic() {
            Ok(true) => {}
            Ok(false) => self.slider_haptic.disabled = true,
            Err(error) => {
                self.slider_haptic.disabled = true;
                self.show_error("Haptic feedback failed", &error);
            }
        }
    }

    pub(super) fn request_strength_haptic(&mut self, strength_percent: u8) {
        let now = Instant::now();
        if !slider_haptic_due(self.slider_haptic.last_tick, now) {
            return;
        }
        self.slider_haptic.last_tick = Some(now);
        self.request_game_haptic(strength_percent);
    }

    pub(super) fn request_game_haptic(&mut self, strength_percent: u8) {
        if self.game_haptic_disabled {
            return;
        }
        match self.platform.request_game_haptic(strength_percent) {
            Ok(true) => {}
            Ok(false) => self.game_haptic_disabled = true,
            Err(error) => {
                self.game_haptic_disabled = true;
                if matches!(self.screen, Screen::Gameplay(_)) {
                    if self.runtime_diagnostics.len() == 8 {
                        self.runtime_diagnostics.pop_front();
                    }
                    self.runtime_diagnostics.push_back(bounded_ui_text(
                        &format!("haptic-feedback: {}", error.message()),
                        MAX_UI_TECHNICAL_ERROR_BYTES,
                    ));
                } else {
                    self.show_error("Haptic feedback failed", &error);
                }
            }
        }
    }
}
