//! Opt-in runtime telemetry with bounded, generation-scoped publication.

use super::{AttemptId, SessionId, session_error};
use diagnostics::EmuError;
use platform::VmDebugStats;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// One generation-tagged snapshot of cumulative emulator telemetry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTelemetry {
    pub session_id: SessionId,
    pub attempt_id: AttemptId,
    pub stats: VmDebugStats,
}

#[derive(Default)]
struct TelemetrySlot {
    active: Option<(SessionId, AttemptId)>,
    latest: Option<RuntimeTelemetry>,
}

/// Opt-in single-slot telemetry mailbox shared by product frontends.
///
/// Publication is disabled by default, coalesces intermediate samples, and
/// rejects stale samples from an earlier launch attempt.
#[derive(Clone, Default)]
pub struct LatestTelemetryMailbox {
    inner: Arc<Mutex<TelemetrySlot>>,
    enabled: Arc<AtomicBool>,
}

impl LatestTelemetryMailbox {
    /// Enables or disables collection and atomically drops the previous sample.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn set_enabled(&self, enabled: bool) -> Result<(), EmuError> {
        let mut slot = self.lock()?;
        slot.latest = None;
        self.enabled.store(enabled, Ordering::Release);
        Ok(())
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    pub(crate) fn activate(
        &self,
        session_id: SessionId,
        attempt_id: AttemptId,
    ) -> Result<(), EmuError> {
        let mut slot = self.lock()?;
        slot.active = Some((session_id, attempt_id));
        slot.latest = None;
        Ok(())
    }

    pub(crate) fn publish(&self, telemetry: RuntimeTelemetry) -> Result<bool, EmuError> {
        if !self.enabled() {
            return Ok(false);
        }
        let mut slot = self.lock()?;
        if !self.enabled() || slot.active != Some((telemetry.session_id, telemetry.attempt_id)) {
            return Ok(false);
        }
        slot.latest = Some(telemetry);
        Ok(true)
    }

    /// Takes the newest sample, leaving the slot empty until the VM publishes again.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn take_latest(&self) -> Result<Option<RuntimeTelemetry>, EmuError> {
        Ok(self.lock()?.latest.take())
    }

    pub(crate) fn clear(&self) -> Result<(), EmuError> {
        let mut slot = self.lock()?;
        slot.active = None;
        slot.latest = None;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, TelemetrySlot>, EmuError> {
        self.inner.lock().map_err(|_| {
            session_error(
                "telemetry-mailbox-lock",
                "latest-telemetry mailbox lock is poisoned",
            )
        })
    }
}
