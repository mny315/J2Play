use super::{
    AttemptId, HostDecision, InputEvent, PauseReason, SessionCommand, SessionCommandKind,
    SessionEvent, SessionEventKind, SessionId, session_error,
};
use diagnostics::EmuError;

const MAX_ENTRY_ID_BYTES: usize = 80;
const MAX_TEXT_INPUT_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SessionState {
    #[default]
    Idle,
    Starting,
    Running,
    Paused {
        user: bool,
        lifecycle: bool,
    },
    Stopping,
    Failed,
}

/// UI-side state machine. User pause and lifecycle suspension remain separate,
/// while every retry gets a new attempt identifier under the same session.
#[derive(Debug, Default)]
pub struct SessionController {
    next_session: u64,
    next_attempt: u64,
    active: Option<ActiveSession>,
    state: SessionState,
}

#[derive(Debug)]
struct ActiveSession {
    session_id: SessionId,
    attempt_id: AttemptId,
    entry_id: String,
    orientation: Option<launch::CanvasOrientation>,
    started: bool,
    user_paused: bool,
    lifecycle_suspended: bool,
}

impl SessionController {
    #[must_use]
    pub const fn state(&self) -> SessionState {
        self.state
    }

    #[must_use]
    pub fn active_ids(&self) -> Option<(SessionId, AttemptId)> {
        self.active
            .as_ref()
            .map(|active| (active.session_id, active.attempt_id))
    }

    /// Begins a new session for one validated library identifier.
    ///
    /// # Errors
    /// Returns a platform diagnostic if a session is already active or IDs are exhausted.
    pub fn start(&mut self, entry_id: &str) -> Result<SessionCommand, EmuError> {
        self.start_with_orientation(entry_id, None)
    }

    /// Begins a new session with an optional transient guest Canvas orientation.
    ///
    /// # Errors
    /// Returns a platform diagnostic if a session is already active or IDs are exhausted.
    pub fn start_with_orientation(
        &mut self,
        entry_id: &str,
        orientation: Option<launch::CanvasOrientation>,
    ) -> Result<SessionCommand, EmuError> {
        if self.active.is_some() && self.state != SessionState::Failed {
            return Err(session_error(
                "session-active",
                "an emulator session is already active",
            ));
        }
        validate_entry_id(entry_id)?;
        let session_id = SessionId(next_id(&mut self.next_session, "session-id-exhausted")?);
        let attempt_id = AttemptId(next_id(&mut self.next_attempt, "attempt-id-exhausted")?);
        self.active = Some(ActiveSession {
            session_id,
            attempt_id,
            entry_id: entry_id.to_owned(),
            orientation,
            started: false,
            user_paused: false,
            lifecycle_suspended: false,
        });
        self.state = SessionState::Starting;
        Ok(SessionCommand {
            session_id,
            attempt_id,
            kind: SessionCommandKind::Start {
                entry_id: entry_id.to_owned(),
                orientation,
            },
        })
    }

    /// Creates a new launch attempt after an explicit recovery choice.
    ///
    /// # Errors
    /// Returns a platform diagnostic when no failed session exists or IDs are exhausted.
    pub fn retry(&mut self) -> Result<SessionCommand, EmuError> {
        if self.state != SessionState::Failed {
            return Err(session_error(
                "session-retry-state",
                "session retry requires a failed attempt",
            ));
        }
        let attempt_id = AttemptId(next_id(&mut self.next_attempt, "attempt-id-exhausted")?);
        let active = self.active.as_mut().ok_or_else(|| {
            session_error(
                "session-retry-state",
                "failed session has no active identity",
            )
        })?;
        active.attempt_id = attempt_id;
        active.started = false;
        active.user_paused = false;
        active.lifecycle_suspended = false;
        self.state = SessionState::Starting;
        Ok(SessionCommand {
            session_id: active.session_id,
            attempt_id,
            kind: SessionCommandKind::Start {
                entry_id: active.entry_id.clone(),
                orientation: active.orientation,
            },
        })
    }

    /// Sets the pacing mode for the current running attempt.
    ///
    /// # Errors
    /// Returns a platform diagnostic when the attempt is not running.
    pub fn set_fast_forward(&self, enabled: bool) -> Result<SessionCommand, EmuError> {
        let active = self.active.as_ref().ok_or_else(|| {
            session_error("session-inactive", "there is no active emulator session")
        })?;
        if self.state != SessionState::Running {
            return Err(session_error(
                "session-fast-forward-state",
                "fast-forward requires a running emulator session",
            ));
        }
        Ok(SessionCommand {
            session_id: active.session_id,
            attempt_id: active.attempt_id,
            kind: SessionCommandKind::SetFastForward { enabled },
        })
    }

    /// Sets one pause source. Lifecycle resume never clears user pause.
    ///
    /// # Errors
    /// Returns a platform diagnostic when no live attempt exists.
    pub fn set_paused(
        &mut self,
        reason: PauseReason,
        paused: bool,
    ) -> Result<SessionCommand, EmuError> {
        let (session_id, attempt_id) = {
            let active = self.active.as_mut().ok_or_else(|| {
                session_error("session-inactive", "there is no active emulator session")
            })?;
            match reason {
                PauseReason::User => active.user_paused = paused,
                PauseReason::Lifecycle => active.lifecycle_suspended = paused,
            }
            (active.session_id, active.attempt_id)
        };
        self.refresh_running_state();
        Ok(SessionCommand {
            session_id,
            attempt_id,
            kind: SessionCommandKind::SetPause { reason, paused },
        })
    }

    /// Requests bounded worker shutdown for the active attempt.
    ///
    /// # Errors
    /// Returns a platform diagnostic when no session exists.
    pub fn stop(&mut self) -> Result<SessionCommand, EmuError> {
        let active = self.active.as_ref().ok_or_else(|| {
            session_error("session-inactive", "there is no active emulator session")
        })?;
        self.state = SessionState::Stopping;
        Ok(SessionCommand {
            session_id: active.session_id,
            attempt_id: active.attempt_id,
            kind: SessionCommandKind::Stop,
        })
    }

    /// Wraps ordinary input with the current generation IDs.
    ///
    /// # Errors
    /// Returns a platform diagnostic when the session cannot accept input.
    pub fn input(&self, event: InputEvent) -> Result<SessionCommand, EmuError> {
        let active = self.active.as_ref().ok_or_else(|| {
            session_error("session-inactive", "there is no active emulator session")
        })?;
        if !matches!(self.state, SessionState::Running) {
            return Err(session_error(
                "session-input-state",
                "emulator session is not accepting input",
            ));
        }
        if matches!(&event, InputEvent::TextCommit { text } | InputEvent::TextComposition { text, .. } if text.len() > MAX_TEXT_INPUT_BYTES)
        {
            return Err(session_error(
                "session-text-input-limit",
                "committed text exceeds 4096 bytes",
            ));
        }
        Ok(SessionCommand {
            session_id: active.session_id,
            attempt_id: active.attempt_id,
            kind: SessionCommandKind::Input(event),
        })
    }

    /// Atomically releases every key and pointer captured by the active attempt.
    ///
    /// # Errors
    /// Returns a platform diagnostic when no live attempt exists.
    pub fn release_all_input(&self) -> Result<SessionCommand, EmuError> {
        let active = self.active.as_ref().ok_or_else(|| {
            session_error("session-inactive", "there is no active emulator session")
        })?;
        Ok(SessionCommand {
            session_id: active.session_id,
            attempt_id: active.attempt_id,
            kind: SessionCommandKind::ReleaseAllInput,
        })
    }

    /// Resolves one typed request for the current attempt.
    ///
    /// # Errors
    /// Returns a platform diagnostic for an inactive session or zero request ID.
    pub fn resolve_host_request(
        &self,
        request_id: u64,
        decision: HostDecision,
    ) -> Result<SessionCommand, EmuError> {
        if request_id == 0 {
            return Err(session_error(
                "host-request-id",
                "host request identifier must be non-zero",
            ));
        }
        let active = self.active.as_ref().ok_or_else(|| {
            session_error("session-inactive", "there is no active emulator session")
        })?;
        Ok(SessionCommand {
            session_id: active.session_id,
            attempt_id: active.attempt_id,
            kind: SessionCommandKind::ResolveHostRequest {
                request_id,
                decision,
            },
        })
    }

    /// Applies an event only if both session and attempt IDs are current.
    /// Returns `false` for stale output from a prior attempt.
    pub fn apply_event(&mut self, event: &SessionEvent) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        if (event.session_id, event.attempt_id) != (active.session_id, active.attempt_id) {
            return false;
        }
        match event.kind {
            SessionEventKind::Started => {
                active.started = true;
                self.refresh_running_state();
            }
            SessionEventKind::Stopped => {
                self.active = None;
                self.state = SessionState::Idle;
            }
            SessionEventKind::ManagedHeapLimit { .. } | SessionEventKind::TerminalError { .. } => {
                self.state = SessionState::Failed;
            }
            SessionEventKind::LaunchPrepared(_)
            | SessionEventKind::FrameAvailable
            | SessionEventKind::TextInputActive { .. }
            | SessionEventKind::HostRequest(_)
            | SessionEventKind::HostRequestClosed { .. }
            | SessionEventKind::PlatformRequestCancelled
            | SessionEventKind::CheckpointSaveFailed { .. }
            | SessionEventKind::Diagnostic { .. } => {}
        }
        true
    }

    /// Clears a completed failed attempt after the UI has presented the error
    /// or the user has declined recovery. A live or stopping attempt cannot be
    /// discarded through this path.
    #[must_use]
    pub fn discard_failed(&mut self) -> bool {
        if self.state != SessionState::Failed {
            return false;
        }
        self.active = None;
        self.state = SessionState::Idle;
        true
    }

    fn refresh_running_state(&mut self) {
        let Some(active) = &self.active else {
            self.state = SessionState::Idle;
            return;
        };
        if self.state == SessionState::Stopping || self.state == SessionState::Failed {
            return;
        }
        self.state = if !active.started {
            SessionState::Starting
        } else if active.user_paused || active.lifecycle_suspended {
            SessionState::Paused {
                user: active.user_paused,
                lifecycle: active.lifecycle_suspended,
            }
        } else {
            SessionState::Running
        };
    }
}

fn next_id(counter: &mut u64, code: &'static str) -> Result<u64, EmuError> {
    *counter = counter
        .checked_add(1)
        .ok_or_else(|| session_error(code, "session identifier space is exhausted"))?;
    Ok(*counter)
}

fn validate_entry_id(entry_id: &str) -> Result<(), EmuError> {
    if !entry_id.is_empty() && entry_id.len() <= MAX_ENTRY_ID_BYTES {
        Ok(())
    } else {
        Err(session_error(
            "session-entry-id",
            format!("library entry identifier must contain 1..={MAX_ENTRY_ID_BYTES} bytes"),
        ))
    }
}
