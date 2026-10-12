use diagnostics::EmuError;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(2);

/// The compositor can acknowledge after several frames or exit fullscreen
/// independently. Retain one request, never a queue of obsolete toggles.
#[derive(Default)]
pub(crate) struct Fullscreen {
    reported: Option<bool>,
    requested: Option<(bool, Instant)>,
}

impl Fullscreen {
    pub(crate) fn request(&mut self, fullscreen: bool, now: Instant) {
        self.requested = Some((fullscreen, now));
    }

    pub(crate) fn pending(&self) -> bool {
        self.requested.is_some()
    }

    pub(crate) fn poll(
        &mut self,
        native: Option<bool>,
        now: Instant,
    ) -> Option<Result<bool, EmuError>> {
        if let Some((requested, started)) = self.requested {
            if native == Some(requested) {
                self.requested = None;
                self.reported = native;
                return Some(Ok(requested));
            }
            if now.saturating_duration_since(started) < TIMEOUT {
                return None;
            }
            self.requested = None;
            // Reconcile to the real window on the next poll after the error.
            self.reported = None;
            return Some(Err(crate::operation::error(
                "linux-fullscreen-timeout",
                "The window manager did not confirm the fullscreen change. Try again.",
            )));
        }
        let native = native?;
        if self.reported.replace(native) == Some(native) {
            None
        } else {
            Some(Ok(native))
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/fullscreen.rs"]
mod tests;
