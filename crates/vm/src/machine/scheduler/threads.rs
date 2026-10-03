//! Thread construction, cooperative scheduling, sleep/join and shutdown.

use super::{
    CallOutcome, EmuError, Handle, HashSet, HeapValue, MAIN_THREAD_ID, Machine, Method,
    NativeResume, ThreadState, Value, WORKER_QUANTUM, display_key, heap_error,
    optional_reference_argument, reference_argument, suspended_native_call, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn pump_host_media(&mut self) -> Result<(), EmuError> {
        let now_millis = self.native_context.monotonic_millis();
        if now_millis <= self.scheduler.media_pump_checkpoint_millis {
            return Ok(());
        }
        self.native_context
            .mmapi_pump(now_millis.saturating_mul(1_000))?;
        self.scheduler.media_pump_checkpoint_millis = now_millis;
        Ok(())
    }

    pub(in crate::machine) fn thread_constructor(
        &mut self,
        args: &[Value],
        descriptor: &str,
    ) -> Result<CallOutcome, EmuError> {
        let thread = reference_argument(args, 0)?;
        let (target, name_index) = match descriptor {
            "()V" => (None, None),
            "(Ljava/lang/Runnable;)V" => (optional_reference_argument(args, 1)?, None),
            "(Ljava/lang/String;)V" => (None, Some(1)),
            "(Ljava/lang/Runnable;Ljava/lang/String;)V" => {
                (optional_reference_argument(args, 1)?, Some(2))
            }
            _ => return Err(type_error()),
        };
        let name = if let Some(index) = name_index {
            let Some(name) = optional_reference_argument(args, index)? else {
                return self.thread_exception(
                    "java/lang/NullPointerException",
                    Some("thread name is null"),
                );
            };
            name
        } else {
            let sequence = self.scheduler.next_thread_name;
            let name = self.allocate_dynamic_string(&format!("Thread-{sequence}"), &[], args)?;
            self.scheduler.next_thread_name = self
                .scheduler
                .next_thread_name
                .checked_add(1)
                .ok_or_else(|| vm_error("thread-limit", "thread name sequence overflow"))?;
            name
        };
        self.set_thread_fields(thread, target, name)?;
        Ok(CallOutcome::Return(None))
    }

    pub(in crate::machine) fn set_thread_fields(
        &mut self,
        thread: Handle,
        target: Option<Handle>,
        name: Handle,
    ) -> Result<(), EmuError> {
        self.heap
            .managed
            .set_field(thread, "java/lang/Thread.priority:I", HeapValue::Int(5))
            .map_err(heap_error)?;
        self.heap
            .managed
            .set_field(
                thread,
                "java/lang/Thread.target:Ljava/lang/Runnable;",
                HeapValue::Reference(target),
            )
            .map_err(heap_error)?;
        self.heap
            .managed
            .set_field(
                thread,
                "java/lang/Thread.name:Ljava/lang/String;",
                HeapValue::Reference(Some(name)),
            )
            .map_err(heap_error)
    }

    pub(in crate::machine) fn allocate_runtime_thread(
        &mut self,
        name: &str,
    ) -> Result<Handle, EmuError> {
        let thread = self.allocate_native_instance("java/lang/Thread", &[])?;
        self.heap.temporary_roots.push(thread);
        let name = self.allocate_dynamic_string(name, &[], &[]);
        self.heap.temporary_roots.pop();
        let name = name?;
        self.set_thread_fields(thread, None, name)?;
        Ok(thread)
    }

    pub(in crate::machine) fn thread_id(&self, handle: Handle) -> u64 {
        if self.scheduler.main_thread == Some(handle) {
            MAIN_THREAD_ID
        } else {
            handle.to_raw()
        }
    }

    pub(in crate::machine) fn current_thread_handle(&mut self) -> Result<Handle, EmuError> {
        if self.scheduler.current_thread != MAIN_THREAD_ID {
            return Ok(Handle::from_raw(self.scheduler.current_thread));
        }
        if let Some(handle) = self.scheduler.main_thread {
            return Ok(handle);
        }
        let handle = self.allocate_runtime_thread("main")?;
        self.scheduler.main_thread = Some(handle);
        Ok(handle)
    }

    pub(in crate::machine) fn timer_thread(&mut self, timer: Handle) -> Result<Handle, EmuError> {
        if let Some(thread) = self.scheduler.timer_threads.get(&timer) {
            return Ok(*thread);
        }
        let thread = self.allocate_runtime_thread(&format!("Timer-{}", timer.to_raw()))?;
        self.scheduler.timer_threads.insert(timer, thread);
        Ok(thread)
    }

    pub(in crate::machine) fn take_interrupt(&mut self, thread: u64) -> bool {
        self.scheduler.interrupted_threads.remove(&thread)
    }

    pub(in crate::machine) fn resume_thread_sleep(&mut self) -> Result<CallOutcome, EmuError> {
        // A sleeping worker resumes a new CPU segment. Host time spent parked
        // must not satisfy the instruction budget for the code after sleep().
        self.begin_instruction_pacing_segment();
        let thread = Handle::from_raw(self.scheduler.current_thread);
        self.scheduler.sleeping_threads.remove(&thread);
        if self.take_interrupt(self.scheduler.current_thread) {
            self.thread_exception("java/lang/InterruptedException", Some("sleep interrupted"))
        } else {
            Ok(CallOutcome::Return(None))
        }
    }

    pub(in crate::machine) fn resume_thread_join(
        &mut self,
        method: &Method,
        target: Handle,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let waiter = self.current_thread_handle()?;
        self.remove_join_waiter(target, waiter);
        if self.scheduler.current_thread == MAIN_THREAD_ID {
            self.scheduler.notified_threads.remove(&MAIN_THREAD_ID);
        }
        if self.take_interrupt(self.scheduler.current_thread) {
            return self
                .thread_exception("java/lang/InterruptedException", Some("join interrupted"));
        }
        self.join_thread(method, target, depth)
    }

    fn park_thread_join(
        &mut self,
        method: &Method,
        target: Handle,
    ) -> Result<CallOutcome, EmuError> {
        let waiter = self.current_thread_handle()?;
        // Creating the main Thread can collect an otherwise unreferenced
        // Timer and finish the target before this waiter is registered.
        if !self.scheduler.thread_is_alive(target) {
            return Ok(CallOutcome::Return(None));
        }
        if self.is_scheduler_worker() {
            self.scheduler
                .thread_states
                .insert(waiter, ThreadState::Sleeping);
        }
        self.scheduler
            .join_waiters
            .entry(target)
            .or_default()
            .push(waiter);
        Ok(suspended_native_call(method, NativeResume::Join { target }))
    }

    pub(in crate::machine) fn join_thread(
        &mut self,
        method: &Method,
        target: Handle,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        if self.thread_id(target) == self.scheduler.current_thread && !self.is_scheduler_worker() {
            return Err(vm_error("deadlock", "thread attempted to join itself"));
        }
        while self.scheduler.thread_is_alive(target) {
            if self.take_interrupt(self.scheduler.current_thread) {
                return self
                    .thread_exception("java/lang/InterruptedException", Some("join interrupted"));
            }
            if self.native_context.execution_cancelled() {
                return Err(vm_error("execution-cancelled", "thread join cancelled"));
            }
            // Workers and lifecycle callbacks leave the host stack while
            // waiting so frontend polling and unrelated Java threads continue.
            if self.is_scheduler_worker()
                || (self.scheduler.current_thread == MAIN_THREAD_ID
                    && self.scheduler.host_driver_active
                    && !self.scheduler.dispatching_display
                    && self.scheduler.suspended_driver_call.is_none())
            {
                return self.park_thread_join(method, target);
            }
            let progress = self.run_one_thread(depth + 1)?;
            if !self.scheduler.thread_is_alive(target) {
                break;
            }
            if !self.scheduler.dispatching_timer && !self.owns_monitor() {
                self.run_due_timer_tasks(depth + 1, &[], &[Value::Reference(Some(target))])?;
                if !self.scheduler.thread_is_alive(target) {
                    break;
                }
                let now = self
                    .native_context
                    .wall_clock_millis()
                    .saturating_add(self.scheduler.virtual_wall_millis);
                if let Some(deadline) = self
                    .scheduler
                    .scheduled_tasks
                    .iter()
                    .map(|task| task.deadline)
                    .min()
                    && deadline > now
                {
                    let remaining = deadline.saturating_sub(now);
                    self.pace_scheduler_wait(remaining, progress)?;
                    continue;
                }
            }
            if progress {
                continue;
            }
            return Err(vm_error("deadlock", "join has no runnable thread"));
        }
        Ok(CallOutcome::Return(None))
    }

    pub(in crate::machine) fn advance_virtual_time(&mut self, millis: i64) -> Result<(), EmuError> {
        self.scheduler.virtual_wall_millis = self
            .scheduler
            .virtual_wall_millis
            .checked_add(millis)
            .ok_or_else(|| vm_error("time-overflow", "wall clock overflow"))?;
        self.scheduler.virtual_monotonic_millis = self
            .scheduler
            .virtual_monotonic_millis
            .checked_add(millis)
            .ok_or_else(|| vm_error("time-overflow", "monotonic clock overflow"))?;
        Ok(())
    }

    pub(in crate::machine) fn pacing_monotonic_millis(&self) -> i64 {
        if self.native_context.realtime_pacing() {
            self.native_context
                .monotonic_millis()
                .saturating_add(self.scheduler.virtual_monotonic_millis)
        } else {
            self.scheduler.virtual_monotonic_millis
        }
    }

    pub(in crate::machine) fn pace_or_advance_time(&mut self, millis: i64) -> Result<(), EmuError> {
        if millis <= 0 {
            return Ok(());
        }
        if self.native_context.realtime_pacing() {
            let millis = u64::try_from(millis)
                .map_err(|_| vm_error("time-overflow", "real-time wait overflow"))?;
            self.native_context.pace_millis(millis)
        } else {
            self.advance_virtual_time(millis)
        }
    }

    pub(in crate::machine) fn pace_scheduler_wait(
        &mut self,
        remaining: i64,
        has_runnable: bool,
    ) -> Result<(), EmuError> {
        // Host time advances while workers run; virtual time needs a bounded
        // step even when another worker never sleeps. Idle real-time waits
        // retain short host-facing slices; idle virtual waits can jump ahead.
        let step = match (self.native_context.realtime_pacing(), has_runnable) {
            (true, true) => 0,
            (true, false) | (false, true) => remaining.min(1),
            (false, false) => remaining,
        };
        self.pace_or_advance_time(step)
    }

    pub(in crate::machine) fn run_workers_during_realtime_sleep(
        &mut self,
        deadline: i64,
        depth: usize,
    ) -> Result<(), EmuError> {
        self.pace_pending_instructions()?;
        while self.pacing_monotonic_millis() < deadline
            && (!self.scheduler.runnable_threads.is_empty()
                || !self.scheduler.sleeping_threads.is_empty())
        {
            if self
                .scheduler
                .interrupted_threads
                .contains(&self.scheduler.current_thread)
                || self.native_context.execution_suspended()
            {
                break;
            }
            if self.native_context.execution_cancelled() {
                return Err(vm_error(
                    "execution-cancelled",
                    "guest execution was cancelled by the host",
                ));
            }
            self.run_one_thread(depth)?;
            self.pace_pending_instructions()?;
        }
        Ok(())
    }

    pub(in crate::machine) fn begin_instruction_pacing_segment(&mut self) {
        self.scheduler.instruction_pacing_checkpoint = self.execution.instructions;
        self.scheduler.instruction_pacing_started_millis = self.native_context.monotonic_millis();
    }

    pub(in crate::machine) fn pace_pending_instructions(&mut self) -> Result<(), EmuError> {
        if !self.native_context.realtime_pacing() {
            self.begin_instruction_pacing_segment();
            return Ok(());
        }
        let Some(rate) = self
            .native_context
            .realtime_interpreter_instructions_per_second()
        else {
            self.begin_instruction_pacing_segment();
            return Ok(());
        };
        if rate == 0 {
            return Err(vm_error(
                "invalid-instruction-rate",
                "real-time interpreter instruction rate must be positive",
            ));
        }
        let pending = self
            .execution
            .instructions
            .saturating_sub(self.scheduler.instruction_pacing_checkpoint);
        let target_millis = pending.saturating_mul(1_000).saturating_add(rate - 1) / rate;
        let elapsed_millis = u64::try_from(
            self.native_context
                .monotonic_millis()
                .saturating_sub(self.scheduler.instruction_pacing_started_millis),
        )
        .unwrap_or(0);
        if target_millis > elapsed_millis {
            self.native_context
                .pace_millis(target_millis - elapsed_millis)?;
        }
        self.begin_instruction_pacing_segment();
        Ok(())
    }

    pub(in crate::machine) fn terminate_application_threads(&mut self) {
        let mut application_thread_ids = HashSet::new();
        for (thread, state) in &mut self.scheduler.thread_states {
            *state = ThreadState::Terminated;
            application_thread_ids.insert(thread.to_raw());
        }
        if self.scheduler.suspended_driver_call.take().is_some() {
            application_thread_ids.insert(MAIN_THREAD_ID);
        }
        for thread in self.scheduler.timer_threads.values() {
            application_thread_ids.insert(thread.to_raw());
            self.scheduler
                .thread_states
                .insert(*thread, ThreadState::Terminated);
        }
        self.scheduler.scheduled_tasks.clear();
        self.scheduler.timer_poll_requested = false;
        self.scheduler.timer_poll_countdown = 0;
        self.scheduler.timer_threads.clear();
        self.scheduler.runnable_threads.clear();
        self.scheduler.sleeping_threads.clear();
        self.scheduler.join_waiters.clear();
        self.classes.initialization_waiters.clear();
        self.scheduler.inline_monitor_entry_waits.clear();
        self.scheduler.monitor_entry_waiters.clear();
        self.classes.initializing.clear();
        self.scheduler.thread_continuations.clear();
        self.scheduler
            .interrupted_threads
            .retain(|thread| !application_thread_ids.contains(thread));
        self.scheduler
            .monitors
            .retain(|_, monitor| !application_thread_ids.contains(&monitor.owner));
        for waiters in self.scheduler.monitor_waiters.values_mut() {
            waiters.retain(|thread| !application_thread_ids.contains(thread));
        }
        self.scheduler
            .monitor_waiters
            .retain(|_, waiters| !waiters.is_empty());
        self.scheduler
            .notified_threads
            .retain(|thread| !application_thread_ids.contains(thread));
    }

    pub(in crate::machine) fn wake_sleeping_threads(&mut self) {
        if self.scheduler.sleeping_threads.is_empty() {
            return;
        }
        let now = self.pacing_monotonic_millis();
        let scheduler = &mut self.scheduler;
        scheduler.sleeping_threads.retain(|thread, deadline| {
            if *deadline > now {
                return true;
            }
            if let Some(state) = scheduler.thread_states.get_mut(thread)
                && *state == ThreadState::Sleeping
            {
                *state = ThreadState::Runnable;
                scheduler.runnable_threads.push_back(*thread);
            }
            false
        });
    }

    pub(in crate::machine) fn next_sleep_deadline(&self) -> Option<i64> {
        self.scheduler.sleeping_threads.values().copied().min()
    }

    pub(in crate::machine) fn remove_join_waiter(&mut self, target: Handle, waiter: Handle) {
        if let Some(waiters) = self.scheduler.join_waiters.get_mut(&target) {
            waiters.retain(|candidate| *candidate != waiter);
            if waiters.is_empty() {
                self.scheduler.join_waiters.remove(&target);
            }
        }
    }

    pub(in crate::machine) fn run_one_thread(&mut self, depth: usize) -> Result<bool, EmuError> {
        self.wake_sleeping_threads();
        if let Some(deadline) = self.next_sleep_deadline() {
            let remaining = deadline.saturating_sub(self.pacing_monotonic_millis());
            if remaining > 0 {
                let has_runnable = !self.scheduler.runnable_threads.is_empty();
                self.pace_scheduler_wait(remaining, has_runnable)?;
                self.wake_sleeping_threads();
            }
        }
        let Some(thread) = self.scheduler.runnable_threads.pop_front() else {
            // Sleeping workers are still live and can make progress once their
            // deadline arrives. Returning true prevents join/monitor waiters
            // from misclassifying this temporary state as a deadlock. When no
            // guest thread exists at all, pace the live host by 1 ms instead
            // of spinning the host event loop millions of times per second.
            if self.scheduler.sleeping_threads.is_empty() && self.native_context.realtime_pacing() {
                self.pace_or_advance_time(1)?;
            }
            return Ok(!self.scheduler.sleeping_threads.is_empty());
        };
        if self.scheduler.thread_states.get(&thread) != Some(&ThreadState::Runnable) {
            return Ok(true);
        }
        if depth == 1 {
            // Host polling and a preceding guest sleep are outside this
            // worker's CPU segment and must not create instruction credit.
            self.begin_instruction_pacing_segment();
        }
        self.scheduler
            .thread_states
            .insert(thread, ThreadState::Running);
        self.scheduler.quantum_remaining = WORKER_QUANTUM;
        self.scheduler.suspend_requested = false;
        let previous = self.scheduler.current_thread;
        self.scheduler.current_thread = thread.to_raw();
        let continuation = self.scheduler.thread_continuations.remove(&thread);
        let outcome = (|| {
            if let Some(continuation) = continuation {
                return self.resume_suspended_call(continuation, depth);
            }
            let class = self.object_class(thread)?;
            let key = self.resolve_virtual(&class, "run", "()V")?;
            let method = self
                .program
                .methods
                .get(&key)
                .ok_or_else(|| vm_error("method-not-found", display_key(&key)))?;
            self.call(method, [Value::Reference(Some(thread))], depth)
        })();
        self.scheduler.current_thread = previous;
        self.publish_vm_telemetry();
        let outcome = outcome?;
        if depth == 1 {
            self.pace_pending_instructions()?;
        }
        match outcome {
            CallOutcome::Suspend(continuation) => {
                self.scheduler
                    .thread_continuations
                    .insert(thread, continuation);
                if self.scheduler.thread_states.get(&thread) == Some(&ThreadState::Running) {
                    self.scheduler
                        .thread_states
                        .insert(thread, ThreadState::Runnable);
                    self.scheduler.runnable_threads.push_back(thread);
                }
                Ok(true)
            }
            CallOutcome::Return(Some(_)) => Err(type_error()),
            // An uncaught exception terminates this Java thread, not the VM or
            // unrelated threads. CLDC has no uncaught-exception handler API.
            CallOutcome::Throw(handle) => {
                self.report_uncaught_thread_exception(thread, handle)?;
                self.scheduler.finish_thread(thread);
                Ok(true)
            }
            CallOutcome::Return(None) => {
                self.scheduler.finish_thread(thread);
                Ok(true)
            }
        }
    }
}
