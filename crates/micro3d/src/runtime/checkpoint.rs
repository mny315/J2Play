//! Validation and allocation accounting for decoded native state.

use super::{EmuError, ObjectKind, RenderLimits, Runtime, estimated_bytes, runtime_error};
use crate::ActionSegmentData;

impl Runtime {
    /// Validates decoded state and rebuilds allocation charges before publishing it.
    /// Guest references include disposed wrappers and ordinary Java endpoints.
    pub fn validate_checkpoint(
        &mut self,
        max_objects: usize,
        max_bytes: usize,
        limits: RenderLimits,
        mut valid_guest: impl FnMut(u64) -> bool,
    ) -> Result<(), EmuError> {
        if self.max_objects != max_objects
            || self.max_bytes != max_bytes
            || self.objects.len() > max_bytes / size_of::<ObjectKind>()
            || self.render_limits != limits
        {
            return Err(invalid_arena());
        }
        self.renderer.validate_checkpoint(limits)?;
        let mut live_bytes = 0_usize;
        let mut live_objects = 0;
        for (&guest, object) in &mut self.objects {
            if !valid_guest(guest) {
                return Err(invalid_arena());
            }
            let Some(kind) = &mut object.kind else {
                object.bytes = 0;
                continue;
            };
            if !kind.guest_references().all(&mut valid_guest) {
                return Err(invalid_arena());
            }
            // Vec capacity is not serialized. Remove spare capacity introduced
            // by decoding before charging the actual retained allocation.
            compact_buffers(kind);
            object.bytes = estimated_bytes(kind);
            live_bytes = live_bytes
                .checked_add(object.bytes)
                .filter(|bytes| *bytes <= max_bytes)
                .ok_or_else(invalid_arena)?;
            live_objects += 1;
            if live_objects > max_objects {
                return Err(invalid_arena());
            }
        }
        // Serialized counters are derived data, not evidence of allocation size.
        self.metrics.live_bytes = live_bytes;
        self.metrics.live_objects = live_objects;
        self.metrics.peak_bytes = self.metrics.peak_bytes.max(live_bytes);
        self.metrics.peak_objects = self.metrics.peak_objects.max(live_objects);
        Ok(())
    }
}

fn compact_buffers(kind: &mut ObjectKind) {
    match kind {
        ObjectKind::Figure(state) => {
            state.data.vertices.shrink_to_fit();
            state.data.normals.shrink_to_fit();
            state.data.faces.shrink_to_fit();
            state.data.bones.shrink_to_fit();
            state.textures.shrink_to_fit();
        }
        ObjectKind::Action(table) => {
            table.frame_counts.shrink_to_fit();
            table.actions.shrink_to_fit();
            for action in &mut table.actions {
                action.segments.shrink_to_fit();
                action.pattern_keys.shrink_to_fit();
                for segment in &mut action.segments {
                    if let ActionSegmentData::Components {
                        translation,
                        scale,
                        rotation,
                        roll,
                    } = segment
                    {
                        translation.shrink_to_fit();
                        scale.shrink_to_fit();
                        rotation.shrink_to_fit();
                        roll.shrink_to_fit();
                    }
                }
            }
        }
        ObjectKind::Layout(state) => state.affines.shrink_to_fit(),
        _ => {}
    }
}

fn invalid_arena() -> EmuError {
    runtime_error(
        "checkpoint-arena",
        "Checkpoint Micro3D limits or guest references are invalid",
    )
}
