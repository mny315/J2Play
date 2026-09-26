//! Bounded, typed transport between Android callbacks and the shared frontend.
use diagnostics::{Category, EmuError};
use frontend_core::physical_input::{
    PhysicalBindings, PhysicalControl, PhysicalInputEvent, PhysicalInputState,
};
use frontend_core::{AudioMailbox, PlatformLifecycleSignal, VibrationMailbox, VibrationRequest};
use frontend_ui::PhysicalInputConfig;
use frontend_ui::{
    DocumentKind, DocumentOutcome, PlatformBridge, PlatformInsets, PlatformLifecycleEvent,
    PlatformOrientation, PlatformTextInputEvent, PlatformTheme, PlatformThemeMode,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

mod text_input;
use text_input::TextEvents;

#[derive(Default)]
struct PhysicalEvents {
    pending: VecDeque<PhysicalInputEvent>,
    axes: PhysicalInputState,
    overflowed: bool,
    devices: Vec<(u32, String)>,
    config: PhysicalInputConfig,
    bindings: PhysicalBindings,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    captured_keys: std::collections::BTreeSet<(u32, PhysicalControl)>,
}

impl PhysicalEvents {
    fn push(&mut self, event: PhysicalInputEvent) -> bool {
        if self.overflowed {
            return false;
        }
        if self.pending.len() == 256 {
            self.pending.clear();
            self.pending.push_back(PhysicalInputEvent::Reset);
            self.overflowed = true;
        } else {
            self.pending.push_back(event);
        }
        true
    }
}

pub(crate) fn platform_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Platform, code, message)
}
fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) enum Command {
    Pick(DocumentKind),
    BindAudio(AudioMailbox),
    Vibration(VibrationRequest),
    Haptic(Option<u8>),
    Fullscreen(bool),
    Orientation(PlatformOrientation),
    ThemeMode(PlatformThemeMode),
    Ime(bool),
    OpenUrl { url: String, generation: u64 },
    Shutdown,
}
impl Command {
    pub(crate) fn is_transient(&self) -> bool {
        matches!(self, Self::Vibration(_) | Self::Haptic(_))
    }
    pub(crate) fn replaces(&self, old: &Self) -> bool {
        matches!(
            (self, old),
            (Self::Vibration(_), Self::Vibration(_))
                | (Self::Haptic(_), Self::Haptic(_))
                | (Self::Fullscreen(_), Self::Fullscreen(_))
                | (Self::Orientation(_), Self::Orientation(_))
                | (Self::ThemeMode(_), Self::ThemeMode(_))
                | (Self::Ime(_), Self::Ime(_))
        )
    }
}

#[derive(Default)]
struct DocumentResults {
    generation: u64,
    delivered: bool,
    external: bool,
    result: Option<Result<DocumentOutcome, EmuError>>,
}

#[derive(Default)]
struct Errors {
    pending: VecDeque<(EmuError, u64)>,
    omitted: u64,
}

impl Errors {
    fn push(&mut self, error: EmuError) {
        if let Some((_, count)) = self
            .pending
            .iter_mut()
            .find(|(old, _)| old.code() == error.code() && old.message() == error.message())
        {
            *count = count.saturating_add(1);
        } else if self.pending.len() < 8 {
            self.pending.push_back((error, 1));
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }

    fn pop(&mut self) -> Option<EmuError> {
        if let Some((error, count)) = self.pending.pop_front() {
            return Some(if count == 1 {
                error
            } else {
                platform_error(
                    error.code(),
                    format!("{} ({count} occurrences)", error.message()),
                )
            });
        }
        let omitted = std::mem::take(&mut self.omitted);
        (omitted != 0).then(|| {
            platform_error(
                "android-diagnostics-overflow",
                format!("{omitted} additional Android errors exceeded the diagnostic queue limit"),
            )
        })
    }
}

pub(crate) struct BridgeState {
    physical: Mutex<PhysicalEvents>,
    pub(crate) signal: PlatformLifecycleSignal,
    suspended: AtomicBool,
    release: AtomicBool,
    destroyed: AtomicBool,
    pub(crate) focus_lost: AtomicBool,
    focused: AtomicBool,
    pub(crate) back: AtomicBool,
    document: Mutex<DocumentResults>,
    pub(crate) document_busy: AtomicBool,
    pub(crate) insets: Mutex<Option<Result<PlatformInsets, EmuError>>>,
    pub(crate) language: Mutex<frontend_core::Language>,
    pub(crate) text_scale: Mutex<frontend_ui::PlatformTextScale>,
    pub(crate) theme: Mutex<Option<Result<PlatformTheme, EmuError>>>,
    text: Mutex<TextEvents>,
    errors: Mutex<Errors>,
    external_request_generation: AtomicU64,
}
impl Default for BridgeState {
    fn default() -> Self {
        Self {
            physical: Mutex::new(PhysicalEvents::default()),
            signal: PlatformLifecycleSignal::initially_suspended(),
            suspended: AtomicBool::new(true),
            release: AtomicBool::new(false),
            destroyed: AtomicBool::new(false),
            focus_lost: AtomicBool::new(false),
            focused: AtomicBool::new(true),
            back: AtomicBool::new(false),
            document: Mutex::new(DocumentResults::default()),
            document_busy: AtomicBool::new(false),
            insets: Mutex::new(None),
            language: Mutex::new(frontend_core::Language::English),
            text_scale: Mutex::new(frontend_ui::PlatformTextScale::default()),
            theme: Mutex::new(None),
            text: Mutex::new(TextEvents::default()),
            errors: Mutex::new(Errors::default()),
            external_request_generation: AtomicU64::new(0),
        }
    }
}
impl BridgeState {
    fn accepts_input(&self) -> bool {
        !self.destroyed.load(Ordering::Acquire)
            && !self.suspended.load(Ordering::Acquire)
            && self.focused.load(Ordering::Acquire)
    }

    pub(crate) fn open_external_url<E>(
        &self,
        generation: u64,
        open: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), E> {
        if self.external_request_generation.load(Ordering::Acquire) == generation
            && !self.destroyed.load(Ordering::Acquire)
        {
            open()
        } else {
            Ok(())
        }
    }

    pub(crate) fn physical_event(&self, event: PhysicalInputEvent) {
        let waker = {
            let mut state = lock(&self.physical);
            let accept_presses = self.accepts_input();
            if !accept_presses && matches!(event, PhysicalInputEvent::Button { pressed: true, .. })
            {
                return;
            }
            // Controllers stream all axes even at rest. Apply the shared
            // hysteresis before transport, preserving every digital edge while
            // avoiding idle queue floods and needless UI wakeups.
            let dead_zone = state.bindings.dead_zone_percent;
            let queued = if matches!(event, PhysicalInputEvent::Axis { .. }) {
                let mut queued = false;
                for change in state.axes.update(event, dead_zone) {
                    // Neutral reports still rearm the stick while focus is
                    // elsewhere. A held axis cannot become a new press on resume.
                    if accept_presses || !change.pressed {
                        queued |= state.push(PhysicalInputEvent::Button {
                            device: change.device,
                            control: change.control,
                            pressed: change.pressed,
                        });
                    }
                }
                queued
            } else {
                if matches!(
                    event,
                    PhysicalInputEvent::Disconnected { .. } | PhysicalInputEvent::Reset
                ) {
                    state.axes.update(event, dead_zone);
                }
                state.push(event)
            };
            if !queued {
                return;
            }
            if state.overflowed {
                self.error(platform_error(
                    "physical-input-overflow",
                    "Physical input exceeded its event limit and was released",
                ));
            }
            state.waker.clone()
        };
        if let Some(waker) = waker {
            waker();
        }
    }

    /// A consumed press owns its release even when capture/gameplay ends meanwhile.
    pub(crate) fn route_physical_key(
        &self,
        device: u32,
        control: PhysicalControl,
        pressed: bool,
    ) -> bool {
        let mut input = lock(&self.physical);
        let requested = match control {
            PhysicalControl::Gamepad { .. } | PhysicalControl::Axis { .. } => true,
            PhysicalControl::Keyboard { .. } => {
                input.config.capture || (input.config.gameplay && !input.config.text_input)
            }
            PhysicalControl::AndroidKey { .. } => {
                input.config.capture
                    || (input.config.gameplay && input.bindings.resolve(control).is_some())
            }
        };
        let source = (device, control);
        if pressed {
            if input.captured_keys.contains(&source) {
                return true;
            }
            if requested && input.captured_keys.len() < 256 {
                input.captured_keys.insert(source);
                return true;
            }
            false
        } else {
            input.captured_keys.remove(&source) || requested
        }
    }

    pub(crate) fn physical_device(&self, id: u32, name: Option<String>) {
        {
            let mut state = lock(&self.physical);
            state.devices.retain(|(old, _)| *old != id);
            state.captured_keys.retain(|(device, _)| *device != id);
            if let Some(name) = name
                && state.devices.len() < 8
            {
                let name = name
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .take(128)
                    .collect();
                state.devices.push((id, name));
            }
        }
        // Changed descriptors may also change the button/axis layout.
        self.physical_event(PhysicalInputEvent::Disconnected { device: id });
    }
    pub(crate) fn suspend(&self, suspended: bool) {
        self.suspended.store(suspended, Ordering::Release);
        if suspended {
            lock(&self.text).cancel();
            self.release.store(true, Ordering::Release);
            self.reset_physical_input();
        }
        self.signal.set_suspended(suspended);
    }
    pub(crate) fn focus(&self, focused: bool) {
        self.focused.store(focused, Ordering::Release);
        if !focused {
            lock(&self.text).cancel();
            self.focus_lost.store(true, Ordering::Release);
            self.reset_physical_input();
        }
    }
    fn reset_physical_input(&self) {
        let waker = {
            let mut input = lock(&self.physical);
            // The UI may already have consumed the corresponding press. Keep
            // releases so its held-state cache observes neutral at the barrier.
            input
                .pending
                .retain(|event| !matches!(event, PhysicalInputEvent::Button { pressed: true, .. }));
            input.overflowed = false;
            input.waker.clone()
        };
        if let Some(waker) = waker {
            waker();
        }
    }
    pub(crate) fn destroy(&self) {
        self.suspend(true);
        self.destroyed.store(true, Ordering::Release);
        self.signal.mark_destroyed();
    }
    pub(crate) fn error(&self, error: EmuError) {
        lock(&self.errors).push(error);
    }
    pub(crate) fn input(&self, event: PlatformTextInputEvent) {
        {
            let mut input = lock(&self.text);
            // Cancellation still clears preedit in product fields when the
            // native editor hides, even while the Activity is unfocused.
            if (!self.accepts_input() && event != PlatformTextInputEvent::Cancel)
                || !input.push(event)
            {
                return;
            }
        }
        // Product fields can be idle between keystrokes; text must wake the
        // frontend just like physical input, without a running guest to poll it.
        let waker = lock(&self.physical).waker.clone();
        if let Some(waker) = waker {
            waker();
        }
    }
    pub(crate) fn begin_document_copy(&self) -> u64 {
        let mut document = lock(&self.document);
        document.generation = document.generation.wrapping_add(1);
        document.delivered = false;
        document.generation
    }
    pub(crate) fn complete_document_copy(
        &self,
        generation: u64,
        result: Result<DocumentOutcome, EmuError>,
    ) {
        let mut document = lock(&self.document);
        if !self.destroyed.load(Ordering::Acquire)
            && document.generation == generation
            && !document.delivered
        {
            document.delivered = true;
            document.result = Some(result);
        }
    }
    pub(crate) fn document_result(&self, result: Result<DocumentOutcome, EmuError>) {
        lock(&self.document).result = Some(result);
    }

    pub(crate) fn begin_external_document(&self) -> Result<(), EmuError> {
        if self.document_busy.swap(true, Ordering::AcqRel) {
            return Err(platform_error(
                "document-picker-busy",
                "Another document selection is already active",
            ));
        }
        lock(&self.document).external = true;
        Ok(())
    }

    fn poll_document_result(&self, external: bool) -> Option<Result<DocumentOutcome, EmuError>> {
        let mut document = lock(&self.document);
        if document.external != external {
            return None;
        }
        let result = document.result.take();
        if result.is_some() {
            document.external = false;
            self.document_busy.store(false, Ordering::Release);
        }
        result
    }
}

pub(crate) struct AndroidPlatformBridge {
    state: Arc<BridgeState>,
    vibration: Option<VibrationMailbox>,
    reported_suspended: bool,
    reported_destroyed: bool,
    shutdown_requested: bool,
}
impl AndroidPlatformBridge {
    #[cfg(target_os = "android")]
    pub(crate) fn new() -> Result<Self, EmuError> {
        Ok(Self::with_state(crate::android_adapter::bridge()?))
    }
    fn with_state(state: Arc<BridgeState>) -> Self {
        Self {
            state,
            vibration: None,
            reported_suspended: true,
            reported_destroyed: false,
            shutdown_requested: false,
        }
    }
    #[cfg_attr(
        not(target_os = "android"),
        allow(
            clippy::unused_self,
            clippy::needless_pass_by_value,
            reason = "Host tests retain the Android command ownership interface"
        )
    )]
    fn post(&self, command: Command) -> Result<(), EmuError> {
        #[cfg(target_os = "android")]
        {
            crate::android_adapter::post(&self.state, command)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = command;
            Err(platform_error(
                "android-unavailable",
                "Android SDK is unavailable on this host",
            ))
        }
    }
}
impl PlatformBridge for AndroidPlatformBridge {
    fn system_language(&self) -> frontend_core::Language {
        *lock(&self.state.language)
    }

    fn system_text_scale(&self) -> frontend_ui::PlatformTextScale {
        *lock(&self.state.text_scale)
    }

    fn poll_physical_input(&mut self) -> Option<PhysicalInputEvent> {
        let mut state = lock(&self.state.physical);
        let event = state.pending.pop_front();
        if state.pending.is_empty() {
            state.overflowed = false;
        }
        event
    }
    fn physical_devices(&self) -> Vec<String> {
        lock(&self.state.physical)
            .devices
            .iter()
            .map(|(_, name)| name.clone())
            .collect()
    }
    fn has_physical_devices(&self) -> bool {
        !lock(&self.state.physical).devices.is_empty()
    }
    fn configure_physical_input(
        &mut self,
        config: PhysicalInputConfig,
        bindings: &PhysicalBindings,
    ) {
        let mut state = lock(&self.state.physical);
        state.config = config;
        if state.bindings != *bindings {
            state.bindings.clone_from(bindings);
        }
    }
    fn set_physical_input_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        lock(&self.state.physical).waker = Some(waker);
    }
    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        if self.state.document_busy.swap(true, Ordering::AcqRel) {
            return Err(platform_error(
                "document-picker-busy",
                "Another document selection is already active",
            ));
        }
        let result = self.post(Command::Pick(kind));
        if result.is_err() {
            self.state.document_busy.store(false, Ordering::Release);
        }
        result
    }
    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        self.state.poll_document_result(false)
    }
    fn poll_external_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        self.state.poll_document_result(true)
    }
    fn bind_runtime_effects(
        &mut self,
        audio: AudioMailbox,
        vibration: VibrationMailbox,
    ) -> Result<(), EmuError> {
        if self.vibration.is_some() {
            return Err(platform_error(
                "android-effects-bound",
                "Android effects are already connected",
            ));
        }
        self.post(Command::BindAudio(audio))?;
        self.vibration = Some(vibration);
        Ok(())
    }
    fn poll_lifecycle(&mut self) -> Option<PlatformLifecycleEvent> {
        if self.state.destroyed.load(Ordering::Acquire) {
            return if self.reported_destroyed {
                None
            } else {
                self.reported_destroyed = true;
                Some(PlatformLifecycleEvent::Destroyed)
            };
        }
        if self.state.release.swap(false, Ordering::AcqRel) {
            self.reported_suspended = true;
            return Some(PlatformLifecycleEvent::Suspended);
        }
        if self.state.focus_lost.swap(false, Ordering::AcqRel) {
            return Some(PlatformLifecycleEvent::FocusLost);
        }
        let suspended = self.state.suspended.load(Ordering::Acquire);
        if suspended == self.reported_suspended {
            return None;
        }
        self.reported_suspended = suspended;
        Some(if suspended {
            PlatformLifecycleEvent::Suspended
        } else {
            PlatformLifecycleEvent::Resumed
        })
    }
    fn lifecycle_signal(&self) -> PlatformLifecycleSignal {
        self.state.signal.clone()
    }
    fn poll_insets(&mut self) -> Option<Result<PlatformInsets, EmuError>> {
        lock(&self.state.insets).take()
    }
    fn poll_theme(&mut self) -> Option<Result<PlatformTheme, EmuError>> {
        lock(&self.state.theme).take()
    }
    fn set_theme_mode(&mut self, mode: PlatformThemeMode) -> Result<(), EmuError> {
        self.post(Command::ThemeMode(mode))
    }
    fn poll_navigation_back(&mut self) -> bool {
        self.state.back.swap(false, Ordering::AcqRel)
    }
    fn uses_platform_navigation_back(&self) -> bool {
        true
    }
    fn needs_periodic_poll(&self) -> bool {
        true
    }
    fn poll_text_input(&mut self) -> Option<Result<PlatformTextInputEvent, EmuError>> {
        lock(&self.state.text).pop()
    }

    fn has_native_text_input(&self) -> bool {
        true
    }
    fn pump_runtime_effects(&mut self) -> Result<(), EmuError> {
        if let Some(error) = lock(&self.state.errors).pop() {
            return Err(error);
        }
        if let Some(vibration) = &self.vibration
            && let Some(effect) = vibration.take_latest()?
        {
            self.post(Command::Vibration(effect.request))?;
        }
        Ok(())
    }
    fn request_selection_haptic(&mut self) -> Result<bool, EmuError> {
        self.post(Command::Haptic(None))?;
        Ok(true)
    }
    fn request_game_haptic(&mut self, strength_percent: u8) -> Result<bool, EmuError> {
        if !(1..=100).contains(&strength_percent) {
            return Err(platform_error(
                "android-haptic-strength",
                "Haptic strength must be between 1 and 100 percent",
            ));
        }
        self.post(Command::Haptic(Some(strength_percent)))?;
        Ok(true)
    }
    fn set_fullscreen(&mut self, fullscreen: bool) -> Result<(), EmuError> {
        self.post(Command::Fullscreen(fullscreen))
    }
    fn set_orientation(&mut self, orientation: PlatformOrientation) -> Result<(), EmuError> {
        self.post(Command::Orientation(orientation))
    }
    fn set_text_input_active(&mut self, active: bool) -> Result<(), EmuError> {
        if active {
            lock(&self.state.text).restart();
        }
        self.post(Command::Ime(active))
    }
    fn open_external_url(&mut self, url: &str) -> Result<(), EmuError> {
        validate_url(url)?;
        self.post(Command::OpenUrl {
            url: url.to_owned(),
            generation: self
                .state
                .external_request_generation
                .load(Ordering::Acquire),
        })
    }
    fn cancel_guest_operations(&mut self) {
        // Invalidate queued commands, including a drained Activity batch whose
        // URL dispatch has not started yet.
        self.state
            .external_request_generation
            .fetch_add(1, Ordering::AcqRel);
    }
    fn shutdown(&mut self) -> Result<(), EmuError> {
        self.cancel_guest_operations();
        self.vibration = None;
        if !self.shutdown_requested {
            self.post(Command::Shutdown)?;
            self.shutdown_requested = true;
        }
        Ok(())
    }
}
fn validate_url(url: &str) -> Result<(), EmuError> {
    if url.is_empty() || url.len() > 4096 || url.contains('\0') {
        return Err(platform_error(
            "platform-request-url",
            "External URL is empty, invalid, or oversized",
        ));
    }
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .filter(|scheme| {
            let mut bytes = scheme.bytes();
            bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
                && bytes
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
        })
        .ok_or_else(|| {
            platform_error("platform-request-url", "External URL has no valid scheme")
        })?;
    if scheme.eq_ignore_ascii_case("file") || scheme.eq_ignore_ascii_case("content") {
        return Err(platform_error(
            "platform-request-scheme",
            "External file and content URLs are not allowed",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-android/platform_bridge/mod.rs"]
mod tests;
