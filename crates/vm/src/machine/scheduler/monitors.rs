use super::{
    CallOutcome, EmuError, Handle, MAIN_THREAD_ID, Machine, Method, Monitor, NativeResume,
    ThreadState, suspended_native_call, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn enter_monitor(
        &mut self,
        object: Handle,
        depth: usize,
    ) -> Result<bool, EmuError> {
        loop {
            if self.is_scheduler_worker()
                && self.scheduler.inline_monitor_entry_waits.contains(&object)
                && !self.scheduler.monitors.contains_key(&object)
            {
                self.park_monitor_entry_waiter(object);
                return Ok(false);
            }
            match self.scheduler.monitors.get_mut(&object) {
                None => {
                    self.scheduler.monitors.insert(
                        object,
                        Monitor {
                            owner: self.scheduler.current_thread,
                            depth: 1,
                        },
                    );
                    if self.is_scheduler_worker() {
                        self.remove_monitor_entry_waiter(
                            object,
                            Handle::from_raw(self.scheduler.current_thread),
                        );
                    }
                    return Ok(true);
                }
                Some(monitor) if monitor.owner == self.scheduler.current_thread => {
                    monitor.depth = monitor
                        .depth
                        .checked_add(1)
                        .ok_or_else(|| vm_error("monitor-limit", "monitor recursion overflow"))?;
                    return Ok(true);
                }
                Some(_) => {
                    if self.is_scheduler_worker() {
                        self.park_monitor_entry_waiter(object);
                        return Ok(false);
                    }
                    let inserted = self.scheduler.inline_monitor_entry_waits.insert(object);
                    let progressed = self.run_one_thread(depth + 1);
                    if inserted {
                        self.scheduler.inline_monitor_entry_waits.remove(&object);
                    }
                    if !progressed? {
                        return Err(vm_error(
                            "deadlock",
                            "contended monitor has no runnable owner",
                        ));
                    }
                }
            }
        }
    }

    pub(in crate::machine) fn park_monitor_entry_waiter(&mut self, object: Handle) {
        let waiter = Handle::from_raw(self.scheduler.current_thread);
        self.scheduler
            .thread_states
            .insert(waiter, ThreadState::Sleeping);
        let waiters = self
            .scheduler
            .monitor_entry_waiters
            .entry(object)
            .or_default();
        if !waiters.contains(&waiter) {
            waiters.push_back(waiter);
        }
    }

    pub(in crate::machine) fn remove_monitor_entry_waiter(
        &mut self,
        object: Handle,
        waiter: Handle,
    ) {
        if let Some(waiters) = self.scheduler.monitor_entry_waiters.get_mut(&object) {
            waiters.retain(|candidate| *candidate != waiter);
            if waiters.is_empty() {
                self.scheduler.monitor_entry_waiters.remove(&object);
            }
        }
    }

    pub(in crate::machine) fn wake_monitor_entry_waiter(&mut self, object: Handle) {
        let mut remove_queue = false;
        if let Some(waiters) = self.scheduler.monitor_entry_waiters.get_mut(&object) {
            while let Some(waiter) = waiters.pop_front() {
                match self.scheduler.thread_states.get_mut(&waiter) {
                    Some(state @ ThreadState::Sleeping) => {
                        *state = ThreadState::Runnable;
                        self.scheduler.sleeping_threads.remove(&waiter);
                        self.scheduler.runnable_threads.push_back(waiter);
                        break;
                    }
                    Some(ThreadState::Runnable | ThreadState::Running) => break,
                    Some(ThreadState::New | ThreadState::Terminated) | None => {}
                }
            }
            remove_queue = waiters.is_empty();
        }
        if remove_queue {
            self.scheduler.monitor_entry_waiters.remove(&object);
        }
    }

    pub(in crate::machine) fn exit_monitor(&mut self, object: Handle) -> Result<(), EmuError> {
        let Some(monitor) = self.scheduler.monitors.get_mut(&object) else {
            return Err(vm_error("illegal-monitor-state", "monitor is not owned"));
        };
        if monitor.owner != self.scheduler.current_thread {
            return Err(vm_error(
                "illegal-monitor-state",
                "monitor is owned by another thread",
            ));
        }
        monitor.depth -= 1;
        if monitor.depth == 0 {
            self.scheduler.monitors.remove(&object);
            self.wake_monitor_entry_waiter(object);
        }
        Ok(())
    }

    pub(in crate::machine) fn owns_monitor(&self) -> bool {
        self.scheduler
            .monitors
            .values()
            .any(|monitor| monitor.owner == self.scheduler.current_thread)
    }

    pub(in crate::machine) fn wait_on_monitor(
        &mut self,
        method: &Method,
        object: Handle,
        timeout: i64,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        if timeout < 0 {
            return self.thread_exception(
                "java/lang/IllegalArgumentException",
                Some("negative wait timeout"),
            );
        }
        let Some(monitor) = self.scheduler.monitors.get(&object).copied() else {
            return self.thread_exception(
                "java/lang/IllegalMonitorStateException",
                Some("wait without owning monitor"),
            );
        };
        if monitor.owner != self.scheduler.current_thread {
            return self.thread_exception(
                "java/lang/IllegalMonitorStateException",
                Some("wait without owning monitor"),
            );
        }
        let owner = self.scheduler.current_thread;
        if self.take_interrupt(owner) {
            return self
                .thread_exception("java/lang/InterruptedException", Some("wait interrupted"));
        }
        let deadline = if timeout > 0 {
            Some(
                self.pacing_monotonic_millis()
                    .checked_add(timeout)
                    .ok_or_else(|| vm_error("time-overflow", "wait deadline overflow"))?,
            )
        } else {
            None
        };
        self.scheduler.monitors.remove(&object);
        // wait releases every recursive acquisition, so an existing monitor
        // entrant must become runnable just as on the final monitorexit.
        self.wake_monitor_entry_waiter(object);
        self.scheduler
            .monitor_waiters
            .entry(object)
            .or_default()
            .push_back(owner);
        if self.is_scheduler_worker() {
            let thread = Handle::from_raw(owner);
            self.scheduler
                .thread_states
                .insert(thread, ThreadState::Sleeping);
            if let Some(deadline) = deadline {
                self.scheduler.sleeping_threads.insert(thread, deadline);
            }
            return Ok(suspended_native_call(
                method,
                NativeResume::MonitorWait {
                    object,
                    monitor_depth: monitor.depth,
                },
            ));
        }
        loop {
            if self.take_interrupt(owner) {
                self.remove_waiter(object, owner);
                if !self.enter_monitor(object, depth + 1)? {
                    return Err(vm_error(
                        "invalid-monitor-suspension",
                        "main wait unexpectedly suspended while reacquiring its monitor",
                    ));
                }
                if let Some(current) = self.scheduler.monitors.get_mut(&object) {
                    current.depth = monitor.depth;
                }
                return self
                    .thread_exception("java/lang/InterruptedException", Some("wait interrupted"));
            }
            if self.scheduler.notified_threads.remove(&owner) {
                break;
            }
            if let Some(deadline) = deadline
                && self.pacing_monotonic_millis() >= deadline
            {
                self.remove_waiter(object, owner);
                break;
            }
            let scheduler_progress = self.run_one_thread(depth + 1)?;
            if self.scheduler.notified_threads.remove(&owner) {
                break;
            }
            if self.scheduler.current_thread == MAIN_THREAD_ID && !self.owns_monitor() {
                // Display.setCurrent/repaint work is deferred to the LCDUI
                // event turn. A lifecycle callback may wait for paint() to
                // notify it before that callback can return to the host
                // driver, so service the independent event turn while the
                // target monitor is released.
                self.run_display_event_turn(depth + 1)?;
                if self.scheduler.notified_threads.remove(&owner) {
                    break;
                }
            }
            if scheduler_progress {
                if let Some(deadline) = deadline {
                    self.pace_scheduler_wait(
                        deadline.saturating_sub(self.pacing_monotonic_millis()),
                        true,
                    )?;
                }
                continue;
            }
            if let Some(deadline) = deadline {
                let remaining = deadline.saturating_sub(self.pacing_monotonic_millis());
                self.pace_or_advance_time(remaining)?;
                self.run_due_timer_tasks(depth + 1, &[], &[])?;
                self.remove_waiter(object, owner);
                break;
            }
            if owner == MAIN_THREAD_ID
                && self.scheduler.host_driver_active
                && self.scheduler.suspended_driver_call.is_none()
            {
                return Ok(suspended_native_call(
                    method,
                    NativeResume::MonitorWait {
                        object,
                        monitor_depth: monitor.depth,
                    },
                ));
            }
            return Err(vm_error(
                "deadlock",
                "indefinite wait has no runnable notifier",
            ));
        }
        if !self.enter_monitor(object, depth + 1)? {
            return Err(vm_error(
                "invalid-monitor-suspension",
                "main wait unexpectedly suspended while reacquiring its monitor",
            ));
        }
        if let Some(current) = self.scheduler.monitors.get_mut(&object) {
            current.depth = monitor.depth;
        }
        Ok(CallOutcome::Return(None))
    }

    pub(in crate::machine) fn resume_monitor_wait(
        &mut self,
        method: &Method,
        object: Handle,
        monitor_depth: usize,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let owner = self.scheduler.current_thread;
        self.remove_waiter(object, owner);
        self.scheduler.notified_threads.remove(&owner);
        self.scheduler
            .sleeping_threads
            .remove(&Handle::from_raw(owner));
        if !self.enter_monitor(object, depth + 1)? {
            return Ok(suspended_native_call(
                method,
                NativeResume::MonitorWait {
                    object,
                    monitor_depth,
                },
            ));
        }
        if let Some(current) = self.scheduler.monitors.get_mut(&object) {
            current.depth = monitor_depth;
        }
        if self.take_interrupt(owner) {
            self.thread_exception("java/lang/InterruptedException", Some("wait interrupted"))
        } else {
            Ok(CallOutcome::Return(None))
        }
    }

    pub(in crate::machine) fn remove_waiter(&mut self, object: Handle, owner: u64) {
        if let Some(waiters) = self.scheduler.monitor_waiters.get_mut(&object) {
            waiters.retain(|candidate| *candidate != owner);
            if waiters.is_empty() {
                self.scheduler.monitor_waiters.remove(&object);
            }
        }
    }

    pub(in crate::machine) fn notify_monitor(
        &mut self,
        object: Handle,
        all: bool,
    ) -> Result<CallOutcome, EmuError> {
        if self
            .scheduler
            .monitors
            .get(&object)
            .map(|monitor| monitor.owner)
            != Some(self.scheduler.current_thread)
        {
            return self.thread_exception(
                "java/lang/IllegalMonitorStateException",
                Some("notify without owning monitor"),
            );
        }
        if let Some(waiters) = self.scheduler.monitor_waiters.get_mut(&object) {
            let count = if all {
                waiters.len()
            } else {
                waiters.len().min(1)
            };
            for thread_id in waiters.drain(..count) {
                self.scheduler.notified_threads.insert(thread_id);
                let thread = Handle::from_raw(thread_id);
                if let Some(state) = self.scheduler.thread_states.get_mut(&thread)
                    && *state == ThreadState::Sleeping
                {
                    *state = ThreadState::Runnable;
                    self.scheduler.sleeping_threads.remove(&thread);
                    self.scheduler.runnable_threads.push_back(thread);
                }
            }
            if waiters.is_empty() {
                self.scheduler.monitor_waiters.remove(&object);
            }
        }
        Ok(CallOutcome::Return(None))
    }
}
