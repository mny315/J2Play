//! `MIDlet` lifecycle state machine and VM callback requests.

use diagnostics::{Category, EmuError};
use natives::{LifecycleCallback, LifecycleOutcome, MidletNotification};
use std::collections::VecDeque;
use vm::{CallTarget, InstanceCall, Value};

const MAX_LIFECYCLE_EVENTS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LifecycleAction {
    Start,
    Pause,
    Resume,
    Destroy { unconditional: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LifecycleState {
    New,
    Active,
    Paused,
    Destroyed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HostEvent {
    Launch,
    Pause,
    Resume,
    Close,
    Destroy { unconditional: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct PendingAction {
    action: LifecycleAction,
    previous_state: LifecycleState,
}

/// Stateful, headless application-management service for one isolated suite.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Ams {
    state: LifecycleState,
    events: VecDeque<HostEvent>,
    resume_requested: bool,
    pending: Option<PendingAction>,
    force_destroy: bool,
    // Re-established by the host after every restore; absent from the saved format.
    #[serde(skip)]
    restore_resume_pending: bool,
}

impl Ams {
    /// Validates a decoded checkpoint before its lifecycle can resume.
    ///
    /// # Errors
    /// Returns an error for closing, destroyed or unbounded lifecycle state.
    pub fn validate_checkpoint(&self) -> Result<(), EmuError> {
        if !matches!(self.state, LifecycleState::Active | LifecycleState::Paused)
            || self.events.len() > MAX_LIFECYCLE_EVENTS
            || self.force_destroy
            || self
                .events
                .iter()
                .any(|event| matches!(event, HostEvent::Close | HostEvent::Destroy { .. }))
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| matches!(pending.action, LifecycleAction::Destroy { .. }))
        {
            return Err(EmuError::new(
                Category::Api,
                "checkpoint-lifecycle",
                "The automatic save contains an invalid MIDlet lifecycle",
            ));
        }
        Ok(())
    }

    /// Creates a suite lifecycle with a bounded initial event sequence.
    ///
    /// # Errors
    /// Rejects more than 16 initial events without consuming the remaining iterator.
    pub fn new(events: impl IntoIterator<Item = HostEvent>) -> Result<Self, EmuError> {
        let events: VecDeque<_> = events.into_iter().take(MAX_LIFECYCLE_EVENTS + 1).collect();
        if events.len() > MAX_LIFECYCLE_EVENTS {
            return Err(event_capacity_error());
        }
        Ok(Self {
            state: LifecycleState::New,
            events,
            resume_requested: false,
            pending: None,
            force_destroy: false,
            restore_resume_pending: false,
        })
    }

    #[must_use]
    pub fn state(&self) -> LifecycleState {
        self.state
    }

    /// Queues one host lifecycle event for the next driver boundary.
    ///
    /// # Errors
    /// Rejects a full queue without changing queued or in-flight callbacks.
    pub fn host_event(&mut self, event: HostEvent) -> Result<(), EmuError> {
        if matches!(
            event,
            HostEvent::Close
                | HostEvent::Destroy {
                    unconditional: true
                }
        ) {
            self.request_close();
            return Ok(());
        }
        if self.state != LifecycleState::Destroyed && !self.force_destroy {
            if self.events.len() + usize::from(self.restore_resume_pending) >= MAX_LIFECYCLE_EVENTS
            {
                return Err(event_capacity_error());
            }
            // New host events follow the resume already requested by restoration.
            if std::mem::take(&mut self.restore_resume_pending) {
                self.events.push_back(HostEvent::Resume);
            }
            self.events.push_back(event);
        }
        Ok(())
    }

    /// Requests unconditional teardown even when the event queue is full.
    pub fn request_close(&mut self) {
        // Destroyed is provisional while destroyApp is running: a conditional
        // request may still be refused, so retain a later unconditional close.
        if self.state == LifecycleState::Destroyed && self.pending.is_none() {
            return;
        }
        // Closing supersedes queued launches and resumes. An in-flight
        // callback still finishes through the normal pending action.
        self.events.clear();
        self.resume_requested = false;
        self.force_destroy = true;
        self.restore_resume_pending = false;
    }

    /// Resumes after the saved callbacks, including when their queue is full.
    /// This single host action is re-established on each restore, not serialized.
    pub fn resume_after_restore(&mut self) {
        self.restore_resume_pending = true;
    }

    /// Applies a notification synchronously, before control returns from the
    /// Java callback which emitted it.
    pub fn notify(&mut self, notification: MidletNotification) {
        match notification {
            MidletNotification::Destroyed => {
                self.state = LifecycleState::Destroyed;
                self.resume_requested = false;
                self.pending = None;
                self.force_destroy = false;
                self.restore_resume_pending = false;
                self.events.clear();
            }
            MidletNotification::Paused if self.state == LifecycleState::Active => {
                self.state = LifecycleState::Paused;
            }
            MidletNotification::ResumeRequested if self.state == LifecycleState::Paused => {
                self.resume_requested = true;
            }
            MidletNotification::Paused | MidletNotification::ResumeRequested => {}
        }
    }

    /// Completes the callback currently owned by the AMS. Lifecycle wrappers
    /// report checked state-change refusals separately from runtime failures so
    /// the host can apply the MIDP state machine without making the VM aware of
    /// `javax.microedition.midlet` exception classes.
    ///
    /// # Errors
    /// Returns a controlled diagnostic for a forged or out-of-order callback
    /// result.
    pub fn callback_finished(
        &mut self,
        callback: LifecycleCallback,
        outcome: LifecycleOutcome,
    ) -> Result<(), EmuError> {
        if self.state == LifecycleState::Destroyed && self.pending.is_none() {
            return Ok(());
        }
        let pending = self.pending.take().ok_or_else(|| {
            EmuError::new(
                Category::Api,
                "lifecycle-callback",
                "MIDlet reported a lifecycle result without a pending callback",
            )
        })?;
        let expected = match pending.action {
            LifecycleAction::Start | LifecycleAction::Resume => LifecycleCallback::Start,
            LifecycleAction::Pause => LifecycleCallback::Pause,
            LifecycleAction::Destroy { .. } => LifecycleCallback::Destroy,
        };
        if callback != expected {
            self.pending = Some(pending);
            return Err(EmuError::new(
                Category::Api,
                "lifecycle-callback",
                "MIDlet reported an out-of-order lifecycle result",
            ));
        }
        match (pending.action, outcome) {
            (_, LifecycleOutcome::Completed) => {}
            (
                LifecycleAction::Start | LifecycleAction::Resume,
                LifecycleOutcome::StateChangeRejected,
            ) => self.state = LifecycleState::Paused,
            (
                LifecycleAction::Destroy {
                    unconditional: false,
                },
                LifecycleOutcome::StateChangeRejected,
            ) => self.state = pending.previous_state,
            (
                LifecycleAction::Destroy {
                    unconditional: true,
                },
                LifecycleOutcome::StateChangeRejected,
            ) => self.state = LifecycleState::Destroyed,
            (LifecycleAction::Pause, LifecycleOutcome::StateChangeRejected)
            | (
                LifecycleAction::Start | LifecycleAction::Resume | LifecycleAction::Pause,
                LifecycleOutcome::RuntimeFailure,
            ) => {
                self.request_close();
            }
            (LifecycleAction::Destroy { .. }, LifecycleOutcome::RuntimeFailure) => {
                self.state = LifecycleState::Destroyed;
            }
        }
        Ok(())
    }

    /// Returns the next legal callback after applying current state and queued
    /// host events. Events which do not represent a valid transition are
    /// deliberately ignored.
    #[must_use]
    pub fn next_action(&mut self) -> Option<LifecycleAction> {
        if self.state == LifecycleState::Destroyed || self.pending.is_some() {
            return None;
        }
        if self.force_destroy {
            self.force_destroy = false;
            return Some(self.begin(LifecycleAction::Destroy {
                unconditional: true,
            }));
        }
        if self.resume_requested && self.state == LifecycleState::Paused {
            self.resume_requested = false;
            return Some(self.begin(LifecycleAction::Resume));
        }
        while let Some(event) = self.events.pop_front() {
            let action = match (self.state, event) {
                (LifecycleState::New, HostEvent::Launch) => Some(LifecycleAction::Start),
                (LifecycleState::Active, HostEvent::Pause) => Some(LifecycleAction::Pause),
                (LifecycleState::Paused, HostEvent::Resume) => Some(LifecycleAction::Resume),
                (
                    LifecycleState::New | LifecycleState::Active | LifecycleState::Paused,
                    HostEvent::Close,
                ) => Some(LifecycleAction::Destroy {
                    unconditional: true,
                }),
                (
                    LifecycleState::New | LifecycleState::Active | LifecycleState::Paused,
                    HostEvent::Destroy { unconditional },
                ) => Some(LifecycleAction::Destroy { unconditional }),
                _ => None,
            };
            if let Some(action) = action {
                return Some(self.begin(action));
            }
        }
        if std::mem::take(&mut self.restore_resume_pending) && self.state == LifecycleState::Paused
        {
            return Some(self.begin(LifecycleAction::Resume));
        }
        None
    }

    fn begin(&mut self, action: LifecycleAction) -> LifecycleAction {
        let previous_state = self.state;
        self.state = match action {
            LifecycleAction::Start | LifecycleAction::Resume => LifecycleState::Active,
            LifecycleAction::Pause => LifecycleState::Paused,
            LifecycleAction::Destroy { .. } => LifecycleState::Destroyed,
        };
        self.pending = Some(PendingAction {
            action,
            previous_state,
        });
        action
    }
}

fn event_capacity_error() -> EmuError {
    EmuError::new(
        Category::Api,
        "lifecycle-event-capacity",
        "MIDlet lifecycle event queue is full",
    )
}

/// Converts one AMS action to its matching VM callback.
#[must_use]
pub fn lifecycle_call(action: LifecycleAction) -> InstanceCall {
    match action {
        LifecycleAction::Start | LifecycleAction::Resume => InstanceCall {
            target: CallTarget::Instance,
            name: "__amsStartApp".to_owned(),
            descriptor: "()V".to_owned(),
            arguments: Vec::new(),
        },
        LifecycleAction::Pause => InstanceCall {
            target: CallTarget::Instance,
            name: "__amsPauseApp".to_owned(),
            descriptor: "()V".to_owned(),
            arguments: Vec::new(),
        },
        LifecycleAction::Destroy { unconditional } => InstanceCall {
            target: CallTarget::Instance,
            name: "__amsDestroyApp".to_owned(),
            descriptor: "(Z)V".to_owned(),
            arguments: vec![Value::Int(i32::from(unconditional))],
        },
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/midp/lifecycle.rs"]
mod tests;
