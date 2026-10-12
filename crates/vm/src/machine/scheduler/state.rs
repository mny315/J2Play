//! Java thread, timer and monitor state owned by one VM scheduler.

use crate::machine::{
    Handle, HashMap, HashSet, Heap, MAIN_THREAD_ID, SuspendedCall, SuspendedDriverCall, VecDeque,
};

#[derive(Debug, serde::Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub(in crate::machine) struct SchedulerState {
    pub(in crate::machine) scheduled_tasks: Vec<ScheduledJavaTask>,
    pub(in crate::machine) timer_poll_requested: bool,
    pub(in crate::machine) timer_poll_instructions: u64,
    pub(in crate::machine) timer_poll_countdown: u64,
    pub(in crate::machine) dispatching_timer: bool,
    pub(in crate::machine) dispatching_display: bool,
    pub(in crate::machine) dispatching_canvas_paint: bool,
    pub(in crate::machine) virtual_wall_millis: i64,
    pub(in crate::machine) virtual_monotonic_millis: i64,
    pub(in crate::machine) instruction_pacing_checkpoint: u64,
    pub(in crate::machine) instruction_pacing_started_millis: i64,
    pub(in crate::machine) media_pump_checkpoint_millis: i64,
    // Keep driver continuations rooted while Object.wait() permits host polling.
    pub(in crate::machine) host_driver_active: bool,
    pub(in crate::machine) suspended_driver_call: Option<SuspendedDriverCall>,
    // Distinguish a host-poll yield from wait/notification suspension.
    pub(in crate::machine) driver_host_poll_yield: bool,
    pub(in crate::machine) current_thread: u64,
    pub(in crate::machine) main_thread: Option<Handle>,
    pub(in crate::machine) next_thread_name: u64,
    pub(in crate::machine) thread_states: HashMap<Handle, ThreadState>,
    pub(in crate::machine) runnable_threads: VecDeque<Handle>,
    // Real-time sleeping Java workers are parked here instead of blocking the
    // single host interpreter thread. The deadline is in pacing monotonic ms.
    pub(in crate::machine) sleeping_threads: HashMap<Handle, i64>,
    // Workers that joined an outer, recursively scheduled worker wait here
    // until that target unwinds from the host interpreter stack.
    pub(in crate::machine) join_waiters: HashMap<Handle, Vec<Handle>>,
    pub(in crate::machine) thread_continuations: HashMap<Handle, Box<SuspendedCall>>,
    pub(in crate::machine) quantum_remaining: u64,
    pub(in crate::machine) suspend_requested: bool,
    pub(in crate::machine) interrupted_threads: HashSet<u64>,
    pub(in crate::machine) monitors: HashMap<Handle, Monitor>,
    // Reserve contested monitors for inline callers while they run the owner;
    // otherwise that worker could release and reacquire before the caller resumes.
    pub(in crate::machine) inline_monitor_entry_waits: HashSet<Handle>,
    // Threads contending for a monitor must leave the recursive interpreter
    // stack before its current owner can resume and release that monitor.
    pub(in crate::machine) monitor_entry_waiters: HashMap<Handle, VecDeque<Handle>>,
    pub(in crate::machine) monitor_waiters: HashMap<Handle, VecDeque<u64>>,
    pub(in crate::machine) notified_threads: HashSet<u64>,
    pub(in crate::machine) timer_threads: HashMap<Handle, Handle>,
}

impl SchedulerState {
    pub(in crate::machine) const MAX_SCHEDULED_TASKS: usize = 1_024;

    pub(in crate::machine) fn new(
        instruction_pacing_started_millis: i64,
        timer_poll_instructions: u64,
    ) -> Self {
        Self {
            scheduled_tasks: Vec::new(),
            timer_poll_requested: false,
            timer_poll_instructions: timer_poll_instructions.max(1),
            timer_poll_countdown: 0,
            dispatching_timer: false,
            dispatching_display: false,
            dispatching_canvas_paint: false,
            virtual_wall_millis: 0,
            virtual_monotonic_millis: 0,
            instruction_pacing_checkpoint: 0,
            instruction_pacing_started_millis,
            media_pump_checkpoint_millis: instruction_pacing_started_millis,
            host_driver_active: false,
            suspended_driver_call: None,
            driver_host_poll_yield: false,
            current_thread: MAIN_THREAD_ID,
            main_thread: None,
            next_thread_name: 0,
            thread_states: HashMap::new(),
            runnable_threads: VecDeque::new(),
            sleeping_threads: HashMap::new(),
            join_waiters: HashMap::new(),
            thread_continuations: HashMap::new(),
            quantum_remaining: 0,
            suspend_requested: false,
            interrupted_threads: HashSet::new(),
            monitors: HashMap::new(),
            inline_monitor_entry_waits: HashSet::new(),
            monitor_entry_waiters: HashMap::new(),
            monitor_waiters: HashMap::new(),
            notified_threads: HashSet::new(),
            timer_threads: HashMap::new(),
        }
    }

    pub(in crate::machine) fn append_roots(&self, roots: &mut Vec<Handle>) {
        roots.extend(
            self.scheduled_tasks
                .iter()
                .flat_map(|task| [task.timer, task.task]),
        );
        roots.extend(
            self.thread_states
                .iter()
                .filter_map(|(thread, state)| state.is_alive().then_some(*thread)),
        );
        roots.extend(self.monitors.keys().copied());
        roots.extend(self.inline_monitor_entry_waits.iter().copied());
        roots.extend(self.monitor_entry_waiters.keys().copied());
        roots.extend(self.main_thread);
        for (thread, continuation) in &self.thread_continuations {
            roots.push(*thread);
            continuation.roots(roots);
        }
        if let Some(suspended) = &self.suspended_driver_call {
            suspended.continuation.roots(roots);
        }
    }

    pub(in crate::machine) fn sweep(&mut self, heap: &Heap) {
        let retired_threads = self
            .timer_threads
            .extract_if(|timer, thread| heap.get(*timer).is_err() || heap.get(*thread).is_err())
            .map(|(_, thread)| thread)
            .collect::<Vec<_>>();
        for thread in retired_threads {
            self.finish_thread(thread);
        }
        self.thread_states
            .retain(|thread, state| *state != ThreadState::Terminated || heap.get(*thread).is_ok());
    }

    pub(in crate::machine) fn thread_is_alive(&self, thread: Handle) -> bool {
        self.main_thread == Some(thread)
            || self
                .timer_threads
                .values()
                .any(|candidate| *candidate == thread)
            || self
                .thread_states
                .get(&thread)
                .is_some_and(|state| state.is_alive())
    }

    pub(in crate::machine) fn cancel_timer(&mut self, timer: Handle) {
        self.scheduled_tasks
            .retain(|scheduled| scheduled.timer != timer);
        if self.scheduled_tasks.is_empty() {
            self.timer_poll_requested = false;
            self.timer_poll_countdown = 0;
        }
        if let Some(thread) = self.timer_threads.remove(&timer)
            && self.thread_states.get(&thread) != Some(&ThreadState::Running)
        {
            self.finish_thread(thread);
        }
    }

    pub(in crate::machine) fn finish_thread(&mut self, thread: Handle) {
        self.thread_states.insert(thread, ThreadState::Terminated);
        self.wake_join_waiters(thread);
        self.sleeping_threads.remove(&thread);
        self.interrupted_threads.remove(&thread.to_raw());
        self.thread_continuations.remove(&thread);
    }

    pub(in crate::machine) fn wake_join_waiters(&mut self, target: Handle) {
        let Some(waiters) = self.join_waiters.remove(&target) else {
            return;
        };
        for waiter in waiters {
            if self.main_thread == Some(waiter) {
                self.notified_threads.insert(MAIN_THREAD_ID);
            } else if let Some(state) = self.thread_states.get_mut(&waiter)
                && *state == ThreadState::Sleeping
            {
                *state = ThreadState::Runnable;
                self.runnable_threads.push_back(waiter);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(in crate::machine) struct ScheduledJavaTask {
    pub(in crate::machine) timer: Handle,
    pub(in crate::machine) task: Handle,
    pub(in crate::machine) deadline: i64,
    pub(in crate::machine) period: i64,
    pub(in crate::machine) fixed_rate: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(in crate::machine) enum ThreadState {
    New,
    Runnable,
    Running,
    Sleeping,
    Terminated,
}

impl ThreadState {
    pub(in crate::machine) fn is_alive(self) -> bool {
        matches!(self, Self::Runnable | Self::Running | Self::Sleeping)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(in crate::machine) struct Monitor {
    pub(in crate::machine) owner: u64,
    pub(in crate::machine) depth: usize,
}
