//! Semantic checkpoints captured only after guest Rust frames have unwound.

use super::{
    Allocation, ClassState, DeviceState, EmuError, Handle, HeapState, Jsr239State, M3gState,
    Machine, Micro3dState, SchedulerState, VibrationRequest, vm_error,
};
use serde::{Deserialize, Serialize};

mod continuation;
mod heap_payloads;
mod scheduler;

// Includes the bootstrap heap and continuation contracts: fields, method flags,
// bytecode offsets and local layouts must match the serialized Rust structures.
const FORMAT: u32 = 8;

#[derive(Serialize)]
struct CheckpointRef<'a> {
    format: u32,
    instance: Handle,
    instructions: u64,
    #[serde(serialize_with = "save_state::serialize_bytes")]
    classes: Vec<u8>,
    heap: &'a HeapState,
    #[serde(serialize_with = "save_state::serialize_bytes")]
    scheduler: Vec<u8>,
    device: &'a DeviceState,
    m3g: &'a M3gState,
    micro3d: &'a Micro3dState,
    jsr239: &'a Jsr239State,
}

#[derive(Deserialize)]
struct Checkpoint {
    format: u32,
    instance: Handle,
    instructions: u64,
    #[serde(deserialize_with = "save_state::deserialize_bytes")]
    classes: Vec<u8>,
    heap: HeapState,
    #[serde(deserialize_with = "save_state::deserialize_bytes")]
    scheduler: Vec<u8>,
    device: DeviceState,
    m3g: M3gState,
    micro3d: Micro3dState,
    jsr239: Jsr239State,
}

impl Machine<'_, '_> {
    pub(super) fn encode_checkpoint(&self, instance: Handle) -> Result<Vec<u8>, EmuError> {
        if !self.execution.call_stack.is_empty()
            || self.execution.frame_slots != 0
            || self.execution.stack_slots != 0
        {
            return Err(vm_error(
                "checkpoint-boundary",
                "A Java call is still active at the checkpoint boundary.",
            ));
        }
        let started = std::time::Instant::now();
        let cancelled = || {
            started.elapsed() >= std::time::Duration::from_secs(1)
                || self.native_context.execution_cancelled()
        };
        save_state::encode_cancellable(
            &CheckpointRef {
                format: FORMAT,
                instance,
                instructions: self.execution.instructions,
                classes: self.classes.encode_checkpoint(&cancelled)?,
                heap: &self.heap,
                scheduler: save_state::encode_cancellable(
                    &self.scheduler,
                    save_state::MAX_COMPONENT_BYTES,
                    &cancelled,
                )?,
                device: &self.device,
                m3g: &self.m3g,
                micro3d: &self.micro3d,
                jsr239: &self.jsr239,
            },
            save_state::MAX_COMPONENT_BYTES,
            &cancelled,
        )
    }

    pub(super) fn restore_checkpoint(
        &mut self,
        class: &str,
        bytes: &[u8],
    ) -> Result<Handle, EmuError> {
        let mut saved: Checkpoint = save_state::decode(bytes)?;
        if saved.format != FORMAT || saved.instructions >= self.limits.max_instructions {
            return Err(vm_error(
                "checkpoint-version",
                "This game checkpoint is incompatible with this version of J2Play.",
            ));
        }
        let fields = self
            .program
            .classes
            .values()
            .flat_map(|class| &class.fields)
            .filter(|field| !field.is_static)
            .map(|field| (field.key.as_ref(), field))
            .collect::<std::collections::HashMap<_, _>>();
        saved
            .heap
            .managed
            .validate_checkpoint(self.limits.max_heap_bytes, |class, key, kind| {
                let field = fields.get(key)?;
                (field.kind == kind && self.program.is_assignable_to(class, &field.declaring_class))
                    .then(|| field.field_token.clone())
            })
            .map_err(|_| {
                vm_error(
                    "checkpoint-heap",
                    "The checkpoint contains invalid Java heap state.",
                )
            })?;
        saved.heap.validate_checkpoint_payloads(self.program)?;
        if !matches!(saved.heap.managed.get(saved.instance), Ok(Allocation::Object { class: found, .. }) if found.as_ref() == class)
        {
            return Err(vm_error(
                "checkpoint-instance",
                "The checkpoint belongs to a different MIDlet.",
            ));
        }
        let classes = ClassState::restore_checkpoint(self.program, &saved.classes)?;
        let scheduler = SchedulerState::restore_checkpoint(
            &saved.scheduler,
            self.program,
            &self.limits,
            &saved.heap.managed,
        )?;
        let mut roots = vec![saved.instance];
        classes.append_roots(&mut roots);
        saved.heap.append_roots(&mut roots);
        scheduler.append_roots(&mut roots);
        saved.m3g.append_roots(&mut roots);
        saved.micro3d.append_roots(&mut roots);
        saved.jsr239.append_roots(&mut roots);
        for root in roots {
            saved
                .heap
                .managed
                .validate_checkpoint_root(root)
                .map_err(|_| {
                    vm_error(
                        "checkpoint-root",
                        "The checkpoint contains a stale Java reference.",
                    )
                })?;
        }
        saved.m3g.runtime.validate_checkpoint(
            self.limits.m3g_arena,
            self.limits.m3g_graph_depth,
            self.limits.m3g_max_sprite_crop_dimension,
            |reference| {
                saved
                    .heap
                    .managed
                    .validate_checkpoint_root(Handle::from_raw(reference))
                    .is_ok()
            },
        )?;
        saved
            .m3g
            .graphics
            .renderer
            .validate_checkpoint(self.limits.m3g_render)?;
        saved.micro3d.runtime.validate_checkpoint(
            self.limits.micro3d_objects,
            self.limits.micro3d_bytes,
            self.limits.m3g_render,
            |reference| {
                saved
                    .heap
                    .managed
                    .validate_checkpoint_root(Handle::from_raw(reference))
                    .is_ok()
            },
        )?;
        saved
            .jsr239
            .validate_checkpoint(&saved.heap, &self.limits)?;
        saved.device.last_vibration_request = VibrationRequest::Stop;
        self.execution.instructions = saved.instructions;
        self.classes = classes;
        self.heap = saved.heap;
        self.scheduler = scheduler;
        // Pacing starts at the restored clock; time spent away never becomes
        // catch-up work or immediate expiry of a sleeping Java thread.
        self.scheduler.instruction_pacing_checkpoint = saved.instructions;
        self.scheduler.instruction_pacing_started_millis = self.native_context.monotonic_millis();
        self.device = saved.device;
        self.m3g = saved.m3g;
        self.micro3d = saved.micro3d;
        self.jsr239 = saved.jsr239;
        Ok(saved.instance)
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/checkpoint.rs"]
mod tests;
