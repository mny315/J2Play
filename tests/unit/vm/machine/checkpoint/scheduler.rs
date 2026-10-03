use super::*;
use crate::machine::{Handle, Limits, SchedulerState, SuspendedDriverCall, SuspendedDriverWake};

fn rejects_scheduler(change: impl FnOnce(&mut SchedulerState, Handle, &Program)) {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let instance = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    change(&mut machine.scheduler, instance, &program);
    let bytes = machine.encode_checkpoint(instance).unwrap();
    let mut restored_context = DefaultNativeContext;
    let mut restored = program.machine(Limits::default(), false, &mut restored_context);
    restored.device.random_seed_sequence = 77;
    assert_eq!(
        restored
            .restore_checkpoint("Checkpoint", &bytes)
            .unwrap_err()
            .code(),
        "checkpoint-scheduler"
    );
    assert_eq!(restored.device.random_seed_sequence, 77);
    assert!(restored.scheduler.thread_states.is_empty());
    assert!(restored.heap.managed.is_empty());
}

fn continuation(program: &Program) -> Box<SuspendedCall> {
    let method = program.methods.values().next().unwrap();
    Box::new(SuspendedCall {
        locals: vec![Some(Value::Int(41))],
        ..SuspendedCall::new(method)
    })
}

#[test]
fn checkpoint_rejects_active_scheduler_callbacks_after_the_host_stack_unwinds() {
    for flag in [
        "timer",
        "display",
        "paint",
        "inline-monitor",
        "running-thread",
    ] {
        rejects_scheduler(|scheduler, instance, _| match flag {
            "timer" => scheduler.dispatching_timer = true,
            "display" => scheduler.dispatching_display = true,
            "paint" => scheduler.dispatching_canvas_paint = true,
            "inline-monitor" => {
                scheduler.inline_monitor_entry_waits.insert(instance);
            }
            "running-thread" => {
                scheduler
                    .thread_states
                    .insert(instance, ThreadState::Running);
            }
            _ => unreachable!(),
        });
    }
}

#[test]
fn checkpoint_rejects_duplicate_or_inconsistent_runnable_queues() {
    for state in [
        None,
        Some(ThreadState::Sleeping),
        Some(ThreadState::Terminated),
        Some(ThreadState::Runnable),
    ] {
        rejects_scheduler(|scheduler, instance, _| {
            if let Some(state) = state {
                scheduler.thread_states.insert(instance, state);
            }
            scheduler.runnable_threads.push_back(instance);
            if state == Some(ThreadState::Runnable) {
                scheduler.runnable_threads.push_back(instance);
            }
        });
    }
    rejects_scheduler(|scheduler, instance, _| {
        scheduler
            .thread_states
            .insert(instance, ThreadState::Runnable);
    });
}

#[test]
fn checkpoint_rejects_orphaned_worker_continuations_and_sleep_deadlines() {
    for state in [None, Some(ThreadState::New), Some(ThreadState::Terminated)] {
        rejects_scheduler(|scheduler, instance, program| {
            if let Some(state) = state {
                scheduler.thread_states.insert(instance, state);
            }
            scheduler
                .thread_continuations
                .insert(instance, continuation(program));
        });
    }
    rejects_scheduler(|scheduler, instance, _| {
        scheduler
            .thread_states
            .insert(instance, ThreadState::Sleeping);
        scheduler.sleeping_threads.insert(instance, 1000);
    });
    rejects_scheduler(|scheduler, instance, program| {
        scheduler
            .thread_states
            .insert(instance, ThreadState::Runnable);
        scheduler.runnable_threads.push_back(instance);
        scheduler
            .thread_continuations
            .insert(instance, continuation(program));
        scheduler.sleeping_threads.insert(instance, 1000);
    });
}

#[test]
fn checkpoint_rejects_a_driver_continuation_without_an_active_host_driver() {
    rejects_scheduler(|scheduler, _, program| {
        scheduler.suspended_driver_call = Some(SuspendedDriverCall {
            continuation: continuation(program),
            operation: "fixture".into(),
            run_worker_after: false,
            wake: SuspendedDriverWake::PollHost,
        });
    });
}

#[test]
fn checkpoint_resumes_queued_and_sleeping_workers_without_restarting_their_calls() {
    let mut program = program();
    let run = runtime_method(
        "Checkpoint",
        "run",
        "()V",
        &[0x00, 0xb1],
        0,
        1,
        vec![],
        false,
    );
    program.methods.insert(run.key.clone(), run.clone());
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let queued = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    let sleeping = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    machine
        .scheduler
        .thread_states
        .insert(queued, ThreadState::Runnable);
    machine.scheduler.runnable_threads.push_back(queued);
    machine
        .scheduler
        .thread_states
        .insert(sleeping, ThreadState::Sleeping);
    machine.scheduler.sleeping_threads.insert(sleeping, 1000);
    machine.scheduler.thread_continuations.insert(
        sleeping,
        Box::new(SuspendedCall {
            locals: vec![Some(Value::Reference(Some(sleeping)))],
            pc: 1,
            ..SuspendedCall::new(&run)
        }),
    );
    let bytes = machine.encode_checkpoint(queued).unwrap();
    let mut restored_context = DefaultNativeContext;
    let mut restored = program.machine(Limits::default(), false, &mut restored_context);
    restored.restore_checkpoint("Checkpoint", &bytes).unwrap();
    assert!(restored.run_one_thread(1).unwrap());
    assert!(restored.run_one_thread(1).unwrap());
    assert_eq!(
        restored.scheduler.thread_states[&queued],
        ThreadState::Terminated
    );
    assert_eq!(
        restored.scheduler.thread_states[&sleeping],
        ThreadState::Terminated
    );
    assert_eq!(restored.execution.instructions, 3);
    assert!(restored.scheduler.thread_continuations.is_empty());
    assert!(restored.scheduler.sleeping_threads.is_empty());
}
