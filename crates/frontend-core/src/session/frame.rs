//! Validated guest frames and generation-scoped latest-frame publication.

use super::{AttemptId, SessionId, session_error};
use diagnostics::EmuError;
use std::sync::{Arc, Mutex};

const MAX_FRAME_PIXELS: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub session_id: SessionId,
    pub attempt_id: AttemptId,
    pub width: u32,
    pub height: u32,
    pub canvas_region: platform::LogicalRect,
    pub pixels: Arc<[u32]>,
}

impl Frame {
    /// Creates a complete bounded ARGB framebuffer.
    ///
    /// # Errors
    /// Returns a platform diagnostic for zero, excessive, or mismatched dimensions.
    pub fn new(
        session_id: SessionId,
        attempt_id: AttemptId,
        width: u32,
        height: u32,
        pixels: Arc<[u32]>,
    ) -> Result<Self, EmuError> {
        Self::with_canvas_region(
            session_id,
            attempt_id,
            width,
            height,
            platform::LogicalRect {
                x: 0,
                y: 0,
                width,
                height,
            },
            pixels,
        )
    }

    /// Creates a complete bounded framebuffer with the guest Canvas region
    /// identified inside a larger physical-display image.
    ///
    /// # Errors
    /// Returns a platform diagnostic for invalid pixels or an out-of-frame Canvas.
    pub fn with_canvas_region(
        session_id: SessionId,
        attempt_id: AttemptId,
        width: u32,
        height: u32,
        canvas_region: platform::LogicalRect,
        pixels: Arc<[u32]>,
    ) -> Result<Self, EmuError> {
        let expected = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .filter(|pixels| *pixels != 0 && *pixels <= MAX_FRAME_PIXELS);
        if expected != Some(pixels.len()) {
            return Err(session_error(
                "frame-dimensions",
                "frame dimensions are zero, excessive, or do not match the pixel buffer",
            ));
        }
        let canvas_end_x = canvas_region.x.checked_add(canvas_region.width);
        let canvas_end_y = canvas_region.y.checked_add(canvas_region.height);
        if canvas_region.width == 0
            || canvas_region.height == 0
            || canvas_end_x.is_none_or(|end| end > width)
            || canvas_end_y.is_none_or(|end| end > height)
        {
            return Err(session_error(
                "frame-canvas-region",
                "guest Canvas region is empty or lies outside the framebuffer",
            ));
        }
        Ok(Self {
            session_id,
            attempt_id,
            width,
            height,
            canvas_region,
            pixels,
        })
    }

    /// Converts a physical-frame point into guest Canvas coordinates and
    /// rejects system chrome around a non-fullscreen Canvas.
    #[must_use]
    pub fn canvas_point(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let x = u32::try_from(x).ok()?.checked_sub(self.canvas_region.x)?;
        let y = u32::try_from(y).ok()?.checked_sub(self.canvas_region.y)?;
        if x >= self.canvas_region.width || y >= self.canvas_region.height {
            return None;
        }
        Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))
    }
}

#[derive(Default)]
struct FrameSlot {
    active: Option<(SessionId, AttemptId)>,
    latest: Option<Arc<Frame>>,
}

/// Single-slot latest-frame mailbox. Publication coalesces intermediate frames,
/// and activation rejects stale frames from an earlier attempt.
#[derive(Clone, Default)]
pub struct LatestFrameMailbox {
    inner: Arc<Mutex<FrameSlot>>,
}

impl LatestFrameMailbox {
    /// Activates a generation and atomically drops any previous frame.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn activate(&self, session_id: SessionId, attempt_id: AttemptId) -> Result<(), EmuError> {
        let mut slot = self.lock()?;
        slot.active = Some((session_id, attempt_id));
        slot.latest = None;
        Ok(())
    }

    /// Replaces the pending frame only when it belongs to the active generation.
    /// `false` means the producer was stale and its frame was discarded.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn publish(&self, frame: Arc<Frame>) -> Result<bool, EmuError> {
        let mut slot = self.lock()?;
        if slot.active != Some((frame.session_id, frame.attempt_id)) {
            return Ok(false);
        }
        slot.latest = Some(frame);
        Ok(true)
    }

    /// Takes the newest frame, leaving the slot empty until another publication.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn take_latest(&self) -> Result<Option<Arc<Frame>>, EmuError> {
        Ok(self.lock()?.latest.take())
    }

    /// Deactivates publication and atomically releases the retained framebuffer.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned.
    pub fn clear(&self) -> Result<(), EmuError> {
        let mut slot = self.lock()?;
        slot.active = None;
        slot.latest = None;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, FrameSlot>, EmuError> {
        self.inner.lock().map_err(|_| {
            session_error(
                "frame-mailbox-lock",
                "latest-frame mailbox lock is poisoned",
            )
        })
    }
}
