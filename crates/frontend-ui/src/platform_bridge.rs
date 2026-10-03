//! Public boundary between the shared UI and platform-owned host services.

use super::{DocumentBrowser, PhysicalInputConfig, PlatformTheme, PlatformThemeMode};
use diagnostics::{Category, EmuError};
use frontend_core::{AudioMailbox, PlatformLifecycleSignal, VibrationMailbox};
use platform::HostAction;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentKind {
    Jar,
    Jad,
}

#[derive(Debug)]
pub struct PickedDocument {
    pub kind: DocumentKind,
    pub display_name: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum DocumentOutcome {
    Selected(PickedDocument),
    Cancelled { kind: DocumentKind },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformLifecycleEvent {
    Suspended,
    Resumed,
    FocusLost,
    Destroyed,
}

/// Physical-pixel Android/host occlusion reported by the platform shell.
/// The shared UI converts it to egui points using the active viewport scale.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformInsets {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

/// Host window orientation; independent of the guest Canvas dimensions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlatformOrientation {
    #[default]
    Automatic,
    Portrait,
    Landscape,
}

/// Whether the host window can rotate and how the control editor previews layouts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrientationControl {
    /// A phone rotates its window; Landscape editing follows the physical device.
    HostWindow,
    /// Gameplay follows the window shape; the editor can preview either layout.
    Layout,
}

/// Ordered Android/host IME operations. Composition is a display preview until
/// the IME explicitly commits it to the MIDP editor's public text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformTextInputEvent {
    Key(HostAction),
    Composition {
        text: String,
        selection_start: usize,
        selection_end: usize,
    },
    Commit(String),
    Selection {
        start: usize,
        end: usize,
    },
    Cancel,
    DeleteSurrounding {
        before_chars: usize,
        after_chars: usize,
    },
    DeleteUtf16 {
        before: u16,
        after: u16,
    },
    MoveCaret {
        offset: i16,
    },
}

/// Narrow host boundary used by the shared UI. Android URI/grant/JNI objects
/// never cross it; the shell returns only a display leaf and bounded bytes.
pub trait PlatformBridge {
    /// Text-only system scaling; window density and app zoom are applied separately.
    fn system_text_scale(&self) -> crate::PlatformTextScale {
        crate::PlatformTextScale::default()
    }

    /// Current host language; UI selection remains independent of guest profiles.
    fn system_language(&self) -> frontend_core::Language {
        frontend_core::Language::English
    }

    fn orientation_control(&self) -> OrientationControl {
        OrientationControl::HostWindow
    }

    /// Uses Escape as Back in menus, independently of gameplay assignments.
    fn escape_navigates_back(&self) -> bool {
        false
    }

    /// Desktop window Close exits the app; mobile Back navigates shared screens.
    fn close_exits_app(&self) -> bool {
        false
    }

    /// Cancels pending pickers and host operations before waiting for the worker.
    fn cancel_pending_operations(&mut self) {}

    /// Cancels pending external operations on Stop or an empty platformRequest.
    fn cancel_guest_operations(&mut self) {}

    /// Uses egui-winit's IME service for the guest editor. Android has its own
    /// native editor and leaves this disabled to avoid duplicate input.
    fn uses_window_ime(&self) -> bool {
        false
    }

    /// Drains normalized physical input. Device IDs are transient and never saved.
    fn poll_physical_input(&mut self) -> Option<frontend_core::physical_input::PhysicalInputEvent> {
        None
    }

    /// Names of currently attached controllers, bounded by the shell.
    fn physical_devices(&self) -> Vec<String> {
        Vec::new()
    }

    /// Whether a controller is attached. Shells can avoid copying names each frame.
    fn has_physical_devices(&self) -> bool {
        !self.physical_devices().is_empty()
    }

    /// Routes native keys to capture/gameplay while preserving system keys and IME.
    fn configure_physical_input(
        &mut self,
        _config: PhysicalInputConfig,
        _bindings: &frontend_core::physical_input::PhysicalBindings,
    ) {
    }

    /// Wakes the event-driven UI immediately when native input arrives.
    fn set_physical_input_waker(&mut self, _waker: Arc<dyn Fn() + Send + Sync>) {}

    /// Starts one platform document-picker request.
    ///
    /// # Errors
    /// Returns a display-safe platform diagnostic if the request cannot start.
    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError>;

    /// Polls a completed picker result without blocking the render thread.
    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>>;

    /// Polls a file opened by the system. The UI calls this only when the
    /// library can start an import without replacing gameplay or a draft.
    fn poll_external_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }

    /// Optional in-window picker when the shell cannot use a system dialog.
    fn document_browser(&self) -> Option<&DocumentBrowser> {
        None
    }

    /// Opens a displayed folder or reads the selected file off the UI thread.
    ///
    /// # Errors
    /// Returns a display-safe diagnostic for an expired or unavailable selection.
    fn activate_document_entry(&mut self, _id: u64) -> Result<(), EmuError> {
        Err(EmuError::new(
            Category::Platform,
            "document-browser-unavailable",
            "File selection is unavailable.",
        ))
    }

    /// Cancels the in-window picker, retaining ownership of pending operations.
    fn cancel_document_browser(&mut self) {}

    /// Connects bounded runtime effect mailboxes to the platform audio and
    /// vibration adapters. Implementations retain clones, never VM objects.
    ///
    /// # Errors
    /// Returns a platform diagnostic if an adapter cannot be initialized.
    fn bind_runtime_effects(
        &mut self,
        _audio: AudioMailbox,
        _vibration: VibrationMailbox,
    ) -> Result<(), EmuError> {
        Ok(())
    }

    /// Drains platform lifecycle transitions in order.
    fn poll_lifecycle(&mut self) -> Option<PlatformLifecycleEvent> {
        None
    }

    /// Shares the platform lifecycle state with the VM worker. Shells whose
    /// lifecycle is delivered independently of the render loop override this.
    fn lifecycle_signal(&self) -> PlatformLifecycleSignal {
        PlatformLifecycleSignal::default()
    }

    /// Polls the newest system-bar, cutout, and IME occlusion in physical pixels.
    fn poll_insets(&mut self) -> Option<Result<PlatformInsets, EmuError>> {
        None
    }

    /// Polls the newest host appearance snapshot without blocking.
    ///
    /// A platform may omit semantic colors while still reporting light/dark;
    /// the shared UI then uses its bounded Material fallback palette.
    fn poll_theme(&mut self) -> Option<Result<PlatformTheme, EmuError>> {
        None
    }

    /// Matches host chrome (for example Android system-bar icons) to the
    /// resolved app appearance. Platforms without app-owned chrome may omit it.
    ///
    /// # Errors
    /// Reports a failure to apply the host appearance.
    fn set_theme_mode(&mut self, _mode: PlatformThemeMode) -> Result<(), EmuError> {
        Ok(())
    }

    /// Coalesces pending system Back invocations into one frontend navigation
    /// request. Android 13 and newer deliver the gesture through the platform
    /// back dispatcher instead of a key event.
    fn poll_navigation_back(&mut self) -> bool {
        false
    }

    /// The shell supplies Back through its dispatcher; matching native key
    /// events must not trigger a second navigation action.
    fn uses_platform_navigation_back(&self) -> bool {
        false
    }

    /// Tells the event-driven UI that a platform operation can complete
    /// without producing a native window event.
    fn needs_periodic_poll(&self) -> bool {
        false
    }

    /// Polls one ordered platform IME operation without blocking.
    fn poll_text_input(&mut self) -> Option<Result<PlatformTextInputEvent, EmuError>> {
        None
    }

    /// The shell supplies an editor for product fields as well as guest text.
    fn has_native_text_input(&self) -> bool {
        false
    }

    /// Services nonblocking platform effects and completion signals.
    ///
    /// # Errors
    /// Returns a platform diagnostic if a queued effect cannot be delivered.
    fn pump_runtime_effects(&mut self) -> Result<(), EmuError> {
        Ok(())
    }

    /// Requests a nonblocking selection tick, respecting system haptic settings.
    /// Returns `Ok(false)` when no haptic adapter is available.
    ///
    /// # Errors
    /// Returns an error if a configured adapter cannot accept the request.
    fn request_selection_haptic(&mut self) -> Result<bool, EmuError> {
        Ok(false)
    }

    /// Requests a nonblocking gameplay tick at the given strength percentage.
    /// Returns `Ok(false)` when no controllable haptic adapter is available.
    ///
    /// # Errors
    /// Returns a platform diagnostic when the configured adapter rejects it.
    fn request_game_haptic(&mut self, _strength_percent: u8) -> Result<bool, EmuError> {
        Ok(false)
    }

    /// Shows or hides platform system bars for the gameplay fullscreen mode.
    ///
    /// The shared UI separately owns its gameplay chrome; this hook only asks
    /// the platform shell to update host-owned system UI.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the request cannot be delivered.
    fn set_fullscreen(&mut self, _fullscreen: bool) -> Result<(), EmuError> {
        Ok(())
    }

    /// Reports confirmed native window changes, including compositor actions.
    /// A shell with asynchronous requests waits for acknowledgement before
    /// reporting a different state, and reports a bounded failure on timeout.
    fn poll_fullscreen(&mut self) -> Option<Result<bool, EmuError>> {
        None
    }

    /// Requests a host orientation, or restores system rotation on session exit.
    ///
    /// # Errors
    /// Returns a platform diagnostic if manual rotation is unavailable or the
    /// request cannot be delivered.
    fn set_orientation(&mut self, orientation: PlatformOrientation) -> Result<(), EmuError> {
        if orientation == PlatformOrientation::Automatic {
            return Ok(());
        }
        Err(EmuError::new(
            Category::Platform,
            "orientation-unavailable",
            "Screen rotation is unavailable in this shell",
        ))
    }

    /// Shows or hides the platform text input service.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the text input service is unavailable.
    fn set_text_input_active(&mut self, _active: bool) -> Result<(), EmuError> {
        Ok(())
    }

    /// Opens one user-approved external platform URL.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the URL is rejected or cannot be opened.
    fn open_external_url(&mut self, _url: &str) -> Result<(), EmuError> {
        Err(EmuError::new(
            Category::Platform,
            "platform-request-unavailable",
            "external platform requests are unavailable in this shell",
        ))
    }

    /// Releases platform-owned transient effects during application exit.
    /// Completes platform teardown or returns an error which keeps a requested
    /// desktop close pending in the existing window. Must be idempotent.
    ///
    /// # Errors
    /// Returns an adapter failure or a platform-worker shutdown deadline.
    fn shutdown(&mut self) -> Result<(), EmuError> {
        Ok(())
    }
}

#[derive(Default)]
pub struct UnavailablePlatformBridge;

impl PlatformBridge for UnavailablePlatformBridge {
    fn request_document(&mut self, _kind: DocumentKind) -> Result<(), EmuError> {
        Err(EmuError::new(
            Category::Platform,
            "document-picker-unavailable",
            "the system document picker is unavailable in this platform shell",
        ))
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        None
    }
}
