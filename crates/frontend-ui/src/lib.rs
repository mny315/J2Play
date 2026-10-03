//! Shared localized product UI. Platform objects remain behind [`PlatformBridge`].

mod app_platform;
mod app_settings;
mod control_editor;
mod controls;
mod dialogs;
mod document_browser;
mod eframe_app;
mod focus_navigation;
mod focus_ring;
mod fullscreen;
mod gameplay;
mod gameplay_portrait;
mod gameplay_toolbar;
mod gameplay_transition;
mod i18n;
mod import;
mod layout;
mod library;
mod library_folders;
mod library_icons;
mod library_motion;
mod library_search;
mod library_selection;
mod library_view;
mod navigation;
mod physical_controls;
mod platform_bridge;
mod profile_picker;
mod resource_helpers;
mod resume;
mod runtime_debug;
mod runtime_events;
mod scrolling;
mod session_input;
mod settings;
mod settings_actions;
mod startup_error;
mod startup_splash;
mod switch;
pub use startup_error::{StartupErrorApp, startup_error_text};
mod text_input;
mod text_scale;
pub use text_scale::PlatformTextScale;
mod theme;
mod ui_motion;

use controls::{
    NUMBER_ROW_OUTLINE_WIDTH, VIRTUAL_BUTTON_CORNER_RADIUS, button_rect, control_corner_radius,
    number_button_size, paint_direction_icon, paint_virtual_control_icon, virtual_control_outline,
    virtual_controls_visible,
};
pub use document_browser::{DocumentBrowser, DocumentBrowserEntry};
use fullscreen::FullscreenExitGesture;
use import::ImportFlow;
use layout::{
    apply_settings_style, draw_library_icon, draw_library_text, friendly_profile_name,
    fullscreen_exit_gesture_rect, gameplay_layout, gameplay_panel_margin, inset_content_rect,
    library_game_info, library_launch_rect, library_profile_presentation, profile_display_name,
    settings_slider_width, slider_haptic_due,
};
use library_icons::{CachedIcon, ICON_EDGE, LibraryAssetLoader};
pub use physical_controls::PhysicalInputConfig;
pub use platform_bridge::{
    DocumentKind, DocumentOutcome, OrientationControl, PickedDocument, PlatformBridge,
    PlatformInsets, PlatformLifecycleEvent, PlatformOrientation, PlatformTextInputEvent,
    UnavailablePlatformBridge,
};
use resource_helpers::friendly_message;
use theme::{
    MaterialTheme, apply_material_theme, material_app_bar_frame, material_bottom_bar_frame,
    material_card_frame, material_choice_button, material_danger_button,
    material_error_outlined_button, material_gameplay_app_bar_frame, material_library_card_frame,
    material_outlined_button, material_primary_button, material_settings_slider_frame,
    material_supporting_text, material_text_button, material_tonal_button, paint_material_scrim,
};
pub use theme::{PlatformTheme, PlatformThemeColors, PlatformThemeMode};

use diagnostics::{Category, EmuError, bounded_text as bounded_ui_text};
use eframe::egui::{
    self, Color32, Event, ImeEvent, Key, Pos2, Rect, RichText, TextureHandle, TouchPhase, Vec2,
};
use frontend_core::{
    CONTROL_POSITION_UNITS, ControlTransform, FpsLimit, Frame, GameScale, GameSettings,
    HostDecision, HostRequest, HostRequestKind, ImportInspection, ImportSource, InputEvent,
    KeyState, LibraryEntry, LibraryRepository, MAX_CONTROL_CORNER_RADIUS_PERCENT,
    MAX_CONTROL_SIZE_PERCENT, MAX_FPS_LIMIT, MAX_SCALE_PERCENT, MAX_VIBRATION_STRENGTH_PERCENT,
    MIN_CONTROL_CORNER_RADIUS_PERCENT, MIN_CONTROL_SIZE_PERCENT, MIN_FPS_LIMIT, MIN_SCALE_PERCENT,
    PauseReason, PointerEvent, PointerPhase, PreparedImport, ProfileChoice, RuntimeWorker,
    SessionController, SessionEvent, SessionEventKind, SessionState, VibrationSettings,
    VirtualControlLayout, catalog_fingerprint, inspect_import,
};
pub use platform::HostAction;
use runtime_debug::{RuntimeDebugState, paint_runtime_debug_hud, runtime_debug_lines};
use startup_splash::StartupSplash;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_LIBRARY_WARNINGS: usize = 64;
const MAX_TEXT_INPUT_BYTES: usize = 4 * 1024;
const MAX_PLATFORM_EVENTS_PER_TICK: usize = 64;
const MAX_UI_USER_ERROR_BYTES: usize = 512;
const MAX_UI_TECHNICAL_ERROR_BYTES: usize = 8 * 1024;
// App scale percentages are relative to this baseline; native display density
// and the platform's text-only accessibility scale remain independent.
const BASE_UI_ZOOM: f32 = 0.9;
const LIBRARY_ROW_HEIGHT: f32 = 80.0;
const LIBRARY_COMPACT_ACTION_BREAKPOINT: f32 = 248.0;
const SETTINGS_BODY_TEXT_SIZE: f32 = 15.0;
const SETTINGS_HEADING_TEXT_SIZE: f32 = 20.0;
const SETTINGS_TOUCH_TARGET_HEIGHT: f32 = 48.0;
const PROFILE_MENU_MAX_HEIGHT: f32 = 360.0;
const SETTINGS_SLIDER_VALUE_RESERVE: f32 = 92.0;
const DEFAULT_MANUAL_FPS_LIMIT: u32 = 60;
const SLIDER_HAPTIC_MIN_INTERVAL: Duration = Duration::from_millis(45);
// Gameplay chrome and primary controls use the same logical-point target so
// Android's pixels-per-point scale is applied consistently to the whole UI.
const GAMEPLAY_CONTENT_LEFT_MARGIN: i8 = 10;
const GAMEPLAY_CONTENT_MARGIN: i8 = 8;
const GAMEPLAY_SECTION_GAP: f32 = 8.0;
const GAMEPLAY_MIN_FRAME_HEIGHT: f32 = 72.0;
const GAMEPLAY_MIN_CONTROLS_HEIGHT: f32 = 300.0;
const FULLSCREEN_EXIT_GESTURE_FRACTION: f32 = 0.18;
const FULLSCREEN_EXIT_GESTURE_MIN_HEIGHT: f32 = 96.0;
const FULLSCREEN_EXIT_GESTURE_MAX_HEIGHT: f32 = 160.0;
const FULLSCREEN_TAP_MAX_DURATION: Duration = Duration::from_millis(500);
const FULLSCREEN_DOUBLE_TAP_MAX_GAP: Duration = Duration::from_millis(450);
const FULLSCREEN_TAP_MAX_TRAVEL: f32 = 24.0;
const FULLSCREEN_DOUBLE_TAP_MAX_DISTANCE: f32 = 48.0;
const FULLSCREEN_HELP_INSET: f32 = 8.0;
const FULLSCREEN_HELP_DASH_LENGTH: f32 = 10.0;
const FULLSCREEN_HELP_DASH_GAP: f32 = 6.0;
const KEYPAD_REFERENCE_PORTRAIT: (u32, u32) = (240, 320);
const KEYPAD_REFERENCE_LANDSCAPE: (u32, u32) = (320, 240);

enum Screen {
    Library,
    AppSettings(Box<AppSettingsScreen>),
    Settings(Box<SettingsScreen>),
    Gameplay(Box<GameplayScreen>),
}

struct AppSettingsScreen {
    draft: frontend_core::AppSettings,
    control_editor: Option<ControlEditorState>,
    focus_first: bool,
}

impl From<frontend_core::AppSettings> for Box<AppSettingsScreen> {
    fn from(draft: frontend_core::AppSettings) -> Self {
        Box::new(AppSettingsScreen {
            draft,
            control_editor: None,
            focus_first: true,
        })
    }
}

impl Screen {
    fn focus_first_setting(&mut self) {
        match self {
            Self::AppSettings(settings) => settings.focus_first = true,
            Self::Settings(settings) => settings.focus_profile = true,
            Self::Library | Self::Gameplay(_) => {}
        }
    }

    fn control_editor_mut(&mut self) -> Option<&mut ControlEditorState> {
        match self {
            Self::AppSettings(settings) => settings.control_editor.as_mut(),
            Self::Settings(settings) => settings.control_editor.as_mut(),
            Self::Library | Self::Gameplay(_) => None,
        }
    }

    fn discard_control_editor(&mut self) {
        match self {
            Self::AppSettings(settings) => settings.control_editor = None,
            Self::Settings(settings) => settings.discard_control_editor(),
            Self::Library | Self::Gameplay(_) => {}
        }
    }

    fn commit_control_editor(&mut self) {
        match self {
            Self::AppSettings(settings) => {
                if let Some(editor) = settings.control_editor.take() {
                    let mut controls = settings.draft.control_defaults();
                    editor.apply_to(&mut controls);
                    settings.draft.control_layout = controls.control_layout;
                    settings.draft.landscape_control_layout = controls.landscape_control_layout;
                    settings.draft.vibration = controls.vibration;
                }
            }
            Self::Settings(settings) => settings.commit_control_editor(),
            Self::Library | Self::Gameplay(_) => {}
        }
    }
}

#[derive(Clone)]
struct GameplayScreen {
    entry_id: String,
    title: String,
    canvas_dimensions: (u32, u32),
    pointer_events: bool,
    game_scale: GameScale,
    portrait_frame_percent: Option<u8>,
    control_layout: VirtualControlLayout,
    landscape_control_layout: VirtualControlLayout,
    vibration: VibrationSettings,
    orientation: Option<launch::CanvasOrientation>,
    fast_forward: bool,
    fullscreen: bool,
}

struct SettingsScreen {
    target: SettingsTarget,
    draft: GameSettings,
    focus_profile: bool,
    show_all_profiles: bool,
    editor_request: Option<(u64, frontend_core::ProfileChoice)>,
    control_editor: Option<ControlEditorState>,
}

impl SettingsScreen {
    fn discard_control_editor(&mut self) {
        self.control_editor = None;
    }

    fn commit_control_editor(&mut self) {
        if let Some(editor) = self.control_editor.take() {
            editor.apply_to(&mut self.draft);
        }
    }
}

enum SettingsTarget {
    Existing { entry_id: String },
    PendingImport(Box<PreparedImport>),
}

#[derive(Clone, Debug)]
struct DisplayError {
    title: String,
    user_message: String,
    technical_details: String,
}

impl DisplayError {
    fn from_emu(title: &str, error: &EmuError) -> Self {
        Self {
            title: bounded_ui_text(title, MAX_UI_USER_ERROR_BYTES),
            user_message: bounded_ui_text(&friendly_message(error), MAX_UI_USER_ERROR_BYTES),
            technical_details: bounded_ui_text(
                &format!(
                    "{title}\n{}[{}]: {}",
                    error.category().as_str(),
                    error.code(),
                    error.message()
                ),
                MAX_UI_TECHNICAL_ERROR_BYTES,
            ),
        }
    }

    fn runtime(user_message: &str, technical_details: &str) -> Self {
        Self {
            title: "Game stopped".to_owned(),
            user_message: bounded_ui_text(user_message, MAX_UI_USER_ERROR_BYTES),
            technical_details: bounded_ui_text(technical_details, MAX_UI_TECHNICAL_ERROR_BYTES),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TouchOwner {
    Canvas {
        last_x: i32,
        last_y: i32,
    },
    // Keep keypad ownership through gaps between keys during a drag.
    VirtualKey(Option<HostAction>),
    VirtualKeyPair(HostAction, HostAction),
    VirtualStick {
        action: Option<HostAction>,
        offset: Vec2,
        two_key_diagonals: bool,
    },
}

struct HeapRecovery {
    detail: String,
    candidate_profile_ids: Vec<String>,
}

#[derive(Clone, Eq, PartialEq)]
struct ImeComposition {
    text: String,
    selection_start: usize,
    selection_end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FullscreenHelpState {
    Pending,
    Visible,
    Acknowledged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectionIcon {
    Up,
    UpRight,
    Down,
    DownRight,
    Left,
    DownLeft,
    Right,
    UpLeft,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VirtualControlIcon {
    Fire,
    SoftLeft,
    SoftRight,
}

#[derive(Clone, Copy, Debug)]
struct VirtualControlMetrics {
    direction: f32,
    number: f32,
    soft: f32,
}

#[derive(Clone, Copy, Debug)]
struct VirtualControlGeometry {
    corner_radius_percent: u8,
    direction_pad_center: Pos2,
    direction_button_size: f32,
    fire_center: Pos2,
    fire_size: f32,
    left_soft_key_center: Pos2,
    left_soft_key_size: f32,
    right_soft_key_center: Pos2,
    right_soft_key_size: f32,
    number_keys: [(Pos2, f32); 12],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ControlSelection {
    DirectionPad,
    Fire,
    LeftSoftKey,
    RightSoftKey,
    NumberKey(u8),
}

#[derive(Clone, Debug)]
struct ControlEditorState {
    layout: VirtualControlLayout,
    other_layout: VirtualControlLayout,
    landscape: bool,
    landscape_requested: bool,
    canvas_dimensions: (u32, u32),
    game_scale: GameScale,
    portrait_frame_percent: Option<u8>,
    vibration: VibrationSettings,
    selected: ControlSelection,
    drag_origin: Option<(ControlTransform, Pos2)>,
    navigation: control_editor::ControlNavigation,
    grid_enabled: bool,
}

impl ControlEditorState {
    fn new(settings: &GameSettings, canvas_dimensions: (u32, u32)) -> Self {
        Self {
            layout: settings.control_layout.clone(),
            other_layout: settings.landscape_control_layout.clone(),
            landscape: false,
            landscape_requested: false,
            canvas_dimensions,
            game_scale: settings.game_scale,
            portrait_frame_percent: settings.portrait_frame_percent,
            vibration: settings.vibration,
            selected: ControlSelection::DirectionPad,
            drag_origin: None,
            navigation: control_editor::ControlNavigation::default(),
            grid_enabled: true,
        }
    }

    fn select_orientation(&mut self, landscape: bool) {
        if self.landscape != landscape {
            std::mem::swap(&mut self.layout, &mut self.other_layout);
            self.landscape = landscape;
            self.drag_origin = None;
            self.navigation.moving = None;
        }
    }

    fn reset_layout(&mut self) {
        self.layout = VirtualControlLayout::default();
        self.drag_origin = None;
        self.navigation.moving = None;
    }

    fn apply_to(self, settings: &mut GameSettings) {
        if self.landscape {
            settings.landscape_control_layout = self.layout;
            settings.control_layout = self.other_layout;
        } else {
            settings.control_layout = self.layout;
            settings.landscape_control_layout = self.other_layout;
        }
        settings.vibration = self.vibration;
        settings.controls_override = true;
    }
}

const DIRECTION_PAD_BUTTONS: [(DirectionIcon, HostAction, f32, f32); 8] = [
    (DirectionIcon::UpLeft, HostAction::Num1, -1.0, -1.0),
    (DirectionIcon::UpRight, HostAction::Num3, 1.0, -1.0),
    (DirectionIcon::DownLeft, HostAction::Num7, -1.0, 1.0),
    (DirectionIcon::DownRight, HostAction::Num9, 1.0, 1.0),
    (DirectionIcon::Up, HostAction::Up, 0.0, -1.0),
    (DirectionIcon::Down, HostAction::Down, 0.0, 1.0),
    (DirectionIcon::Left, HostAction::Left, -1.0, 0.0),
    (DirectionIcon::Right, HostAction::Right, 1.0, 0.0),
];

const NUMBER_ROW_KEYS: [(&str, HostAction, u8); 12] = [
    ("1", HostAction::Num1, 0),
    ("2", HostAction::Num2, 1),
    ("3", HostAction::Num3, 2),
    ("4", HostAction::Num4, 3),
    ("5", HostAction::Num5, 4),
    ("6", HostAction::Num6, 5),
    ("7", HostAction::Num7, 6),
    ("8", HostAction::Num8, 7),
    ("9", HostAction::Num9, 8),
    ("*", HostAction::Star, 9),
    ("0", HostAction::Num0, 10),
    ("#", HostAction::Pound, 11),
];

#[derive(Clone, Copy, Debug)]
struct GameplayLayout {
    game_top_padding: f32,
    game_height: f32,
    controls_height: f32,
    gap: f32,
    controls_overlay: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DataActionKind {
    EntryRecord,
    PrivateArchives,
    RuntimeData,
    ResumeSave,
    Game,
}

#[derive(Clone, Debug)]
struct DataAction {
    entry_ids: Vec<String>,
    kind: DataActionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProfileOption {
    profile_id: String,
    display_name: String,
    basic_name: Option<String>,
    short_manufacturer: String,
    canvas_dimensions: Option<(u32, u32)>,
    touch: bool,
}

#[derive(Default)]
struct SliderHapticState {
    last_tick: Option<Instant>,
    disabled: bool,
}

impl ProfileOption {
    fn from_profile(profile: &device_profile::DeviceProfile) -> Self {
        Self {
            profile_id: profile.profile_id().to_owned(),
            display_name: friendly_profile_name(profile),
            basic_name: profile_picker::basic_profile_name(profile),
            short_manufacturer: match profile.device().manufacturer() {
                "Sony Ericsson" => "SE",
                manufacturer => manufacturer,
            }
            .to_owned(),
            canvas_dimensions: profile.canvas_dimensions(),
            touch: profile
                .input()
                .pointer()
                .and_then(|pointer| pointer.events().value())
                .copied()
                .unwrap_or(false),
        }
    }
}

/// Reusable product UI shared by Android and Linux.
// These flags describe independent platform/UI concerns, not mutually
// exclusive states of one state machine.
#[allow(clippy::struct_excessive_bools)]
pub struct FrontendApp {
    repository: LibraryRepository,
    app_settings: frontend_core::AppSettings,
    entries: Vec<LibraryEntry>,
    library_warnings: Vec<String>,
    profile_options: Vec<ProfileOption>,
    catalog_fingerprint: String,
    platform: Box<dyn PlatformBridge>,
    runtime: RuntimeWorker,
    runtime_waker_bound: bool,
    session: SessionController,
    import_flow: Option<ImportFlow>,
    screen: Screen,
    display_error: Option<DisplayError>,
    exit_confirmation: bool,
    exit_requested: bool,
    icons: HashMap<String, CachedIcon>,
    library_icon_edge: u32,
    library_assets: LibraryAssetLoader,
    editor_preparation: control_editor::EditorPreparation,
    latest_frame: Option<Arc<Frame>>,
    game_texture: Option<TextureHandle>,
    canvas_rect: Option<Rect>,
    control_regions: Vec<(Rect, TouchOwner)>,
    stick_region: Option<(Rect, bool)>,
    touch_owners: HashMap<u64, TouchOwner>,
    physical_input: frontend_core::physical_input::PhysicalInputState,
    #[cfg(test)]
    observed_input: Option<Vec<InputEvent>>,
    slider_repeat: Option<physical_controls::SliderRepeat>,
    navigation_repeat: Option<physical_controls::NavigationRepeat>,
    physical_editor: Option<physical_controls::PhysicalEditor>,
    physical_bindings: frontend_core::physical_input::PhysicalBindings,
    gameplay_transition: gameplay_transition::GameplayTransition,
    ui_motion: ui_motion::UiMotion,
    library_motion: library_motion::LibraryMotion,
    library_search: library_search::LibrarySearch,
    library_folders: library_folders::LibraryFoldersUi,
    library_selection: library_selection::LibrarySelection,
    library_added: Option<library_search::AddedGames>,
    focus_ring: focus_ring::FocusRing,
    host_request: Option<HostRequest>,
    heap_recovery: Option<HeapRecovery>,
    data_action: Option<DataAction>,
    ime_composition: Option<ImeComposition>,
    last_native_input_time: Option<u64>,
    text_input_active: bool,
    platform_text_input_visible: Option<bool>,
    external_after_stop: Option<String>,
    runtime_diagnostics: VecDeque<String>,
    runtime_debug: RuntimeDebugState,
    fullscreen_help: FullscreenHelpState,
    fullscreen_policy: Option<fullscreen::GameplayFullscreenPolicy>,
    fullscreen_exit_gesture: FullscreenExitGesture,
    host_orientation: PlatformOrientation,
    platform_fullscreen: Option<bool>,
    platform_insets: PlatformInsets,
    material_theme: MaterialTheme,
    platform_theme: Option<PlatformTheme>,
    platform_theme_mode: Option<PlatformThemeMode>,
    platform_suspended: bool,
    safe_content_rect: Rect,
    picker_pending: bool,
    startup_splash: Option<StartupSplash>,
    slider_haptic: SliderHapticState,
    game_haptic_disabled: bool,
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/mod.rs"]
mod tests;
