//! Live frame-rate policy shared by frontends and the runtime.

use diagnostics::{Category, EmuError};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

/// Shared, live frame-rate policy owned by a frontend session.
///
/// Device/workload policy supplies the automatic ceiling. A frontend may
/// install or clear a manual ceiling without coupling the VM to a particular
/// UI or keeping a repository-side per-game database.
#[derive(Clone, Debug)]
pub struct FrameRateControl {
    limits: Arc<FrameRateLimits>,
}

#[derive(Debug)]
struct FrameRateLimits {
    automatic: AtomicU32,
    manual: AtomicU32,
}

impl FrameRateControl {
    pub const MIN_FRAMES_PER_SECOND: u32 = 1;
    pub const MAX_FRAMES_PER_SECOND: u32 = 1_000;
    const NO_MANUAL_LIMIT: u32 = 0;

    /// Creates a session control with an automatic runtime ceiling.
    ///
    /// # Errors
    /// Returns a platform diagnostic when the ceiling is outside 1..=1000.
    pub fn new(automatic: u32) -> Result<Self, EmuError> {
        validate_frame_rate(automatic)?;
        Ok(Self {
            limits: Arc::new(FrameRateLimits {
                automatic: AtomicU32::new(automatic),
                manual: AtomicU32::new(Self::NO_MANUAL_LIMIT),
            }),
        })
    }

    /// Replaces the automatic ceiling selected by device/workload policy.
    ///
    /// # Errors
    /// Returns a platform diagnostic when the ceiling is outside 1..=1000.
    pub fn set_automatic_limit(&self, frames_per_second: u32) -> Result<(), EmuError> {
        validate_frame_rate(frames_per_second)?;
        self.limits
            .automatic
            .store(frames_per_second, Ordering::Relaxed);
        Ok(())
    }

    /// Installs a manual frontend override, or clears it with `None`.
    ///
    /// # Errors
    /// Returns a platform diagnostic when the ceiling is outside 1..=1000.
    pub fn set_manual_limit(&self, frames_per_second: Option<u32>) -> Result<(), EmuError> {
        if let Some(frames_per_second) = frames_per_second {
            validate_frame_rate(frames_per_second)?;
        }
        self.limits.manual.store(
            frames_per_second.unwrap_or(Self::NO_MANUAL_LIMIT),
            Ordering::Relaxed,
        );
        Ok(())
    }

    /// Returns the active manual frontend override.
    #[must_use]
    pub fn manual_limit(&self) -> Option<u32> {
        match self.limits.manual.load(Ordering::Relaxed) {
            Self::NO_MANUAL_LIMIT => None,
            frames_per_second => Some(frames_per_second),
        }
    }

    /// Returns the effective ceiling used for the next LCDUI frame request.
    #[must_use]
    pub fn effective_limit(&self) -> u32 {
        self.manual_limit()
            .unwrap_or_else(|| self.limits.automatic.load(Ordering::Relaxed))
    }
}

fn validate_frame_rate(frames_per_second: u32) -> Result<(), EmuError> {
    if (FrameRateControl::MIN_FRAMES_PER_SECOND..=FrameRateControl::MAX_FRAMES_PER_SECOND)
        .contains(&frames_per_second)
    {
        return Ok(());
    }
    Err(EmuError::new(
        Category::Platform,
        "invalid-max-fps",
        "frame-rate limit must be between 1 and 1000",
    ))
}

#[cfg(test)]
#[path = "../../../tests/unit/natives/frame_rate.rs"]
mod tests;
