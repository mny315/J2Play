use crate::api_error;
use diagnostics::EmuError;
use std::sync::Arc;
use std::time::Duration;

pub(super) const CANCEL_POLL: Duration = Duration::from_millis(25);

/// A host-owned cancellation check, independent of guest request timeouts.
#[derive(Clone)]
pub struct Cancellation(Arc<dyn Fn() -> bool + Send + Sync>);

impl Cancellation {
    #[must_use]
    pub fn new(cancelled: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self(Arc::new(cancelled))
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0()
    }

    pub(super) fn check(&self) -> Result<(), EmuError> {
        if self.is_cancelled() {
            Err(api_error("network-io", "network request cancelled by host"))
        } else {
            Ok(())
        }
    }
}

impl Default for Cancellation {
    fn default() -> Self {
        Self::new(|| false)
    }
}

impl std::fmt::Debug for Cancellation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Cancellation")
            .finish_non_exhaustive()
    }
}
