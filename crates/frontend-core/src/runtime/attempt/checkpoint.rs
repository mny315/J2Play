//! Automatic save scheduling and host input cleanup after a resumed launch.

use super::input::text_edit_call;
use super::{AttemptDriver, EmuError, VecDeque};
use runtime::{key_call, pointer_call};
use serde::{Deserialize, Serialize};
use std::{cell::Cell, rc::Rc};

// Restoration combines these queues before replay. Keep their aggregate
// budget valid for another Stop, regardless of which queue now owns a step.
const MAX_SAVED_STEPS: usize = 512 + 8192 + 64;
const MAX_SAVED_RELEASES: usize = 64;

#[derive(Default)]
pub(super) struct DriverCheckpoints {
    pub(super) enabled: bool,
    pub(super) host_close: Rc<Cell<bool>>,
    saved_on_stop: bool,
}

// Capture borrows the queues; restoration owns the decoded steps.
#[derive(Serialize, Deserialize)]
struct SavedDriver<Queue = VecDeque<vm::DriverStep>> {
    pending: Queue,
    pending_text: Queue,
    releases: Vec<vm::InstanceCall>,
}

impl AttemptDriver {
    pub(super) fn checkpoint_step(&mut self) -> Result<Option<vm::DriverStep>, EmuError> {
        if !self.checkpoints.enabled
            || self.close_queued
            || self.checkpoints.saved_on_stop
            || !self.urgent.stop_requested(self.attempt_id)
            || !matches!(
                self.ams.borrow().state(),
                midp::LifecycleState::Active | midp::LifecycleState::Paused
            )
        {
            return Ok(None);
        }
        self.stop_transient_effects()?;
        self.checkpoints.saved_on_stop = true;
        let mut releases = self
            .input
            .release_events()
            .iter()
            .map(key_call)
            .collect::<Vec<_>>();
        if self.captured_pointer.is_some() {
            releases.push(pointer_call(platform::PointerEvent {
                tick: 0,
                kind: platform::PointerKind::PointerReleased,
                x: self.pointer_position.0,
                y: self.pointer_position.1,
            }));
        }
        if self.composing {
            // Finish saved text edits before clearing the host-only preview.
            // Release callbacks can change focus, so cleanup precedes them.
            releases.insert(0, text_edit_call(1, 0));
        }
        let saved = SavedDriver {
            pending: &self.pending,
            pending_text: &self.pending_text,
            releases,
        };
        let driver_state = save_state::encode_bounded(&saved, 1024 * 1024)?;
        Ok(Some(vm::DriverStep::SaveCheckpoint { driver_state }))
    }

    pub(super) fn restore_driver_checkpoint(&mut self, bytes: &[u8]) -> Result<(), EmuError> {
        if bytes.len() > 1024 * 1024 {
            return Err(super::runtime_error(
                "checkpoint-driver",
                "The saved input state is too large",
            ));
        }
        let saved: SavedDriver = save_state::decode(bytes)?;
        let steps = saved
            .pending
            .len()
            .saturating_add(saved.pending_text.len())
            .saturating_add(saved.releases.len());
        if steps > MAX_SAVED_STEPS
            || saved.releases.len() > MAX_SAVED_RELEASES
            || saved.pending.iter().chain(&saved.pending_text).any(|step| {
                matches!(
                    step,
                    vm::DriverStep::SaveCheckpoint { .. }
                        | vm::DriverStep::CallAfterApplicationShutdown(_)
                        | vm::DriverStep::Stop
                )
            })
        {
            return Err(super::runtime_error(
                "checkpoint-driver",
                "The saved input state is invalid",
            ));
        }
        self.pending = saved.pending;
        self.pending.extend(saved.pending_text);
        self.pending
            .extend(saved.releases.into_iter().map(vm::DriverStep::Call));
        self.ams.borrow_mut().resume_after_restore();
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/frontend-core/runtime/attempt/checkpoint.rs"]
mod tests;
