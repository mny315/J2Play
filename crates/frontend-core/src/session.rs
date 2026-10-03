//! Session identities, typed commands/events and the UI-side lifecycle state machine.

use diagnostics::{Category, EmuError, bounded_text};
use platform::HostAction;

mod controller;
mod frame;
mod telemetry;

pub use controller::{SessionController, SessionState};
pub use frame::{Frame, LatestFrameMailbox};
pub use telemetry::{LatestTelemetryMailbox, RuntimeTelemetry};

const MAX_REQUEST_VALUE_BYTES: usize = 4 * 1024;
const MAX_USER_ERROR_BYTES: usize = 512;
const MAX_TECHNICAL_ERROR_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(pub(crate) u64);

impl SessionId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AttemptId(pub(crate) u64);

impl AttemptId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauseReason {
    User,
    Lifecycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerPhase {
    Pressed,
    Dragged,
    Released,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerEvent {
    pub touch_id: u64,
    pub phase: PointerPhase,
    pub canvas_x: i32,
    pub canvas_y: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputEvent {
    Key {
        action: HostAction,
        state: KeyState,
    },
    Pointer(PointerEvent),
    TextCommit {
        text: String,
    },
    TextComposition {
        text: String,
        anchor: u16,
        caret: u16,
    },
    DeleteUtf16 {
        before: u16,
        after: u16,
    },
    MoveTextCaret {
        offset: i16,
    },
    DeleteBackward,
    DeleteForward,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostRequestKind {
    ResumeGame {
        title: String,
        can_resume: bool,
        detail: String,
    },
    Network {
        url: String,
    },
    FileConnection {
        url: String,
    },
    PlatformRequest {
        url: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostRequest {
    pub request_id: u64,
    pub kind: HostRequestKind,
}

impl HostRequest {
    /// Creates a bounded request that is safe to hold in frontend state.
    ///
    /// # Errors
    /// Returns a platform diagnostic when the request identifier or value is invalid.
    pub fn new(request_id: u64, kind: HostRequestKind) -> Result<Self, EmuError> {
        if request_id == 0 {
            return Err(session_error(
                "host-request-id",
                "host request identifier must be non-zero",
            ));
        }
        let value = match &kind {
            HostRequestKind::ResumeGame { title, detail, .. } => {
                if detail.len() > MAX_REQUEST_VALUE_BYTES {
                    return Err(session_error(
                        "host-request-value",
                        "resume detail exceeds its byte limit",
                    ));
                }
                title
            }
            HostRequestKind::Network { url }
            | HostRequestKind::FileConnection { url }
            | HostRequestKind::PlatformRequest { url } => url,
        };
        if value.is_empty() || value.len() > MAX_REQUEST_VALUE_BYTES {
            return Err(session_error(
                "host-request-value",
                format!("host request value must contain 1..={MAX_REQUEST_VALUE_BYTES} bytes"),
            ));
        }
        Ok(Self { request_id, kind })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostDecision {
    ResumeGame { continue_game: bool, remember: bool },
    Deny,
    AllowOnce,
    AllowForSession,
    AllowAfterExit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionCommand {
    pub session_id: SessionId,
    pub attempt_id: AttemptId,
    pub kind: SessionCommandKind,
}

impl SessionCommand {
    /// Start establishes an attempt before its later lifecycle transitions;
    /// stop, pause, input-release, and host decisions precede ordinary input.
    #[must_use]
    pub const fn is_priority(&self) -> bool {
        matches!(
            self.kind,
            SessionCommandKind::Start { .. }
                | SessionCommandKind::SetPause { .. }
                | SessionCommandKind::Stop
                | SessionCommandKind::ReleaseAllInput
                | SessionCommandKind::ResolveHostRequest { .. }
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionCommandKind {
    Start {
        entry_id: String,
        orientation: Option<launch::CanvasOrientation>,
    },
    SetPause {
        reason: PauseReason,
        paused: bool,
    },
    Stop,
    ReleaseAllInput,
    SetFastForward {
        enabled: bool,
    },
    Input(InputEvent),
    ResolveHostRequest {
        request_id: u64,
        decision: HostDecision,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionEvent {
    pub session_id: SessionId,
    pub attempt_id: AttemptId,
    pub kind: SessionEventKind,
}

impl SessionEvent {
    #[must_use]
    pub fn terminal_error(
        session_id: SessionId,
        attempt_id: AttemptId,
        user_message: &str,
        technical_details: &str,
    ) -> Self {
        Self {
            session_id,
            attempt_id,
            kind: SessionEventKind::TerminalError {
                user_message: bounded_text(user_message, MAX_USER_ERROR_BYTES),
                technical_details: bounded_text(technical_details, MAX_TECHNICAL_ERROR_BYTES),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionEventKind {
    LaunchPrepared(Box<crate::LaunchPlan>),
    Started,
    Stopped,
    FrameAvailable,
    TextInputActive {
        active: bool,
    },
    HostRequest(HostRequest),
    HostRequestClosed {
        request_id: u64,
    },
    PlatformRequestCancelled,
    CheckpointSaveFailed {
        detail: String,
    },
    Diagnostic {
        code: String,
        detail: String,
        repeated: u32,
    },
    ManagedHeapLimit {
        detail: String,
        candidate_profile_ids: Vec<String>,
    },
    TerminalError {
        user_message: String,
        technical_details: String,
    },
}

fn session_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Platform, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-core/session/mod.rs"]
mod tests;
