//! Restores scheduler queues and relinks saved Java continuations.

use super::super::{
    EmuError, Handle, HashMap, HashSet, Heap, Limits, MAIN_THREAD_ID, Monitor, Program,
    ScheduledJavaTask, SchedulerState, ThreadState, VecDeque, vm_error,
};
use super::continuation::{SavedCall, SavedDriverCall};
use serde::Deserialize;

// Field order matches SchedulerState's checkpoint serialization.
#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
struct SavedScheduler {
    scheduled_tasks: Vec<ScheduledJavaTask>,
    timer_poll_requested: bool,
    timer_poll_instructions: u64,
    timer_poll_countdown: u64,
    dispatching_timer: bool,
    dispatching_display: bool,
    dispatching_canvas_paint: bool,
    virtual_wall_millis: i64,
    virtual_monotonic_millis: i64,
    instruction_pacing_checkpoint: u64,
    instruction_pacing_started_millis: i64,
    media_pump_checkpoint_millis: i64,
    host_driver_active: bool,
    suspended_driver_call: Option<SavedDriverCall>,
    driver_host_poll_yield: bool,
    current_thread: u64,
    main_thread: Option<Handle>,
    next_thread_name: u64,
    thread_states: HashMap<Handle, ThreadState>,
    runnable_threads: VecDeque<Handle>,
    sleeping_threads: HashMap<Handle, i64>,
    join_waiters: HashMap<Handle, Vec<Handle>>,
    thread_continuations: HashMap<Handle, SavedCall>,
    quantum_remaining: u64,
    suspend_requested: bool,
    interrupted_threads: HashSet<u64>,
    monitors: HashMap<Handle, Monitor>,
    inline_monitor_entry_waits: HashSet<Handle>,
    monitor_entry_waiters: HashMap<Handle, VecDeque<Handle>>,
    monitor_waiters: HashMap<Handle, VecDeque<u64>>,
    notified_threads: HashSet<u64>,
    timer_threads: HashMap<Handle, Handle>,
}

impl SchedulerState {
    pub(in crate::machine) fn restore_checkpoint(
        bytes: &[u8],
        program: &Program,
        limits: &Limits,
        heap: &Heap,
    ) -> Result<Self, EmuError> {
        let saved: SavedScheduler = save_state::decode(bytes)?;
        if saved.current_thread != MAIN_THREAD_ID
            // Inline callbacks and monitor entrants cannot outlive their Rust
            // frames. A saved continuation is parked, never actively running.
            || saved.dispatching_timer
            || saved.dispatching_display
            || saved.dispatching_canvas_paint
            || !saved.inline_monitor_entry_waits.is_empty()
            || (!saved.host_driver_active && saved.suspended_driver_call.is_some())
            // Retained terminated Thread objects must stay non-restartable, but
            // consume only heap capacity, not the limit of simultaneous workers.
            || saved.thread_states.len() > heap.len()
            || saved.thread_states.keys().any(|thread| heap.get(*thread).is_err())
            || saved.thread_states.values().filter(|state| state.is_alive()).count() > limits.max_threads
            || saved.thread_continuations.len() > limits.max_threads
            || saved.runnable_threads.len() > limits.max_threads
            || saved.sleeping_threads.len() > limits.max_threads
            || saved.scheduled_tasks.len() > Self::MAX_SCHEDULED_TASKS
            || saved.scheduled_tasks.len() > limits.max_heap_bytes / 8
            || saved.timer_poll_instructions == 0
            || saved
                .monitors
                .values()
                .any(|monitor| monitor.depth == 0 || monitor.depth > limits.max_stack_slots)
        {
            return Err(vm_error(
                "checkpoint-scheduler",
                "The checkpoint contains invalid Java scheduler state.",
            ));
        }
        saved.validate_worker_queues()?;
        let suspended_driver_call = saved
            .suspended_driver_call
            .map(|call| call.restore(program, limits))
            .transpose()?;
        let thread_continuations = saved
            .thread_continuations
            .into_iter()
            .map(|(thread, call)| Ok((thread, call.restore(program, limits)?)))
            .collect::<Result<_, EmuError>>()?;
        Ok(Self {
            scheduled_tasks: saved.scheduled_tasks,
            timer_poll_requested: saved.timer_poll_requested,
            timer_poll_instructions: saved.timer_poll_instructions,
            timer_poll_countdown: saved.timer_poll_countdown,
            dispatching_timer: saved.dispatching_timer,
            dispatching_display: saved.dispatching_display,
            dispatching_canvas_paint: saved.dispatching_canvas_paint,
            virtual_wall_millis: saved.virtual_wall_millis,
            virtual_monotonic_millis: saved.virtual_monotonic_millis,
            instruction_pacing_checkpoint: saved.instruction_pacing_checkpoint,
            instruction_pacing_started_millis: saved.instruction_pacing_started_millis,
            media_pump_checkpoint_millis: saved.media_pump_checkpoint_millis,
            host_driver_active: saved.host_driver_active,
            suspended_driver_call,
            driver_host_poll_yield: saved.driver_host_poll_yield,
            current_thread: saved.current_thread,
            main_thread: saved.main_thread,
            next_thread_name: saved.next_thread_name,
            thread_states: saved.thread_states,
            runnable_threads: saved.runnable_threads,
            sleeping_threads: saved.sleeping_threads,
            join_waiters: saved.join_waiters,
            thread_continuations,
            quantum_remaining: saved.quantum_remaining,
            suspend_requested: saved.suspend_requested,
            interrupted_threads: saved.interrupted_threads,
            monitors: saved.monitors,
            inline_monitor_entry_waits: saved.inline_monitor_entry_waits,
            monitor_entry_waiters: saved.monitor_entry_waiters,
            monitor_waiters: saved.monitor_waiters,
            notified_threads: saved.notified_threads,
            timer_threads: saved.timer_threads,
        })
    }
}

impl SavedScheduler {
    fn validate_worker_queues(&self) -> Result<(), EmuError> {
        let runnable: HashSet<_> = self.runnable_threads.iter().copied().collect();
        if runnable.len() != self.runnable_threads.len()
            || runnable
                .iter()
                .any(|thread| self.thread_states.get(thread) != Some(&ThreadState::Runnable))
            || self
                .thread_states
                .iter()
                .any(|(thread, state)| match state {
                    ThreadState::Runnable => !runnable.contains(thread),
                    // Without a continuation a woken worker would restart run()
                    // instead of returning from its original wait or sleep.
                    ThreadState::Sleeping => !self.thread_continuations.contains_key(thread),
                    ThreadState::Running => true,
                    ThreadState::New | ThreadState::Terminated => false,
                })
            || self
                .sleeping_threads
                .keys()
                .any(|thread| self.thread_states.get(thread) != Some(&ThreadState::Sleeping))
            || self.thread_continuations.keys().any(|thread| {
                !matches!(
                    self.thread_states.get(thread),
                    Some(ThreadState::Runnable | ThreadState::Sleeping)
                )
            })
        {
            return Err(vm_error(
                "checkpoint-scheduler",
                "The checkpoint contains inconsistent Java worker queues.",
            ));
        }
        Ok(())
    }
}
