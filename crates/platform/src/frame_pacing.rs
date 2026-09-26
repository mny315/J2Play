use std::time::{Duration, Instant};

/// Real-time LCDUI frame pacer driven by a live [`natives::FrameRateControl`].
///
/// Changing either ceiling resets the pending deadline so a frontend update
/// cannot release a burst of catch-up frames.
pub struct FramePacer {
    control: natives::FrameRateControl,
    active_limit: u32,
    interval: Duration,
    pub(crate) next_frame_at: Option<Instant>,
    pub(crate) presentation_pre_paced: bool,
}

impl FramePacer {
    #[must_use]
    pub fn new(control: natives::FrameRateControl) -> Self {
        let active_limit = control.effective_limit();
        Self {
            control,
            active_limit,
            interval: frame_interval(active_limit),
            next_frame_at: None,
            presentation_pre_paced: false,
        }
    }

    /// Returns a clone of the live control for frontend wiring.
    #[must_use]
    pub fn control(&self) -> natives::FrameRateControl {
        self.control.clone()
    }

    /// Reserves a guest `Canvas.repaint` slot. The host performs the returned
    /// wait through its normal cancellation-aware pacing path.
    pub fn frame_request_delay_millis(&mut self, enabled: bool) -> u64 {
        let delay = self.request_delay(Instant::now(), enabled);
        delay_millis(delay)
    }

    /// Reserves a presentation deadline for cooperative VM scheduling.
    /// A partial millisecond rounds up so the guest cannot present early.
    pub fn frame_delay_millis(&mut self, enabled: bool, changed: bool) -> u64 {
        let delay = self.presentation_delay(Instant::now(), enabled, changed);
        delay_millis(delay)
    }

    pub(crate) fn request_delay(&mut self, now: Instant, enabled: bool) -> Duration {
        if !self.refresh_policy(enabled) {
            return Duration::ZERO;
        }
        let delay = self.scheduled_delay(now);
        self.presentation_pre_paced = true;
        delay
    }

    pub(crate) fn presentation_delay(
        &mut self,
        now: Instant,
        enabled: bool,
        changed: bool,
    ) -> Duration {
        if !self.refresh_policy(enabled) {
            return Duration::ZERO;
        }
        if std::mem::take(&mut self.presentation_pre_paced) || !changed {
            return Duration::ZERO;
        }
        self.scheduled_delay(now)
    }

    fn refresh_policy(&mut self, enabled: bool) -> bool {
        let limit = self.control.effective_limit();
        if limit != self.active_limit {
            self.active_limit = limit;
            self.interval = frame_interval(limit);
            self.next_frame_at = None;
            self.presentation_pre_paced = false;
        }
        if !enabled {
            self.next_frame_at = None;
            self.presentation_pre_paced = false;
            return false;
        }
        true
    }

    fn scheduled_delay(&mut self, now: Instant) -> Duration {
        let Some(mut deadline) = self.next_frame_at else {
            self.next_frame_at = now.checked_add(self.interval);
            return Duration::ZERO;
        };

        // Retain at most one second of missed frame deadlines. This lets a
        // frame-driven guest catch back up after bounded CPU or host delay
        // instead of permanently converting every late frame into another
        // full interval. Long pauses cannot create an unbounded burst.
        let maximum_credit = self
            .interval
            .saturating_mul(self.active_limit.saturating_sub(1));
        if let Some(earliest) = now.checked_sub(maximum_credit)
            && deadline < earliest
        {
            deadline = earliest;
        }
        self.next_frame_at = deadline.checked_add(self.interval);
        deadline.saturating_duration_since(now)
    }
}

fn delay_millis(delay: Duration) -> u64 {
    u64::try_from(delay.as_nanos().div_ceil(1_000_000)).unwrap_or(u64::MAX)
}

pub(crate) fn frame_interval(frames_per_second: u32) -> Duration {
    Duration::from_nanos(1_000_000_000 / u64::from(frames_per_second))
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/frame_pacing/mod.rs"]
mod tests;
