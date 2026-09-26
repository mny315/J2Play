//! Frontend-neutral product contracts shared by the Android and Linux shells.

mod language;
mod library;
pub use language::Language;
pub mod physical_input;
mod preparation;
mod runtime;
mod session;
mod settings;
mod transport;

pub use library::{
    AccentColor, AppSettings, AppTheme, LibraryEntry, LibraryFolder, LibraryFolders, LibraryLoad,
    LibraryRepository, LibraryView, LibraryWarning, MAX_FOLDER_NAME_CHARS, MAX_LIBRARY_ENTRIES,
    MAX_LIBRARY_FOLDERS, MAX_UI_SCALE_PERCENT, MIN_UI_SCALE_PERCENT, RecoveredGameInfo,
};
pub use natives::VibrationRequest;
pub use preparation::{
    ImportInspection, ImportSource, LaunchPlan, PreparedImport, ProfileSummary,
    catalog_fingerprint, inspect_import,
};
pub use runtime::{AUDIO_SAMPLE_RATE, PlatformLifecycleSignal, RuntimeWorker, VM_HOST_STACK_BYTES};
pub use session::{
    AttemptId, Frame, HostDecision, HostRequest, HostRequestKind, InputEvent, KeyState,
    LatestFrameMailbox, LatestTelemetryMailbox, PauseReason, PointerEvent, PointerPhase,
    RuntimeTelemetry, SessionCommand, SessionCommandKind, SessionController, SessionEvent,
    SessionEventKind, SessionId, SessionState,
};
pub use settings::{
    CONTROL_POSITION_UNITS, ControlTransform, FpsLimit, FullscreenMode, GameScale, GameSettings,
    MAX_CONTROL_CORNER_RADIUS_PERCENT, MAX_CONTROL_SIZE_PERCENT, MAX_FPS_LIMIT,
    MAX_PORTRAIT_FRAME_PERCENT, MAX_SCALE_PERCENT, MAX_VIBRATION_STRENGTH_PERCENT,
    MIN_CONTROL_CORNER_RADIUS_PERCENT, MIN_CONTROL_SIZE_PERCENT, MIN_FPS_LIMIT, MIN_SCALE_PERCENT,
    MIN_VIBRATION_STRENGTH_PERCENT, ProfileChoice, ResumeBehavior, VibrationSettings,
    VirtualControlLayout,
};
pub use transport::{AudioMailbox, AudioStats, AudioStatus, VibrationEffect, VibrationMailbox};

#[cfg(test)]
#[path = "../../../tests/support/storage.rs"]
mod test_storage;
