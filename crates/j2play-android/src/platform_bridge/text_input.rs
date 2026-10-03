//! Bounded IME edits, with cancellation and overflow recovery kept together.

use super::{EmuError, PlatformTextInputEvent, platform_error};
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct TextEvents {
    events: VecDeque<Result<PlatformTextInputEvent, EmuError>>,
    failed: bool,
}

impl TextEvents {
    /// Stop queued edits before publishing a lifecycle barrier. Keep the
    /// native editor's cancellation and every unread diagnostic deliverable.
    pub(super) fn cancel(&mut self) {
        self.events
            .retain(|event| matches!(event, Err(_) | Ok(PlatformTextInputEvent::Cancel)));
    }

    pub(super) fn restart(&mut self) {
        self.events.retain(Result::is_err);
        self.failed = false;
    }

    /// Returns whether an event or the first overflow diagnostic needs a wake.
    pub(super) fn push(&mut self, event: PlatformTextInputEvent) -> bool {
        if self.failed {
            return false;
        }
        let oversized = match &event {
            PlatformTextInputEvent::Commit(text)
            | PlatformTextInputEvent::Composition { text, .. } => text.len() > 4096,
            _ => false,
        };
        if self.events.len() >= 64 || oversized {
            self.failed = true;
            self.events.clear();
            self.events.push_back(Err(platform_error(
                "ime-event-overflow",
                "Android text input exceeded its limit and was cancelled",
            )));
        } else {
            self.events.push_back(Ok(event));
        }
        true
    }

    pub(super) fn pop(&mut self) -> Option<Result<PlatformTextInputEvent, EmuError>> {
        self.events.pop_front()
    }
}
