use diagnostics::{Category, EmuError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// All adapter threads retain their handles until a confirmed join. A failed
/// deadline leaves ownership with the window so closing can be retried.
pub(crate) fn join(thread: &mut Option<JoinHandle<()>>) -> Result<(), EmuError> {
    let deadline = Instant::now() + Duration::from_secs(1);
    while thread.as_ref().is_some_and(|thread| !thread.is_finished()) {
        if Instant::now() >= deadline {
            return Err(error(
                "linux-adapter-deadline",
                "A Linux service did not stop within one second. Try closing again.",
            ));
        }
        thread::sleep(Duration::from_millis(1));
    }
    if let Some(thread) = thread.take() {
        thread.join().map_err(|_| {
            error(
                "linux-adapter-thread",
                "A Linux service stopped unexpectedly.",
            )
        })?;
    }
    Ok(())
}

pub(crate) fn error(code: &'static str, message: &'static str) -> EmuError {
    EmuError::new(Category::Platform, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/operation.rs"]
mod tests;
