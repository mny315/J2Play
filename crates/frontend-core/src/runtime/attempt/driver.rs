//! Attempt command loop, independent pause sources and lifecycle callbacks.

use super::{
    Arc, AttemptDriver, AttemptId, AudioMailbox, Cell, CommandQueue, Duration, EmuError, Instant,
    PauseReason, PauseSources, PlatformLifecycleSignal, Rc, RefCell, SessionCommand,
    SessionCommandKind, SessionId, VecDeque, VibrationMailbox, WORKER_IDLE_POLL, checkpoint,
    retain_input_release_steps,
};
use runtime::idle_call;

impl AttemptDriver {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        session_id: SessionId,
        attempt_id: AttemptId,
        commands: CommandQueue,
        ams: Rc<RefCell<midp::Ams>>,
        input_map: platform::DeviceInputMap,
        pointer: platform::PointerInputPolicy,
        active_canvas_dimensions: Rc<Cell<(u32, u32)>>,
        audio: AudioMailbox,
        vibration: VibrationMailbox,
        realtime_pacing: Rc<Cell<bool>>,
        lifecycle: PlatformLifecycleSignal,
    ) -> Self {
        let urgent = Arc::clone(commands.urgent());
        Self {
            session_id,
            attempt_id,
            commands,
            urgent,
            ams,
            input: platform::InputState::new(input_map),
            pointer,
            active_canvas_dimensions,
            audio,
            vibration,
            pending: VecDeque::new(),
            pending_text: VecDeque::new(),
            composing: false,
            paused: PauseSources::default(),
            close_queued: false,
            captured_pointer: None,
            pointer_position: (0, 0),
            last_idle: Instant::now(),
            realtime_pacing,
            lifecycle,
            checkpoints: checkpoint::DriverCheckpoints::default(),
        }
    }

    pub(super) fn next_step(&mut self, turn: vm::DriverTurn) -> Result<vm::DriverStep, EmuError> {
        self.drain_priority()?;
        // Observe Stop once and save before queuing destroyApp. A Stop arriving
        // after this check waits for the next turn instead of bypassing saving.
        if self.urgent.stop_requested(self.attempt_id) {
            if let Some(step) = self.checkpoint_step()? {
                return Ok(step);
            }
            self.request_close()?;
        }
        self.sync_platform_lifecycle()?;
        if self.urgent.shutdown_requested() {
            self.request_close()?;
        }
        if let Some(step) = self.pending.pop_front() {
            return Ok(step);
        }
        if let Some(step) = self.pending_text.pop_front() {
            return Ok(step);
        }
        let lifecycle_action = self.ams.borrow_mut().next_action();
        if let Some(action) = lifecycle_action {
            self.queue_lifecycle_action(action)?;
            return Ok(self.pending.pop_front().unwrap_or(vm::DriverStep::Stop));
        }
        if self.ams.borrow().state() == midp::LifecycleState::Destroyed {
            return Ok(vm::DriverStep::Stop);
        }
        let discard_input = self.is_paused() || self.close_queued;
        while let Some(command) = self.commands.pop_normal()? {
            if !self.matches(&command) {
                continue;
            }
            match command.kind {
                SessionCommandKind::Input(event) if !discard_input => {
                    self.queue_input(event)?;
                    if let Some(step) = self
                        .pending
                        .pop_front()
                        .or_else(|| self.pending_text.pop_front())
                    {
                        return Ok(step);
                    }
                }
                SessionCommandKind::SetFastForward { enabled } => {
                    self.realtime_pacing.set(!enabled);
                }
                SessionCommandKind::Input(_)
                | SessionCommandKind::Start { .. }
                | SessionCommandKind::SetPause { .. }
                | SessionCommandKind::Stop
                | SessionCommandKind::ReleaseAllInput
                | SessionCommandKind::ResolveHostRequest { .. } => {}
            }
        }
        if discard_input {
            self.commands.wait_for_change(WORKER_IDLE_POLL)?;
            return Ok(vm::DriverStep::Idle);
        }
        if turn == vm::DriverTurn::HostPoll {
            return Ok(vm::DriverStep::Tick);
        }
        if self.last_idle.elapsed() >= Duration::from_millis(16) {
            self.last_idle = Instant::now();
            return Ok(vm::DriverStep::Call(idle_call()));
        }
        Ok(vm::DriverStep::Tick)
    }

    pub(super) fn sync_platform_lifecycle(&mut self) -> Result<(), EmuError> {
        let revision = self.lifecycle.snapshot();
        let suspended = self.lifecycle.suspended();
        if suspended != self.paused.platform {
            let was_paused = self.is_paused();
            self.paused.platform = suspended;
            self.apply_pause_transition(was_paused)?;
        }
        if self.lifecycle.destroyed() {
            self.request_close()?;
        }
        self.lifecycle.acknowledge_suspension(revision);
        Ok(())
    }

    pub(super) fn drain_priority(&mut self) -> Result<(), EmuError> {
        while let Some(command) = self.commands.pop_priority()? {
            if !self.matches(&command) {
                continue;
            }
            match command.kind {
                SessionCommandKind::SetPause { reason, paused } => {
                    self.set_pause(reason, paused)?;
                }
                SessionCommandKind::ReleaseAllInput => self.queue_release_all(),
                SessionCommandKind::Input(event) => self.queue_input(event)?,
                // The urgent flag routes every Stop through checkpoint_step.
                SessionCommandKind::Stop
                | SessionCommandKind::Start { .. }
                | SessionCommandKind::SetFastForward { .. }
                | SessionCommandKind::ResolveHostRequest { .. } => {}
            }
        }
        Ok(())
    }

    pub(super) fn matches(&self, command: &SessionCommand) -> bool {
        command.session_id == self.session_id && command.attempt_id == self.attempt_id
    }

    pub(super) fn set_pause(&mut self, reason: PauseReason, paused: bool) -> Result<(), EmuError> {
        let was_paused = self.is_paused();
        match reason {
            PauseReason::User => self.paused.user = paused,
            PauseReason::Lifecycle => self.paused.lifecycle = paused,
        }
        self.apply_pause_transition(was_paused)
    }

    pub(super) fn is_paused(&self) -> bool {
        self.paused.user || self.paused.lifecycle || self.paused.platform
    }

    pub(super) fn apply_pause_transition(&mut self, was_paused: bool) -> Result<(), EmuError> {
        if self.close_queued {
            return Ok(());
        }
        let stopping =
            self.urgent.stop_requested(self.attempt_id) || self.urgent.shutdown_requested();
        let is_paused = self.is_paused();
        if !was_paused && is_paused {
            // A frontend can enqueue ReleaseAllInput immediately before the
            // pause command. Preserve those already-materialized release
            // callbacks while discarding ordinary guest work; otherwise the
            // InputState has already forgotten the held inputs and the pause
            // transition can no longer synthesize their releases.
            retain_input_release_steps(&mut self.pending);
            self.pending_text.clear();
            self.queue_release_all();
            self.stop_transient_effects()?;
            if !stopping {
                self.ams.borrow_mut().host_event(midp::HostEvent::Pause)?;
            }
        } else if was_paused && !is_paused && !stopping {
            self.ams.borrow_mut().host_event(midp::HostEvent::Resume)?;
            self.resume_transient_effects()?;
        }
        Ok(())
    }

    pub(super) fn request_close(&mut self) -> Result<(), EmuError> {
        if !self.close_queued {
            self.checkpoints.host_close.set(true);
            retain_input_release_steps(&mut self.pending);
            self.queue_release_all();
            self.stop_transient_effects()?;
            self.close_queued = true;
            self.ams.borrow_mut().request_close();
        }
        Ok(())
    }

    pub(super) fn queue_lifecycle_action(
        &mut self,
        action: midp::LifecycleAction,
    ) -> Result<(), EmuError> {
        if matches!(
            action,
            midp::LifecycleAction::Pause | midp::LifecycleAction::Destroy { .. }
        ) {
            self.queue_release_all();
            self.stop_transient_effects()?;
        } else if matches!(
            action,
            midp::LifecycleAction::Start | midp::LifecycleAction::Resume
        ) {
            self.resume_transient_effects()?;
        }
        let call = midp::lifecycle_call(action);
        self.pending
            .push_back(if matches!(action, midp::LifecycleAction::Destroy { .. }) {
                vm::DriverStep::CallAfterApplicationShutdown(call)
            } else {
                vm::DriverStep::Call(call)
            });
        if matches!(
            action,
            midp::LifecycleAction::Start | midp::LifecycleAction::Resume
        ) {
            self.pending.push_back(vm::DriverStep::Call(idle_call()));
        }
        Ok(())
    }

    pub(super) fn stop_transient_effects(&self) -> Result<(), EmuError> {
        self.audio
            .set_suspended(self.session_id, self.attempt_id, true)
            .and(
                self.vibration
                    .set_suspended(self.session_id, self.attempt_id, true),
            )
    }

    pub(super) fn resume_transient_effects(&self) -> Result<(), EmuError> {
        if self.is_paused() || self.close_queued {
            return Ok(());
        }
        self.audio
            .set_suspended(self.session_id, self.attempt_id, false)?;
        self.vibration
            .set_suspended(self.session_id, self.attempt_id, false)?;
        Ok(())
    }
}
