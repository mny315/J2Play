//! Native Java thread and monitor operations dispatched by the VM.

use super::{
    CallOutcome, EmuError, Handle, HashSet, HeapValue, MAIN_THREAD_ID, Machine, Method,
    NativeResume, ThreadState, Value, heap_error, reference_argument, suspended_native_call,
    type_error, vm_error,
};
use crate::machine::{int_argument, long_argument};

impl Machine<'_, '_> {
    pub(in crate::machine) fn is_scheduler_worker(&self) -> bool {
        self.scheduler.current_thread != MAIN_THREAD_ID
            // Timer.cancel() may remove the Timer -> thread association from
            // timer_threads while that TimerTask is still on the Java stack.
            // The active callback remains synchronously dispatched until it
            // returns and must not acquire a worker continuation meanwhile.
            && !self.scheduler.dispatching_timer
            && !self
                .scheduler.timer_threads
                .values()
                .any(|thread| thread.to_raw() == self.scheduler.current_thread)
    }

    pub(in crate::machine) fn invoke_thread_native(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
    ) -> Result<Option<CallOutcome>, EmuError> {
        let signature = (
            method.key.class.as_str(),
            method.key.name.as_str(),
            method.key.descriptor.as_str(),
        );
        let outcome = match signature {
            (
                "java/lang/Thread",
                "<init>",
                "()V"
                | "(Ljava/lang/Runnable;)V"
                | "(Ljava/lang/String;)V"
                | "(Ljava/lang/Runnable;Ljava/lang/String;)V",
            ) => self.thread_constructor(args, &method.key.descriptor)?,
            ("java/lang/Thread", "start", "()V") => {
                let thread = reference_argument(args, 0)?;
                match self
                    .scheduler
                    .thread_states
                    .get(&thread)
                    .copied()
                    .unwrap_or(ThreadState::New)
                {
                    ThreadState::New if !self.scheduler.thread_is_alive(thread) => {
                        if self
                            .scheduler
                            .thread_states
                            .values()
                            .filter(|state| state.is_alive())
                            .count()
                            >= self.limits.max_threads
                        {
                            return Err(vm_error(
                                "thread-limit",
                                format!("thread limit {} exceeded", self.limits.max_threads),
                            ));
                        }
                        self.scheduler
                            .thread_states
                            .insert(thread, ThreadState::Runnable);
                        self.scheduler.runnable_threads.push_back(thread);
                        CallOutcome::Return(None)
                    }
                    _ => self.thread_exception(
                        "java/lang/IllegalThreadStateException",
                        Some("thread already started"),
                    )?,
                }
            }
            ("java/lang/Thread", "isAlive", "()Z") => {
                let thread = reference_argument(args, 0)?;
                let alive = self.scheduler.thread_is_alive(thread);
                CallOutcome::Return(Some(Value::Int(i32::from(alive))))
            }
            ("java/lang/Thread", "join", "()V") => {
                let thread = reference_argument(args, 0)?;
                self.join_thread(method, thread, depth)?
            }
            ("java/lang/Thread", "interrupt", "()V") => {
                let thread = reference_argument(args, 0)?;
                let alive = self.scheduler.thread_is_alive(thread);
                if alive {
                    self.scheduler
                        .interrupted_threads
                        .insert(self.thread_id(thread));
                    if self.scheduler.thread_states.get(&thread) == Some(&ThreadState::Sleeping) {
                        self.scheduler.sleeping_threads.remove(&thread);
                        self.scheduler
                            .thread_states
                            .insert(thread, ThreadState::Runnable);
                        self.scheduler.runnable_threads.push_back(thread);
                    }
                }
                CallOutcome::Return(None)
            }
            ("java/lang/Thread", "isInterrupted", "()Z") => {
                let thread = reference_argument(args, 0)?;
                let alive = self.scheduler.thread_is_alive(thread);
                CallOutcome::Return(Some(Value::Int(i32::from(
                    alive
                        && self
                            .scheduler
                            .interrupted_threads
                            .contains(&self.thread_id(thread)),
                ))))
            }
            ("java/lang/Thread", "interrupted", "()Z") => CallOutcome::Return(Some(Value::Int(
                i32::from(self.take_interrupt(self.scheduler.current_thread)),
            ))),
            ("java/lang/Thread", "currentThread", "()Ljava/lang/Thread;") => {
                let handle = self.current_thread_handle()?;
                CallOutcome::Return(Some(Value::Reference(Some(handle))))
            }
            ("java/lang/Thread", "activeCount", "()I") => {
                let workers = self
                    .scheduler
                    .thread_states
                    .values()
                    .filter(|state| state.is_alive())
                    .count();
                let timers = self
                    .scheduler
                    .timer_threads
                    .keys()
                    .copied()
                    .chain(self.scheduler.scheduled_tasks.iter().map(|task| task.timer))
                    .collect::<HashSet<_>>()
                    .len();
                let active_timer_threads = self
                    .scheduler
                    .timer_threads
                    .values()
                    .filter(|thread| {
                        self.scheduler
                            .thread_states
                            .get(thread)
                            .is_some_and(|state| state.is_alive())
                    })
                    .count();
                let active = 1usize
                    .saturating_add(workers.saturating_sub(active_timer_threads))
                    .saturating_add(timers);
                let active = i32::try_from(active)
                    .map_err(|_| vm_error("thread-limit", "active thread count overflow"))?;
                CallOutcome::Return(Some(Value::Int(active)))
            }
            ("java/lang/Thread", "toString", "()Ljava/lang/String;") => {
                let thread = reference_argument(args, 0)?;
                let name = match self
                    .heap
                    .managed
                    .field(thread, "java/lang/Thread.name:Ljava/lang/String;")
                    .map_err(heap_error)?
                {
                    HeapValue::Reference(Some(name)) => self
                        .heap
                        .string_values
                        .get(&name)
                        .map(|units| String::from_utf16_lossy(units))
                        .unwrap_or_default(),
                    HeapValue::Reference(None) => String::new(),
                    _ => return Err(type_error()),
                };
                let HeapValue::Int(priority) = self
                    .heap
                    .managed
                    .field(thread, "java/lang/Thread.priority:I")
                    .map_err(heap_error)?
                else {
                    return Err(type_error());
                };
                let value =
                    self.allocate_dynamic_string(&format!("Thread[{name},{priority}]"), &[], args)?;
                CallOutcome::Return(Some(Value::Reference(Some(value))))
            }
            ("java/lang/Thread", "yield", "()V") => {
                let timer_thread = self.scheduler.dispatching_timer
                    || self
                        .scheduler
                        .timer_threads
                        .values()
                        .any(|thread| thread.to_raw() == self.scheduler.current_thread);
                if timer_thread {
                    // TimerTask callbacks are dispatched synchronously, so a
                    // yield inside one must not acquire another worker's
                    // continuation. It must still give the independent LCDUI
                    // event turn a chance to run: legacy MIDlets commonly use
                    // callSerially followed by a yield loop to wait for the
                    // serialized callback.
                    if !self.owns_monitor() {
                        match self.run_display_event_turn(depth + 1)? {
                            CallOutcome::Return(None) => {}
                            outcome => return Ok(Some(outcome)),
                        }
                    }
                } else if self.scheduler.current_thread == MAIN_THREAD_ID && !self.owns_monitor() {
                    self.run_one_thread(depth + 1)?;
                } else if self.scheduler.current_thread != MAIN_THREAD_ID {
                    self.scheduler.suspend_requested = true;
                }
                CallOutcome::Return(None)
            }
            ("java/lang/Thread", "sleep", "(J)V") => {
                let millis = long_argument(args, 0)?;
                if millis < 0 {
                    self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("negative sleep timeout"),
                    )?
                } else if self.take_interrupt(self.scheduler.current_thread) {
                    self.thread_exception(
                        "java/lang/InterruptedException",
                        Some("sleep interrupted"),
                    )?
                } else {
                    self.pace_pending_instructions()?;
                    let realtime_pacing = self.native_context.realtime_pacing();
                    let effective_millis = if realtime_pacing {
                        let requested = u64::try_from(millis).map_err(|_| {
                            vm_error("time-overflow", "sleep timeout does not fit u64")
                        })?;
                        i64::try_from(self.native_context.realtime_thread_sleep_millis(requested))
                            .map_err(|_| {
                            vm_error("time-overflow", "adjusted sleep timeout does not fit i64")
                        })?
                    } else {
                        millis
                    };
                    let deadline = self
                        .pacing_monotonic_millis()
                        .checked_add(effective_millis)
                        .ok_or_else(|| vm_error("time-overflow", "sleep deadline overflow"))?;
                    let scheduler_worker = self.is_scheduler_worker();
                    if realtime_pacing && scheduler_worker {
                        // Park only this guest worker. All Java workers share
                        // one host interpreter thread, so calling host sleep()
                        // here serializes unrelated game/audio threads.
                        let thread = Handle::from_raw(self.scheduler.current_thread);
                        self.scheduler
                            .thread_states
                            .insert(thread, ThreadState::Sleeping);
                        self.scheduler.sleeping_threads.insert(thread, deadline);
                        return Ok(Some(suspended_native_call(method, NativeResume::Sleep)));
                    }
                    if !self.scheduler.dispatching_timer
                        && !self.scheduler.dispatching_canvas_paint
                        && !self.owns_monitor()
                    {
                        // Deterministic/headless mode keeps one cooperative
                        // scheduling turn before virtual-time advancement.
                        // A Canvas.paint callback must finish before a worker
                        // can observe Canvas.painting again. Other LCDUI
                        // callbacks, including showNotify, may legitimately
                        // sleep while waiting for an application worker.
                        self.run_one_thread(depth + 1)?;
                        if self.scheduler.current_thread == MAIN_THREAD_ID
                            && !self.scheduler.dispatching_display
                        {
                            match self.run_display_event_turn(depth + 1)? {
                                CallOutcome::Return(None) => {}
                                outcome => return Ok(Some(outcome)),
                            }
                        }
                        if realtime_pacing {
                            // A synchronous caller still owns its Java stack,
                            // but its sleep must not idle the whole VM after
                            // just one loader quantum. Let unrelated workers
                            // run and wake within the original sleep deadline.
                            self.run_workers_during_realtime_sleep(deadline, depth + 1)?;
                        }
                    }
                    let remaining = deadline.saturating_sub(self.pacing_monotonic_millis());
                    if !self
                        .scheduler
                        .interrupted_threads
                        .contains(&self.scheduler.current_thread)
                    {
                        self.pace_or_advance_time(remaining)?;
                    }
                    self.begin_instruction_pacing_segment();
                    if !self.scheduler.dispatching_timer && !self.owns_monitor() {
                        self.run_due_timer_tasks(depth + 1, &[], args)?;
                    }
                    if self.take_interrupt(self.scheduler.current_thread) {
                        self.thread_exception(
                            "java/lang/InterruptedException",
                            Some("sleep interrupted"),
                        )?
                    } else {
                        // Scheduler-managed workers must save a continuation after
                        // sleep. Timer callbacks are executed synchronously by the
                        // timer dispatcher and have already paced above; marking a
                        // timer thread for suspension would incorrectly suspend an
                        // already active outer invocation.
                        if scheduler_worker {
                            self.scheduler.suspend_requested = true;
                        } else if realtime_pacing
                            && self.scheduler.current_thread == MAIN_THREAD_ID
                            && self.scheduler.host_driver_active
                            && self.scheduler.suspended_driver_call.is_none()
                        {
                            // A lifecycle callback can contain a permanent
                            // game loop which sleeps between state updates
                            // without presenting every iteration. Treat the
                            // completed sleep as the same safe host-poll
                            // boundary as a completed frame.
                            self.scheduler.driver_host_poll_yield = true;
                            if !self.scheduler.dispatching_display {
                                self.scheduler.suspend_requested = true;
                            }
                        }
                        CallOutcome::Return(None)
                    }
                }
            }
            ("java/lang/Object", "wait", "(J)V") => {
                let object = reference_argument(args, 0)?;
                let timeout = long_argument(args, 1)?;
                self.wait_on_monitor(method, object, timeout, depth + 1)?
            }
            ("java/lang/Object", "wait", "(JI)V") => {
                let object = reference_argument(args, 0)?;
                let timeout = long_argument(args, 1)?;
                let nanos = int_argument(args, 2)?;
                if timeout < 0 || !(0..=999_999).contains(&nanos) {
                    self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid wait timeout"),
                    )?
                } else {
                    let timeout = if nanos > 0 {
                        timeout.saturating_add(1)
                    } else {
                        timeout
                    };
                    self.wait_on_monitor(method, object, timeout, depth + 1)?
                }
            }
            ("java/lang/Object", "notify", "()V") => {
                let object = reference_argument(args, 0)?;
                self.notify_monitor(object, false)?
            }
            ("java/lang/Object", "notifyAll", "()V") => {
                let object = reference_argument(args, 0)?;
                self.notify_monitor(object, true)?
            }
            _ => return Ok(None),
        };
        Ok(Some(outcome))
    }
}
