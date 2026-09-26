//! Flat saved Java call chains; decoding cannot recurse through guest frames.

use super::super::{
    EmuError, Handle, Limits, MethodKey, NativeResume, PendingCall, PendingMonitorEntry, Program,
    SuspendedCall, SuspendedClassInitialization, SuspendedDriverCall, SuspendedDriverWake, Value,
    slot_count, validate_call_arguments, vm_error,
};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeSeq};

#[derive(Serialize)]
struct FrameRef<'a> {
    method: &'a MethodKey,
    locals: &'a [Option<Value>],
    stack: &'a [Value],
    pc: usize,
    synchronized_monitor: Option<Handle>,
    monitor_entry: &'a Option<PendingMonitorEntry>,
    next_pc: Option<usize>,
    native_resume: &'a Option<NativeResume>,
    class_initialization: &'a Option<SuspendedClassInitialization>,
}

#[derive(Deserialize)]
struct SavedFrame {
    method: MethodKey,
    locals: Vec<Option<Value>>,
    stack: Vec<Value>,
    pc: usize,
    synchronized_monitor: Option<Handle>,
    monitor_entry: Option<PendingMonitorEntry>,
    next_pc: Option<usize>,
    native_resume: Option<NativeResume>,
    class_initialization: Option<SuspendedClassInitialization>,
}

impl Serialize for SuspendedCall {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let frame_count = self.frames().take(513).count();
        if frame_count > 512 {
            return Err(serde::ser::Error::custom(
                "checkpoint call chain is too deep",
            ));
        }
        let mut output = serializer.serialize_seq(Some(frame_count))?;
        for frame in self.frames() {
            output.serialize_element(&FrameRef {
                method: &frame.method.key,
                locals: &frame.locals,
                stack: &frame.stack,
                pc: frame.pc,
                synchronized_monitor: frame.synchronized_monitor,
                monitor_entry: &frame.monitor_entry,
                next_pc: frame.pending.as_ref().map(|pending| pending.next_pc),
                native_resume: &frame.native_resume,
                class_initialization: &frame.class_initialization,
            })?;
        }
        output.end()
    }
}

#[derive(Deserialize)]
pub(super) struct SavedCall(Vec<SavedFrame>);

impl SavedCall {
    pub(super) fn restore(
        self,
        program: &Program,
        limits: &Limits,
    ) -> Result<Box<SuspendedCall>, EmuError> {
        if self.0.is_empty() || self.0.len() > limits.max_frames.min(512) {
            return Err(invalid());
        }
        let mut child = None;
        let mut slots = 0_usize;
        for frame in self.0.into_iter().rev() {
            let method = program.methods.get(&frame.method).ok_or_else(invalid)?;
            // A call waiting to enter a synchronized method has arguments but
            // no initialized frame or acquired monitor yet.
            let entry_slots = if let Some(entry) = &frame.monitor_entry {
                if !method.is_synchronized
                    || frame.pc != 0
                    || !frame.locals.is_empty()
                    || !frame.stack.is_empty()
                    || frame.synchronized_monitor.is_some()
                    || frame.next_pc.is_some()
                    || frame.native_resume.is_some()
                    || frame.class_initialization.is_some()
                    || validate_call_arguments(method, &entry.args).is_err()
                    || (!method.is_static
                        && entry.args.first() != Some(&Value::Reference(Some(entry.object))))
                {
                    return Err(invalid());
                }
                slot_count(&entry.args)
            } else {
                0
            };
            let operand_slots = slot_count(&frame.stack);
            slots = slots
                .checked_add(frame.locals.len())
                .and_then(|slots| slots.checked_add(operand_slots))
                .and_then(|slots| slots.checked_add(entry_slots))
                .filter(|slots| *slots <= limits.max_stack_slots)
                .ok_or_else(invalid)?;
            if !valid_pc(method, frame.pc)
                || frame.next_pc.is_some_and(|pc| !valid_pc(method, pc))
                || (frame.class_initialization.is_none()
                    && frame.monitor_entry.is_none()
                    && !method.is_native
                    && (frame.locals.len() != method.max_locals
                        || operand_slots > method.max_stack))
                || frame
                    .class_initialization
                    .as_ref()
                    .is_some_and(|init| !program.classes.contains_key(&init.class))
            {
                return Err(invalid());
            }
            let pending = match (frame.next_pc, child.take()) {
                (Some(next_pc), Some(child)) => Some(PendingCall { child, next_pc }),
                (None, None) => None,
                _ => return Err(invalid()),
            };
            child = Some(Box::new(SuspendedCall {
                method: method.clone(),
                locals: frame.locals,
                stack: frame.stack,
                pc: frame.pc,
                synchronized_monitor: frame.synchronized_monitor,
                monitor_entry: frame.monitor_entry,
                pending,
                native_resume: frame.native_resume,
                class_initialization: frame.class_initialization,
            }));
        }
        child.ok_or_else(invalid)
    }
}

fn valid_pc(method: &super::super::Method, pc: usize) -> bool {
    if method.is_native || method.code.is_empty() {
        return pc == 0;
    }
    method
        .instruction_index
        .get(pc)
        .is_some_and(|index| *index != u16::MAX)
}

#[derive(Deserialize)]
pub(super) struct SavedDriverCall {
    continuation: SavedCall,
    operation: String,
    run_worker_after: bool,
    wake: SuspendedDriverWake,
}

impl SavedDriverCall {
    pub(super) fn restore(
        self,
        program: &Program,
        limits: &Limits,
    ) -> Result<SuspendedDriverCall, EmuError> {
        if self.operation.len() > 65_535 {
            return Err(invalid());
        }
        Ok(SuspendedDriverCall {
            continuation: self.continuation.restore(program, limits)?,
            operation: self.operation,
            run_worker_after: self.run_worker_after,
            wake: self.wake,
        })
    }
}

fn invalid() -> EmuError {
    vm_error(
        "checkpoint-stack",
        "The checkpoint contains an invalid Java continuation.",
    )
}

#[cfg(test)]
#[path = "../../../../../tests/unit/vm/machine/checkpoint/continuation.rs"]
mod tests;
