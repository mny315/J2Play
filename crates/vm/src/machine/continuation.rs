//! Parked Java call frames, heap references and guarded resumption.

use super::{
    CallOutcome, EmuError, Handle, Machine, Method, Value, ValueKind, compatibility_reader,
    value_reference, vm_error,
};

#[derive(Debug)]
pub(super) struct SuspendedCall {
    pub(super) method: Method,
    pub(super) locals: Vec<Option<Value>>,
    pub(super) stack: Vec<Value>,
    pub(super) pc: usize,
    pub(super) synchronized_monitor: Option<Handle>,
    pub(super) monitor_entry: Option<PendingMonitorEntry>,
    pub(super) pending: Option<PendingCall>,
    pub(super) native_resume: Option<NativeResume>,
    pub(super) class_initialization: Option<SuspendedClassInitialization>,
}

#[derive(Debug)]
pub(super) struct PendingCall {
    pub(super) child: Box<SuspendedCall>,
    pub(super) next_pc: usize,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct SuspendedDriverCall {
    pub(super) continuation: Box<SuspendedCall>,
    pub(super) operation: String,
    pub(super) run_worker_after: bool,
    pub(super) wake: SuspendedDriverWake,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) enum SuspendedDriverWake {
    MainThreadSignal,
    PollHost,
    ResumeAfterHostPoll,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct PendingMonitorEntry {
    pub(super) object: Handle,
    pub(super) args: Vec<Value>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) enum NativeResume {
    Sleep,
    FramePresentation {
        pixels: Handle,
        deadline: i64,
    },
    Join {
        target: Handle,
    },
    MonitorWait {
        object: Handle,
        monitor_depth: usize,
    },
    ClassNewInstance {
        instance: Handle,
    },
    ClassForName {
        class: String,
    },
    ClassNewInstanceInitialization {
        class: String,
    },
    DataInputReadUtf {
        input: Handle,
        length: Option<usize>,
        bytes: Vec<u8>,
    },
    InputStreamReaderClose {
        reader: Handle,
    },
    InputStreamReaderRead(compatibility_reader::ReaderState),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct SuspendedClassInitialization {
    pub(super) class: String,
    pub(super) phase: ClassInitializationPhase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) enum ClassInitializationPhase {
    Waiting,
    AfterParent,
    AfterInitializer,
}

impl SuspendedCall {
    pub(super) fn new(method: &Method) -> Self {
        Self {
            method: method.clone(),
            locals: Vec::new(),
            stack: Vec::new(),
            pc: 0,
            synchronized_monitor: None,
            monitor_entry: None,
            pending: None,
            native_resume: None,
            class_initialization: None,
        }
    }

    pub(super) fn expected_return(&self) -> Option<ValueKind> {
        if self.class_initialization.is_some() {
            None
        } else {
            self.method.descriptor.returns
        }
    }

    pub(super) fn frames(&self) -> impl Iterator<Item = &Self> {
        std::iter::successors(Some(self), |frame| {
            frame.pending.as_ref().map(|pending| pending.child.as_ref())
        })
    }

    pub(super) fn roots(&self, roots: &mut Vec<Handle>) {
        for frame in self.frames() {
            roots.extend(frame.locals.iter().flatten().filter_map(value_reference));
            roots.extend(frame.stack.iter().filter_map(value_reference));
            roots.extend(frame.synchronized_monitor);
            if let Some(entry) = &frame.monitor_entry {
                roots.push(entry.object);
                roots.extend(entry.args.iter().filter_map(value_reference));
            }
            match frame.native_resume {
                Some(NativeResume::FramePresentation { pixels, .. }) => roots.push(pixels),
                Some(NativeResume::Join { target }) => roots.push(target),
                Some(NativeResume::MonitorWait { object, .. }) => roots.push(object),
                Some(NativeResume::ClassNewInstance { instance }) => roots.push(instance),
                Some(NativeResume::DataInputReadUtf { input, .. }) => roots.push(input),
                Some(NativeResume::InputStreamReaderClose { reader }) => roots.push(reader),
                Some(NativeResume::InputStreamReaderRead(ref state)) => state.append_roots(roots),
                Some(
                    NativeResume::Sleep
                    | NativeResume::ClassForName { .. }
                    | NativeResume::ClassNewInstanceInitialization { .. },
                )
                | None => {}
            }
        }
    }
}

pub(super) fn suspended_native_call(method: &Method, native_resume: NativeResume) -> CallOutcome {
    CallOutcome::Suspend(Box::new(SuspendedCall {
        native_resume: Some(native_resume),
        ..SuspendedCall::new(method)
    }))
}

impl Machine<'_, '_> {
    pub(super) fn resume_suspended_call(
        &mut self,
        mut call: Box<SuspendedCall>,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        if let Some(entry) = call.monitor_entry.take() {
            let method = call.method;
            return self.call_with_synchronized_monitor(
                &method,
                &entry.args,
                depth,
                Some(entry.object),
            );
        }
        if let Some(initialization) = call.class_initialization.take() {
            let child = call.pending.take().map(|pending| pending.child);
            return self.resume_class_initialization(&call.method, initialization, child, depth);
        }
        // call_inner accounts for the saved frame while it runs. Restore the
        // caller's slots on every outcome, including another suspension, so
        // successive quanta cannot accumulate the same frame's stack usage.
        let caller_frame_slots = self.execution.frame_slots;
        let caller_stack_slots = self.execution.stack_slots;
        let method = call.method.clone();
        let monitor = call.synchronized_monitor;
        self.execution
            .call_stack
            .push(method.active_stack_frame(call.pc));
        let mut outcome = if self.execution.call_stack.len() > self.limits.max_frames
            || depth > self.limits.max_frames
        {
            Err(vm_error(
                "stack-overflow",
                format!("frame limit {} exceeded", self.limits.max_frames),
            ))
        } else if self.execution.call_stack.len() >= super::DIRECT_HOST_FRAME_DEPTH {
            // Restoring a saved Java stack recursively enters the same large
            // interpreter frames as an ordinary call. Retain its host stack
            // guard as well as the guest frame bound, including on re-resume.
            stacker::maybe_grow(
                super::HOST_STACK_RED_ZONE,
                super::HOST_STACK_SEGMENT,
                || self.call_inner(&method, &[], depth, monitor, Some(call)),
            )
        } else {
            self.call_inner(&method, &[], depth, monitor, Some(call))
        };
        if !matches!(outcome, Ok(CallOutcome::Suspend(_)))
            && let Some(monitor) = monitor
            && let Err(error) = self.exit_monitor(monitor)
        {
            outcome = Err(error);
        }
        self.execution.frame_slots = caller_frame_slots;
        self.execution.stack_slots = caller_stack_slots;
        self.heap.frame_roots.remove(depth);
        if let Err(error) = outcome {
            outcome = self.call_error_outcome(&method, error, &[]);
        }
        self.execution.call_stack.pop();
        outcome
    }
}

pub(super) fn suspended_monitor_entry(
    method: &Method,
    object: Handle,
    args: Vec<Value>,
) -> CallOutcome {
    CallOutcome::Suspend(Box::new(SuspendedCall {
        monitor_entry: Some(PendingMonitorEntry { object, args }),
        ..SuspendedCall::new(method)
    }))
}

pub(super) fn suspended_native_pending_call(
    method: &Method,
    native_resume: NativeResume,
    child: Box<SuspendedCall>,
) -> CallOutcome {
    CallOutcome::Suspend(Box::new(SuspendedCall {
        pending: Some(PendingCall { child, next_pc: 0 }),
        native_resume: Some(native_resume),
        ..SuspendedCall::new(method)
    }))
}

pub(super) enum ClassInitializationOutcome {
    Ready,
    Suspend(Box<SuspendedCall>),
    Throw(Handle),
}

pub(super) fn suspended_class_initialization(
    method: &Method,
    class: String,
    phase: ClassInitializationPhase,
    child: Option<Box<SuspendedCall>>,
) -> ClassInitializationOutcome {
    ClassInitializationOutcome::Suspend(Box::new(SuspendedCall {
        pending: child.map(|child| PendingCall { child, next_pc: 0 }),
        class_initialization: Some(SuspendedClassInitialization { class, phase }),
        ..SuspendedCall::new(method)
    }))
}
