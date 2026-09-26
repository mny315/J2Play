use super::{
    Arc, AudioMailbox, Category, CommandQueue, EmuError, EventMailbox, FORCED_SHUTDOWN_DEADLINE,
    GRACEFUL_SHUTDOWN_DEADLINE, JoinHandle, LatestFrameMailbox, LatestTelemetryMailbox,
    LibraryRepository, Ordering, Receiver, RecvTimeoutError, SessionCommand, SessionCommandKind,
    SessionEvent, SessionEventKind, VM_HOST_STACK_BYTES, VibrationMailbox, WORKER_IDLE_POLL,
    bounded_detail, mpsc, recovery_candidates, run_attempt, thread,
};
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use vm::is_managed_heap_limit_error;

/// Latest platform lifecycle state, readable without the UI/render thread.
/// Android updates this from its app-private bridge monitor, so a long-running
/// guest callback can park even after the native surface has disappeared.
#[derive(Clone, Debug, Default)]
pub struct PlatformLifecycleSignal {
    state: Arc<PlatformLifecycleState>,
}

#[derive(Debug, Default)]
pub(super) struct PlatformLifecycleState {
    revision: AtomicU64,
    acknowledged: AtomicU64,
}

const SUSPENDED: u64 = 1;
const DESTROYED: u64 = 2;
const LIFECYCLE_FLAGS: u64 = SUSPENDED | DESTROYED;

impl PlatformLifecycleSignal {
    #[must_use]
    pub fn initially_suspended() -> Self {
        let signal = Self::default();
        signal.set_suspended(true);
        signal
    }

    pub fn set_suspended(&self, suspended: bool) {
        self.update(if suspended { SUSPENDED } else { 0 });
    }

    pub fn mark_destroyed(&self) {
        self.update(SUSPENDED | DESTROYED);
    }

    fn update(&self, flags: u64) {
        let _ = self
            .state
            .revision
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                if old & DESTROYED != 0 || old & LIFECYCLE_FLAGS == flags {
                    None
                } else {
                    Some((old.wrapping_add(LIFECYCLE_FLAGS + 1) & !LIFECYCLE_FLAGS) | flags)
                }
            });
    }

    #[must_use]
    pub fn suspended(&self) -> bool {
        self.snapshot() & SUSPENDED != 0
    }

    #[must_use]
    pub fn destroyed(&self) -> bool {
        self.snapshot() & DESTROYED != 0
    }

    /// True after the worker has applied the current suspension's input and
    /// output barrier, or observed it while idle. A previous cycle cannot
    /// acknowledge a later one. Shells use this before releasing a sleep delay.
    #[must_use]
    pub fn suspension_acknowledged(&self) -> bool {
        let revision = self.snapshot();
        revision & SUSPENDED != 0 && self.state.acknowledged.load(Ordering::Acquire) == revision
    }

    pub(super) fn snapshot(&self) -> u64 {
        self.state.revision.load(Ordering::Acquire)
    }

    pub(super) fn acknowledge_suspension(&self, revision: u64) {
        if revision & SUSPENDED != 0 {
            self.state.acknowledged.store(revision, Ordering::Release);
        }
    }
}

/// One persistent VM owner. Launch attempts are serialized on a dedicated
/// 16 MiB stack and communicate only through bounded frontend-neutral mailboxes.
pub struct RuntimeWorker {
    commands: CommandQueue,
    events: EventMailbox,
    frames: LatestFrameMailbox,
    telemetry: LatestTelemetryMailbox,
    audio: AudioMailbox,
    vibration: VibrationMailbox,
    completion: Receiver<Result<(), EmuError>>,
    completed: Option<Result<(), EmuError>>,
    completion_reported: bool,
    thread: Option<JoinHandle<()>>,
}

impl RuntimeWorker {
    /// Starts the sole emulator worker for one application process.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the host thread cannot be created.
    pub fn spawn(repository: LibraryRepository) -> Result<Self, EmuError> {
        Self::spawn_with_lifecycle(repository, PlatformLifecycleSignal::default())
    }

    /// Starts the worker with a platform-owned lifecycle signal which remains
    /// available when the UI surface cannot run an egui logic pass.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the host thread cannot be created.
    pub fn spawn_with_lifecycle(
        repository: LibraryRepository,
        lifecycle: PlatformLifecycleSignal,
    ) -> Result<Self, EmuError> {
        let commands = CommandQueue::default();
        let events = EventMailbox::default();
        let frames = LatestFrameMailbox::default();
        let telemetry = LatestTelemetryMailbox::default();
        let audio = AudioMailbox::default();
        let vibration = VibrationMailbox::default();
        let (completion_tx, completion) = mpsc::sync_channel(1);
        let worker_commands = commands.clone();
        let worker_events = events.clone();
        let worker_frames = frames.clone();
        let worker_telemetry = telemetry.clone();
        let worker_audio = audio.clone();
        let worker_vibration = vibration.clone();
        let worker_lifecycle = lifecycle;
        let thread = thread::Builder::new()
            .name("j2play-vm".to_owned())
            .stack_size(VM_HOST_STACK_BYTES)
            .spawn(move || {
                let result = vm::with_vm_host_stack(|| {
                    worker_loop(
                        &repository,
                        &worker_commands,
                        &worker_events,
                        &worker_frames,
                        &worker_telemetry,
                        &worker_audio,
                        &worker_vibration,
                        &worker_lifecycle,
                    )
                });
                let result = result.and(clear_outputs(
                    &worker_audio,
                    &worker_telemetry,
                    &worker_frames,
                    &worker_vibration,
                ));
                let _ = completion_tx.send(result);
                worker_events.wake();
            })
            .map_err(|error| {
                EmuError::with_source(
                    Category::Platform,
                    "vm-thread-create",
                    "cannot create emulator worker thread",
                    error,
                )
            })?;
        Ok(Self {
            commands,
            events,
            frames,
            telemetry,
            audio,
            vibration,
            completion,
            completed: None,
            completion_reported: false,
            thread: Some(thread),
        })
    }

    /// Enqueues one generation-tagged command.
    ///
    /// # Errors
    /// Returns an explicit bounded-transport diagnostic when the queue is full.
    pub fn submit(&self, command: SessionCommand) -> Result<(), EmuError> {
        if self.completed.is_some()
            || self.commands.urgent().shutdown_requested()
            || self.thread.as_ref().is_none_or(JoinHandle::is_finished)
        {
            return Err(runtime_error(
                "vm-worker-stopped",
                "The emulator worker is unavailable. Restart the app.",
            ));
        }
        let stop = matches!(command.kind, SessionCommandKind::Stop);
        let (session_id, attempt_id) = (command.session_id, command.attempt_id);
        // Stop must reach the worker even when muting a failed output mailbox
        // reports an error. Attempt both outputs, including on queue failure.
        let submitted = self.commands.push(command);
        if stop {
            submitted.and(
                self.audio
                    .set_suspended(session_id, attempt_id, true)
                    .and(self.vibration.set_suspended(session_id, attempt_id, true)),
            )
        } else {
            submitted
        }
    }

    /// Drains the current bounded event snapshot.
    ///
    /// # Errors
    /// Returns a platform diagnostic if the mailbox lock is poisoned or the
    /// worker has stopped unexpectedly. A worker failure is reported once.
    pub fn poll_events(&mut self) -> Result<Vec<SessionEvent>, EmuError> {
        if self.thread.is_none() {
            return Ok(Vec::new());
        }
        if self.completed.is_none() {
            self.completed = match self.completion.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(runtime_error(
                    "vm-thread-panic",
                    "Emulator worker terminated unexpectedly",
                ))),
            };
        }
        if let Some(result) = &self.completed {
            if !self.completion_reported && !self.commands.urgent().shutdown_requested() {
                self.completion_reported = true;
                let detail = result.as_ref().err().map_or_else(
                    || "Emulator worker stopped unexpectedly".to_owned(),
                    ToString::to_string,
                );
                return Err(runtime_error(
                    "vm-worker-stopped",
                    bounded_detail(&detail, 4096),
                ));
            }
            return Ok(Vec::new());
        }
        self.events
            .take_all()
            .inspect_err(|_| self.commands.request_shutdown())
    }

    /// Binds the event-loop wake callback used when the worker publishes a
    /// frame or another session event.
    ///
    /// # Errors
    /// Returns a transport diagnostic if a callback was already bound.
    pub fn set_event_waker(
        &self,
        waker: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), EmuError> {
        self.events.set_waker(Arc::new(waker))
    }

    #[must_use]
    pub const fn frames(&self) -> &LatestFrameMailbox {
        &self.frames
    }

    #[must_use]
    pub const fn telemetry(&self) -> &LatestTelemetryMailbox {
        &self.telemetry
    }

    #[must_use]
    pub const fn audio(&self) -> &AudioMailbox {
        &self.audio
    }

    #[must_use]
    pub const fn vibration(&self) -> &VibrationMailbox {
        &self.vibration
    }

    /// Requests graceful lifecycle teardown, then enables cooperative forced
    /// cancellation only if the measured grace period expires.
    ///
    /// # Errors
    /// Returns a deadline, worker, or panic diagnostic. A timed-out worker is
    /// never reported as successfully shut down.
    pub fn shutdown(&mut self) -> Result<(), EmuError> {
        if self.thread.is_none() {
            return Ok(());
        }
        let started = Instant::now();
        let grace = started + GRACEFUL_SHUTDOWN_DEADLINE;
        let deadline = grace + FORCED_SHUTDOWN_DEADLINE;
        self.commands.request_shutdown();
        if let Some(result) = self.completed.take() {
            return self.finish_thread(result, deadline);
        }
        match self
            .completion
            .recv_timeout(grace.saturating_duration_since(Instant::now()))
        {
            Ok(result) => self.finish_thread(result, deadline),
            Err(RecvTimeoutError::Disconnected) => self.finish_thread(Ok(()), deadline),
            Err(RecvTimeoutError::Timeout) => {
                self.commands.urgent().force_cancel();
                self.commands.request_shutdown();
                match self
                    .completion
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                {
                    Ok(result) => self.finish_thread(result, deadline),
                    Err(RecvTimeoutError::Disconnected) => self.finish_thread(Ok(()), deadline),
                    Err(RecvTimeoutError::Timeout) => Err(shutdown_deadline_error()),
                }
            }
        }
    }

    fn finish_thread(
        &mut self,
        result: Result<(), EmuError>,
        deadline: Instant,
    ) -> Result<(), EmuError> {
        // Completion precedes the final platform wake callback and thread-local
        // destructors. Receiving it does not make an unbounded join safe.
        while self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
        {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.completed = Some(result);
                return Err(shutdown_deadline_error());
            }
            thread::sleep(Duration::from_millis(1).min(remaining));
        }
        let join = self
            .thread
            .take()
            .ok_or_else(|| runtime_error("vm-thread-state", "emulator worker handle is missing"))?;
        join.join().map_err(|_| {
            runtime_error("vm-thread-panic", "emulator worker terminated unexpectedly")
        })?;
        result
    }
}

fn shutdown_deadline_error() -> EmuError {
    runtime_error(
        "vm-shutdown-deadline",
        "emulator worker did not stop within the 3 second shutdown deadline",
    )
}

impl Drop for RuntimeWorker {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn worker_loop(
    repository: &LibraryRepository,
    commands: &CommandQueue,
    events: &EventMailbox,
    frames: &LatestFrameMailbox,
    telemetry: &LatestTelemetryMailbox,
    audio: &AudioMailbox,
    vibration: &VibrationMailbox,
    lifecycle: &PlatformLifecycleSignal,
) -> Result<(), EmuError> {
    let urgent = commands.urgent();
    while !urgent.shutdown_requested() && !lifecycle.destroyed() {
        lifecycle.acknowledge_suspension(lifecycle.snapshot());
        let Some(command) = commands.wait_next(WORKER_IDLE_POLL)? else {
            continue;
        };
        let (entry_id, orientation) = match &command.kind {
            SessionCommandKind::Start {
                entry_id,
                orientation,
            } => (entry_id, *orientation),
            _ => continue,
        };
        // A launch queued during suspension must not initialize guest code or
        // reactivate outputs after the idle worker acknowledged the barrier.
        while lifecycle.suspended()
            && !lifecycle.destroyed()
            && !urgent.shutdown_requested()
            && !urgent.stop_requested(command.attempt_id)
        {
            lifecycle.acknowledge_suspension(lifecycle.snapshot());
            commands.wait_for_change(WORKER_IDLE_POLL)?;
        }
        if lifecycle.destroyed() || urgent.shutdown_requested() {
            break;
        }
        if urgent.stop_requested(command.attempt_id) {
            events.publish(SessionEvent {
                session_id: command.session_id,
                attempt_id: command.attempt_id,
                kind: SessionEventKind::Stopped,
            })?;
            continue;
        }
        events.clear_generation()?;
        frames.activate(command.session_id, command.attempt_id)?;
        telemetry.activate(command.session_id, command.attempt_id)?;
        audio.activate(command.session_id, command.attempt_id)?;
        vibration.activate(command.session_id, command.attempt_id)?;
        let result = run_attempt(
            repository,
            entry_id,
            orientation,
            command.session_id,
            command.attempt_id,
            commands.clone(),
            events.clone(),
            frames.clone(),
            telemetry.clone(),
            audio.clone(),
            vibration.clone(),
            PlatformLifecycleSignal::clone(lifecycle),
        );
        clear_outputs(audio, telemetry, frames, vibration)?;
        if urgent.shutdown_requested() {
            break;
        }
        publish_attempt_result(
            repository,
            entry_id,
            orientation,
            &command,
            events,
            urgent,
            result,
        )?;
    }
    Ok(())
}

fn publish_attempt_result(
    repository: &LibraryRepository,
    entry_id: &str,
    orientation: Option<launch::CanvasOrientation>,
    command: &SessionCommand,
    events: &EventMailbox,
    urgent: &super::UrgentState,
    result: Result<(), EmuError>,
) -> Result<(), EmuError> {
    let event = |kind| SessionEvent {
        session_id: command.session_id,
        attempt_id: command.attempt_id,
        kind,
    };
    match result {
        Ok(()) => events.publish(event(SessionEventKind::Stopped)),
        Err(error) if is_managed_heap_limit_error(&error) => {
            let cancelled = || super::attempt::attempt_stop_requested(urgent, command.attempt_id);
            let candidates = recovery_candidates(repository, entry_id, orientation, cancelled);
            // Stop can arrive while private bytes are being inspected.
            // Do not turn that cancellation into a new recovery prompt.
            if cancelled() {
                return events.publish(event(SessionEventKind::Stopped));
            }
            let candidates = match candidates {
                Ok(candidates) => candidates,
                Err(recovery_error) => {
                    events.publish(event(SessionEventKind::Diagnostic {
                        code: recovery_error.code().to_owned(),
                        detail: bounded_detail(recovery_error.message(), 2 * 1024),
                        repeated: 1,
                    }))?;
                    Vec::new()
                }
            };
            events.publish(event(SessionEventKind::ManagedHeapLimit {
                detail: bounded_detail(error.message(), 8 * 1024),
                candidate_profile_ids: candidates,
            }))
        }
        Err(error) => events.publish(SessionEvent::terminal_error(
            command.session_id,
            command.attempt_id,
            "The game stopped because the emulator reported an error.",
            &format!(
                "{}[{}]: {}",
                error.category().as_str(),
                error.code(),
                error.message()
            ),
        )),
    }
}

fn clear_outputs(
    audio: &AudioMailbox,
    telemetry: &LatestTelemetryMailbox,
    frames: &LatestFrameMailbox,
    vibration: &VibrationMailbox,
) -> Result<(), EmuError> {
    // Every output gets its cleanup even if another mailbox was poisoned.
    audio
        .clear()
        .and(telemetry.clear())
        .and(frames.clear())
        .and(vibration.stop_and_deactivate())
}

pub(super) fn runtime_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Platform, code, message)
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-core/runtime/worker/mod.rs"]
mod tests;
