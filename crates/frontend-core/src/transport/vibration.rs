//! Replacement vibration effects and the active host actuator policy.

use super::transport_error;
use crate::{AttemptId, SessionId, VibrationSettings};
use diagnostics::EmuError;
use natives::VibrationRequest;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VibrationEffect {
    pub session_id: SessionId,
    pub attempt_id: AttemptId,
    pub request: VibrationRequest,
}

#[derive(Default)]
struct VibrationState {
    active: Option<(SessionId, AttemptId)>,
    suspended: bool,
    policy: VibrationSettings,
    latest: Option<VibrationEffect>,
    unavailable: bool,
}

/// Replacement mailbox matching Android vibrator semantics: each new effect
/// supersedes the previous one, including an explicit stop.
#[derive(Clone, Default)]
pub struct VibrationMailbox {
    inner: Arc<Mutex<VibrationState>>,
}

impl VibrationMailbox {
    pub(crate) fn activate(
        &self,
        session_id: SessionId,
        attempt_id: AttemptId,
    ) -> Result<(), EmuError> {
        let mut state = self.lock()?;
        let unavailable = state.unavailable;
        *state = VibrationState {
            active: Some((session_id, attempt_id)),
            suspended: false,
            policy: VibrationSettings::default(),
            unavailable,
            // Activation must also cancel an effect from the preceding attempt
            // if the platform has not consumed its teardown Stop yet.
            latest: Some(VibrationEffect {
                session_id,
                attempt_id,
                request: VibrationRequest::Stop,
            }),
        };
        Ok(())
    }

    pub(crate) fn configure(
        &self,
        session_id: SessionId,
        attempt_id: AttemptId,
        policy: VibrationSettings,
    ) -> Result<(), EmuError> {
        let mut state = self.lock()?;
        if state.active != Some((session_id, attempt_id)) {
            return Ok(());
        }
        state.policy = policy;
        Ok(())
    }

    pub(crate) fn publish(&self, effect: VibrationEffect) -> Result<bool, EmuError> {
        let mut state = self.lock()?;
        if state.unavailable {
            return Ok(false);
        }
        if state.active != Some((effect.session_id, effect.attempt_id)) {
            return Ok(false);
        }
        if state.suspended && effect.request != VibrationRequest::Stop {
            return Ok(true);
        }
        state.latest = Some(VibrationEffect {
            request: state.policy.apply_to(effect.request),
            ..effect
        });
        Ok(true)
    }

    pub(crate) fn set_suspended(
        &self,
        session_id: SessionId,
        attempt_id: AttemptId,
        suspended: bool,
    ) -> Result<(), EmuError> {
        let mut state = self.lock()?;
        if state.active != Some((session_id, attempt_id)) || state.suspended == suspended {
            return Ok(());
        }
        state.suspended = suspended;
        if suspended {
            state.latest = Some(VibrationEffect {
                session_id,
                attempt_id,
                request: VibrationRequest::Stop,
            });
        }
        Ok(())
    }

    /// Takes the newest replacement effect, if any.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn take_latest(&self) -> Result<Option<VibrationEffect>, EmuError> {
        Ok(self.lock()?.latest.take())
    }

    /// Reports the actual host actuator capability, including hotplug/loss.
    /// Unavailable adapters reject guest requests instead of claiming success.
    ///
    /// # Errors
    /// Returns a diagnostic if the mailbox lock is poisoned.
    pub fn set_available(&self, available: bool) -> Result<(), EmuError> {
        let mut state = self.lock()?;
        if state.unavailable != available {
            return Ok(());
        }
        state.unavailable = !available;
        if !available && let Some((session_id, attempt_id)) = state.active {
            state.latest = Some(VibrationEffect {
                session_id,
                attempt_id,
                request: VibrationRequest::Stop,
            });
        }
        Ok(())
    }

    pub(crate) fn stop_and_deactivate(&self) -> Result<(), EmuError> {
        let mut state = self.lock()?;
        state.suspended = true;
        if let Some((session_id, attempt_id)) = state.active.take() {
            state.latest = Some(VibrationEffect {
                session_id,
                attempt_id,
                request: VibrationRequest::Stop,
            });
        }
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, VibrationState>, EmuError> {
        self.inner
            .lock()
            .map_err(|_| transport_error("vibration-lock", "vibration mailbox lock is poisoned"))
    }
}
