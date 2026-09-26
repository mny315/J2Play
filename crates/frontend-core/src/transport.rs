//! Bounded commands, authoritative events and urgent worker cancellation.

use crate::{
    AttemptId, HostDecision, InputEvent, PointerPhase, SessionCommand, SessionCommandKind,
    SessionEvent, SessionEventKind, SessionId,
};
use diagnostics::{Category, EmuError};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

mod audio;
mod vibration;

pub use audio::{AudioMailbox, AudioStats, AudioStatus};
pub use vibration::{VibrationEffect, VibrationMailbox};

const PRIORITY_COMMAND_CAPACITY: usize = 64;
const NORMAL_COMMAND_CAPACITY: usize = 256;
const HOST_REQUEST_CAPACITY: usize = 4;
const DIAGNOSTIC_CAPACITY: usize = 64;
const ATTEMPT_STOP_CANCEL_DEADLINE: Duration = Duration::from_secs(2);

#[derive(Default)]
struct CommandState {
    priority: VecDeque<SessionCommand>,
    normal: VecDeque<SessionCommand>,
}

#[derive(Default)]
pub(crate) struct UrgentState {
    stop_attempt: AtomicU64,
    stop_requested_at: Mutex<Option<(u64, Instant)>>,
    shutdown: AtomicBool,
    force_cancel: AtomicBool,
}

impl UrgentState {
    pub(crate) fn note(&self, command: &SessionCommand) {
        if matches!(command.kind, SessionCommandKind::Stop) {
            self.note_stop_at(command.attempt_id, Instant::now());
        }
    }

    fn note_stop_at(&self, attempt_id: AttemptId, requested_at: Instant) {
        let raw_attempt = attempt_id.get();
        if self.stop_attempt.fetch_max(raw_attempt, Ordering::AcqRel) > raw_attempt {
            return;
        }
        if let Ok(mut state) = self.stop_requested_at.lock()
            && self.stop_requested(attempt_id)
            && state
                .as_ref()
                .is_none_or(|(current_attempt, _)| *current_attempt != raw_attempt)
        {
            *state = Some((raw_attempt, requested_at));
        }
    }

    pub(crate) fn stop_requested(&self, attempt_id: AttemptId) -> bool {
        self.stop_attempt.load(Ordering::Acquire) == attempt_id.get()
    }

    pub(crate) fn stop_cancellation_due(&self, attempt_id: AttemptId) -> bool {
        // Polling a running VM needs only the atomic flag. Read the clock
        // once a stop has actually requested its cancellation deadline.
        self.stop_requested(attempt_id) && self.stop_cancellation_due_at(attempt_id, Instant::now())
    }

    fn stop_cancellation_due_at(&self, attempt_id: AttemptId, now: Instant) -> bool {
        if !self.stop_requested(attempt_id) {
            return false;
        }
        match self.stop_requested_at.lock() {
            Ok(state) => state
                .as_ref()
                .is_some_and(|(current_attempt, requested_at)| {
                    *current_attempt == attempt_id.get()
                        && now.saturating_duration_since(*requested_at)
                            >= ATTEMPT_STOP_CANCEL_DEADLINE
                }),
            Err(_) => true,
        }
    }

    pub(crate) fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    pub(crate) fn shutdown_requested(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub(crate) fn force_cancel(&self) {
        self.force_cancel.store(true, Ordering::Release);
    }

    pub(crate) fn force_cancelled(&self) -> bool {
        self.force_cancel.load(Ordering::Acquire)
    }
}

/// Bounded two-lane command transport. Safety transitions are always drained
/// before ordinary input, while pointer motion may coalesce under pressure.
#[derive(Clone, Default)]
pub(crate) struct CommandQueue {
    inner: Arc<(Mutex<CommandState>, Condvar)>,
    urgent: Arc<UrgentState>,
}

impl CommandQueue {
    pub(crate) fn urgent(&self) -> &Arc<UrgentState> {
        &self.urgent
    }

    pub(crate) fn push(&self, command: SessionCommand) -> Result<(), EmuError> {
        self.urgent.note(&command);
        let (lock, ready) = &*self.inner;
        let mut state = lock
            .lock()
            .map_err(|_| transport_error("command-lock", "command queue lock is poisoned"))?;
        if command.is_priority() {
            let discard_queued_input = matches!(
                command.kind,
                SessionCommandKind::ReleaseAllInput
                    | SessionCommandKind::Stop
                    | SessionCommandKind::SetPause { paused: true, .. }
            );
            let generation = (command.session_id, command.attempt_id);
            push_priority(&mut state.priority, command)?;
            if discard_queued_input {
                state.normal.retain(|queued| {
                    (queued.session_id, queued.attempt_id) != generation
                        || !matches!(queued.kind, SessionCommandKind::Input(_))
                });
            }
        } else if state.normal.len() < NORMAL_COMMAND_CAPACITY {
            state.normal.push_back(command);
        } else if !coalesce_normal(&mut state.normal, command) {
            return Err(transport_error(
                "command-queue-full",
                "emulator input queue is full",
            ));
        }
        ready.notify_all();
        Ok(())
    }

    pub(crate) fn request_shutdown(&self) {
        self.urgent.request_shutdown();
        self.inner.1.notify_all();
    }

    pub(crate) fn wait_next(&self, timeout: Duration) -> Result<Option<SessionCommand>, EmuError> {
        let (lock, ready) = &*self.inner;
        let mut state = lock
            .lock()
            .map_err(|_| transport_error("command-lock", "command queue lock is poisoned"))?;
        if state.priority.is_empty() && state.normal.is_empty() && !self.urgent.shutdown_requested()
        {
            state = ready
                .wait_timeout(state, timeout)
                .map_err(|_| transport_error("command-lock", "command queue lock is poisoned"))?
                .0;
        }
        Ok(state
            .priority
            .pop_front()
            .or_else(|| state.normal.pop_front()))
    }

    pub(crate) fn pop_priority(&self) -> Result<Option<SessionCommand>, EmuError> {
        Ok(self.lock()?.priority.pop_front())
    }

    pub(crate) fn pop_normal(&self) -> Result<Option<SessionCommand>, EmuError> {
        Ok(self.lock()?.normal.pop_front())
    }

    pub(crate) fn take_decision(
        &self,
        session_id: SessionId,
        attempt_id: AttemptId,
        request_id: u64,
    ) -> Result<Option<HostDecision>, EmuError> {
        let mut state = self.lock()?;
        let Some(index) = state.priority.iter().position(|command| {
            command.session_id == session_id
                && command.attempt_id == attempt_id
                && matches!(
                    command.kind,
                    SessionCommandKind::ResolveHostRequest {
                        request_id: candidate,
                        ..
                    } if candidate == request_id
                )
        }) else {
            return Ok(None);
        };
        let Some(command) = state.priority.remove(index) else {
            return Ok(None);
        };
        match command.kind {
            SessionCommandKind::ResolveHostRequest { decision, .. } => Ok(Some(decision)),
            _ => Ok(None),
        }
    }

    pub(crate) fn wait_for_change(&self, timeout: Duration) -> Result<(), EmuError> {
        let (lock, ready) = &*self.inner;
        let state = lock
            .lock()
            .map_err(|_| transport_error("command-lock", "command queue lock is poisoned"))?;
        drop(
            ready
                .wait_timeout(state, timeout)
                .map_err(|_| transport_error("command-lock", "command queue lock is poisoned"))?,
        );
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, CommandState>, EmuError> {
        self.inner
            .0
            .lock()
            .map_err(|_| transport_error("command-lock", "command queue lock is poisoned"))
    }
}

fn push_priority(
    queue: &mut VecDeque<SessionCommand>,
    command: SessionCommand,
) -> Result<(), EmuError> {
    // Only adjacent duplicates are redundant. Collapsing a pause/resume pair
    // would lose the pause's input-release and lifecycle side effects.
    if let Some(queued) = queue.back_mut().filter(|queued| {
        queued.session_id == command.session_id
            && queued.attempt_id == command.attempt_id
            && same_coalescible_priority(&queued.kind, &command.kind)
    }) {
        *queued = command;
        return Ok(());
    }
    if queue.len() >= PRIORITY_COMMAND_CAPACITY {
        return Err(transport_error(
            "priority-command-queue-full",
            "emulator safety-command queue is full",
        ));
    }
    queue.push_back(command);
    Ok(())
}

fn same_coalescible_priority(left: &SessionCommandKind, right: &SessionCommandKind) -> bool {
    matches!(
        (left, right),
        (
            SessionCommandKind::SetPause { reason: left, paused: left_paused },
            SessionCommandKind::SetPause { reason: right, paused: right_paused }
        ) if left == right && left_paused == right_paused
    ) || matches!(
        (left, right),
        (SessionCommandKind::Stop, SessionCommandKind::Stop)
            | (
                SessionCommandKind::ReleaseAllInput,
                SessionCommandKind::ReleaseAllInput
            )
    )
}

fn coalesce_normal(queue: &mut VecDeque<SessionCommand>, command: SessionCommand) -> bool {
    let SessionCommandKind::Input(InputEvent::Pointer(pointer)) = &command.kind else {
        return false;
    };
    if pointer.phase != PointerPhase::Dragged {
        return false;
    }
    // Never move a drag across another input, especially a release followed
    // by a new gesture that reuses the same touch identifier.
    let Some(queued) = queue.back_mut().filter(|queued| {
        queued.session_id == command.session_id
            && queued.attempt_id == command.attempt_id
            && matches!(
                &queued.kind,
                SessionCommandKind::Input(InputEvent::Pointer(candidate))
                    if candidate.touch_id == pointer.touch_id
                        && candidate.phase == PointerPhase::Dragged
            )
    }) else {
        return false;
    };
    *queued = command;
    true
}

#[derive(Default)]
struct EventState {
    launch_prepared: Option<SessionEvent>,
    checkpoint_failure: Option<SessionEvent>,
    platform_cancelled: Option<SessionEvent>,
    state_change: Option<SessionEvent>,
    terminal: Option<SessionEvent>,
    managed_heap: Option<SessionEvent>,
    frame_available: Option<SessionEvent>,
    text_input: Option<SessionEvent>,
    host_requests: VecDeque<SessionEvent>,
    diagnostics: VecDeque<SessionEvent>,
}

type EventWaker = Arc<dyn Fn() + Send + Sync + 'static>;

/// Bounded event mailbox with dedicated authoritative slots. Terminal errors,
/// recovery decisions, and state transitions cannot be evicted by diagnostics.
#[derive(Clone, Default)]
pub(crate) struct EventMailbox {
    inner: Arc<Mutex<EventState>>,
    waker: Arc<OnceLock<EventWaker>>,
}

impl EventMailbox {
    pub(crate) fn set_waker(&self, waker: EventWaker) -> Result<(), EmuError> {
        self.waker.set(waker).map_err(|_| {
            transport_error(
                "event-waker-bound",
                "runtime event wake callback is already bound",
            )
        })
    }

    pub(crate) fn publish(&self, event: SessionEvent) -> Result<(), EmuError> {
        let mut state = self.lock()?;
        match &event.kind {
            SessionEventKind::LaunchPrepared(_) => state.launch_prepared = Some(event),
            SessionEventKind::CheckpointSaveFailed { .. } => state.checkpoint_failure = Some(event),
            SessionEventKind::PlatformRequestCancelled => state.platform_cancelled = Some(event),
            SessionEventKind::Started | SessionEventKind::Stopped => {
                state.state_change = Some(event);
            }
            SessionEventKind::TerminalError { .. } => state.terminal = Some(event),
            SessionEventKind::ManagedHeapLimit { .. } => state.managed_heap = Some(event),
            SessionEventKind::FrameAvailable => state.frame_available = Some(event),
            SessionEventKind::TextInputActive { .. } => state.text_input = Some(event),
            SessionEventKind::HostRequest(_) | SessionEventKind::HostRequestClosed { .. } => {
                if state.host_requests.len() >= HOST_REQUEST_CAPACITY {
                    return Err(transport_error(
                        "host-request-capacity",
                        "too many outstanding host requests",
                    ));
                }
                state.host_requests.push_back(event);
            }
            SessionEventKind::Diagnostic {
                code,
                detail,
                repeated,
            } => {
                if let Some(SessionEvent {
                    kind:
                        SessionEventKind::Diagnostic {
                            repeated: current, ..
                        },
                    ..
                }) = state.diagnostics.iter_mut().find(|candidate| {
                    candidate.session_id == event.session_id
                        && candidate.attempt_id == event.attempt_id
                        && matches!(
                            &candidate.kind,
                            SessionEventKind::Diagnostic {
                                code: candidate_code,
                                detail: candidate_detail,
                                ..
                            } if candidate_code == code && candidate_detail == detail
                        )
                }) {
                    *current = current.saturating_add(*repeated);
                } else {
                    if state.diagnostics.len() == DIAGNOSTIC_CAPACITY {
                        state.diagnostics.pop_front();
                    }
                    state.diagnostics.push_back(event);
                }
            }
        }
        drop(state);
        self.wake();
        Ok(())
    }

    pub(crate) fn wake(&self) {
        if let Some(waker) = self.waker.get() {
            waker();
        }
    }

    pub(crate) fn take_all(&self) -> Result<Vec<SessionEvent>, EmuError> {
        let mut state = self.lock()?;
        // Publication bounds every slot and queue. Drain this snapshot in
        // priority order without imposing a second, unreachable capacity.
        let mut output: Vec<_> = [
            state.launch_prepared.take(),
            // Stop clears the UI's active session. Deliver its save result
            // first, independently of the bounded diagnostic queue.
            state.checkpoint_failure.take(),
            // Cancel the deferred URL before Stop could launch it. Repeated
            // empty platformRequest calls occupy only this one bounded slot.
            state.platform_cancelled.take(),
            state.state_change.take(),
            state.managed_heap.take(),
            state.terminal.take(),
            state.text_input.take(),
            state.frame_available.take(),
        ]
        .into_iter()
        .flatten()
        .collect();
        output.extend(state.host_requests.drain(..));
        output.extend(state.diagnostics.drain(..));
        Ok(output)
    }

    pub(crate) fn clear_generation(&self) -> Result<(), EmuError> {
        *self.lock()? = EventState::default();
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, EventState>, EmuError> {
        self.inner
            .lock()
            .map_err(|_| transport_error("event-lock", "event mailbox lock is poisoned"))
    }
}

fn transport_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Platform, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-core/transport/mod.rs"]
mod tests;
