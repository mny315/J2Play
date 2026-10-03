use super::super::diagnostic_helpers::{append_java_stack, diagnostic_message};
use super::{
    CallOutcome, EmuError, Handle, HeapValue, MAIN_THREAD_ID, Machine, MethodKey, ThreadState,
    Value, display_key, heap_error, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn run_due_timer_tasks(
        &mut self,
        depth: usize,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<(), EmuError> {
        const MAX_TIMER_FIRINGS_PER_POLL: usize = 1_024;

        // Timer callbacks run under a synthetic Java thread id. An initializer
        // can still be Running on an ancestor host stack while a nested worker
        // reaches this poll. A callback that actively uses that class cannot
        // resume such an owner, so leave the due task queued until every owner
        // is either suspended or runnable through the scheduler.
        self.scheduler.timer_poll_requested = false;
        self.scheduler.timer_poll_countdown = self.scheduler.timer_poll_instructions;
        if self.classes.initializing.values().any(|owner| {
            *owner == MAIN_THREAD_ID
                || *owner == self.scheduler.current_thread
                || self.scheduler.thread_states.get(&Handle::from_raw(*owner))
                    == Some(&ThreadState::Running)
        }) {
            return Ok(());
        }
        let now = self
            .native_context
            .wall_clock_millis()
            .saturating_add(self.scheduler.virtual_wall_millis);
        for _ in 0..MAX_TIMER_FIRINGS_PER_POLL {
            let Some(index) = self
                .scheduler
                .scheduled_tasks
                .iter()
                .enumerate()
                .filter(|(_, task)| task.deadline <= now)
                .min_by_key(|(_, task)| task.deadline)
                .map(|(index, _)| index)
            else {
                return Ok(());
            };
            let mut scheduled = self.scheduler.scheduled_tasks[index];
            let cancelled = matches!(
                self.heap.managed
                    .field(
                        scheduled.task,
                        "java/util/TimerTask.cancelled:Z",
                    )
                    .map_err(heap_error)?,
                HeapValue::Int(value) if value != 0
            );
            if cancelled {
                self.scheduler.scheduled_tasks.remove(index);
                continue;
            }
            self.heap
                .managed
                .set_field(
                    scheduled.task,
                    "java/util/TimerTask.executionTime:J",
                    HeapValue::Long(scheduled.deadline),
                )
                .map_err(heap_error)?;
            let class = self.object_class(scheduled.task)?;
            let key = self.resolve_virtual(&class, "run", "()V")?;
            let method = self
                .program
                .methods
                .get(&key)
                .ok_or_else(|| vm_error("method-not-found", display_key(&key)))?;
            // Thread creation and the callback may collect. Keep the active
            // task rooted even after it leaves the scheduler queue.
            self.publish_frame_roots(
                depth,
                locals,
                stack,
                &[
                    Value::Reference(Some(scheduled.timer)),
                    Value::Reference(Some(scheduled.task)),
                ],
            );
            let timer_thread = match self.timer_thread(scheduled.timer) {
                Ok(thread) => thread,
                Err(error) => {
                    self.heap.frame_roots.remove(depth);
                    return Err(error);
                }
            };
            self.scheduler.scheduled_tasks.remove(index);
            if scheduled.period <= 0 {
                // This one-shot execution has begun: cancel() can no longer
                // prevent it and must report false even inside run(). Wait
                // until thread creation succeeds so allocation failure leaves
                // the queued task retryable.
                self.heap
                    .managed
                    .set_field(
                        scheduled.task,
                        "java/util/TimerTask.cancelled:Z",
                        HeapValue::Int(1),
                    )
                    .map_err(heap_error)?;
            }
            self.scheduler.dispatching_timer = true;
            let previous_thread = self.scheduler.current_thread;
            let previous_quantum = self.scheduler.quantum_remaining;
            let previous_suspend_requested = self.scheduler.suspend_requested;
            self.scheduler.current_thread = timer_thread.to_raw();
            self.scheduler.quantum_remaining = u64::MAX;
            // A due timer can be dispatched from inside a worker's Thread.sleep().
            // That worker has already requested its own continuation suspend.
            // Never leak that scheduler flag into the synthetic Timer thread.
            self.scheduler.suspend_requested = false;
            self.scheduler
                .thread_states
                .insert(timer_thread, ThreadState::Running);
            // Fixed-delay periods are measured from this invocation's start.
            // Sampling after run() would add its duration to every interval.
            let next_deadline = if scheduled.period > 0 {
                let base = if scheduled.fixed_rate {
                    scheduled.deadline
                } else {
                    self.native_context
                        .wall_clock_millis()
                        .saturating_add(self.scheduler.virtual_wall_millis)
                };
                base.checked_add(scheduled.period)
            } else {
                None
            };
            let outcome = self.call(method, [Value::Reference(Some(scheduled.task))], depth + 1);
            self.heap.frame_roots.remove(depth);
            self.scheduler.current_thread = previous_thread;
            self.scheduler.quantum_remaining = previous_quantum;
            self.scheduler.suspend_requested = previous_suspend_requested;
            if self.scheduler.timer_threads.contains_key(&scheduled.timer) {
                self.scheduler.thread_states.remove(&timer_thread);
            } else {
                self.scheduler.finish_thread(timer_thread);
            }
            self.scheduler.dispatching_timer = false;
            match outcome? {
                CallOutcome::Return(None) => {}
                CallOutcome::Return(Some(_)) => return Err(type_error()),
                CallOutcome::Suspend(_) => {
                    return Err(vm_error("timer-suspend", "TimerTask suspended"));
                }
                CallOutcome::Throw(handle) => {
                    let exception_class = self.object_class(handle)?;
                    // The CLDC reference implementation catches Exception
                    // around TimerTask.run(), cancels only that task, and
                    // keeps the Timer thread alive. Errors and other direct
                    // Throwable subclasses still terminate the Timer thread.
                    if self.is_instance(&exception_class, "java/lang/Exception") {
                        self.heap
                            .managed
                            .set_field(
                                scheduled.task,
                                "java/util/TimerTask.cancelled:Z",
                                HeapValue::Int(1),
                            )
                            .map_err(heap_error)?;
                        continue;
                    }
                    self.report_uncaught_thread_exception(timer_thread, handle)?;
                    self.heap
                        .managed
                        .set_field(
                            scheduled.timer,
                            "java/util/Timer.cancelled:Z",
                            HeapValue::Int(1),
                        )
                        .map_err(heap_error)?;
                    self.scheduler.cancel_timer(scheduled.timer);
                    continue;
                }
            }
            if scheduled.period > 0 {
                // cancel_timer also removes the current Timer thread. A
                // callback may have cancelled its own Timer while running.
                if !self.scheduler.timer_threads.contains_key(&scheduled.timer) {
                    continue;
                }
                if let Some(next) = next_deadline {
                    scheduled.deadline = next;
                    self.scheduler.scheduled_tasks.push(scheduled);
                } else {
                    self.heap
                        .managed
                        .set_field(
                            scheduled.task,
                            "java/util/TimerTask.cancelled:Z",
                            HeapValue::Int(1),
                        )
                        .map_err(heap_error)?;
                }
            }
        }
        if self
            .scheduler
            .scheduled_tasks
            .iter()
            .any(|task| task.deadline <= now)
        {
            Err(vm_error(
                "timer-limit",
                "too many TimerTask firings at one instruction boundary",
            ))
        } else {
            Ok(())
        }
    }

    pub(in crate::machine) fn run_display_event_turn(
        &mut self,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        const DISPLAY_CLASS: &str = "javax/microedition/lcdui/Display";
        // A worker can yield while its paint callback still owns the singleton
        // Graphics3D target.  Servicing another repaint from __hostIdle at that
        // point would re-enter paint on the host thread and make releaseTarget
        // fail for the original owner.  Leave the coalesced repaint pending
        // until the active M3G paint has released its target.
        if self.scheduler.dispatching_display
            || self.m3g.graphics.target.is_some()
            || !self.classes.initialized.contains(DISPLAY_CLASS)
        {
            return Ok(CallOutcome::Return(None));
        }
        let key = MethodKey {
            class: DISPLAY_CLASS.to_owned(),
            name: "__hostIdle".to_owned(),
            descriptor: "()V".to_owned(),
        };
        let Some(method) = self.program.methods.get(&key) else {
            return Ok(CallOutcome::Return(None));
        };
        self.scheduler.dispatching_display = true;
        let outcome = self.call(method, [], depth);
        self.scheduler.dispatching_display = false;
        match outcome? {
            CallOutcome::Return(None) => Ok(CallOutcome::Return(None)),
            CallOutcome::Return(Some(_)) => Err(type_error()),
            CallOutcome::Suspend(_) => {
                Err(vm_error("display-suspend", "Display event turn suspended"))
            }
            CallOutcome::Throw(handle) => {
                // LCDUI callbacks conceptually run on the implementation's
                // event thread. This cooperative turn happens inline only so
                // a sleeping MIDlet thread does not prevent repaint progress;
                // an exception from paint/callSerially must not be injected
                // into the unrelated thread that happened to yield.
                if self.native_context.vm_flight_recorder_enabled() {
                    let exception_class = self.object_class(handle)?;
                    let message = diagnostic_message(|output| {
                        write!(
                            output,
                            "display-callback-uncaught exception={exception_class} stack="
                        )?;
                        append_java_stack(output, self.throwable_stack_frames(handle))
                    });
                    self.native_context.record_vm_flight(message);
                }
                Ok(CallOutcome::Return(None))
            }
        }
    }
}
