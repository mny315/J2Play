//! Validation and allocation accounting before decoded scene state is published.

use super::{
    ArenaLimits, EmuError, MeshState, Object, ObjectKind, Runtime, estimated_object_bytes,
    graph_error,
};

impl Runtime {
    /// Validates decoded objects and guest references and rebuilds retained byte charges.
    pub fn validate_checkpoint(
        &mut self,
        limits: ArenaLimits,
        max_graph_depth: usize,
        max_sprite_crop: u32,
        mut valid_guest: impl FnMut(u64) -> bool,
    ) -> Result<(), EmuError> {
        self.objects
            .validate_checkpoint(limits, restored_object_bytes)?;
        if self.max_graph_depth != max_graph_depth || self.guest_handles.len() > limits.objects {
            return Err(graph_error(
                "checkpoint-arena",
                "Checkpoint M3G limits do not match the profile",
            ));
        }
        let mut bound_objects = 0;
        let mut references = Vec::new();
        for (handle, object) in self.objects.checkpoint_values() {
            match &object.kind {
                ObjectKind::VertexArray(state) => state.validate_checkpoint()?,
                ObjectKind::VertexBuffer { state, .. } => state.validate_checkpoint()?,
                ObjectKind::TriangleStripArray(state) => state.validate_checkpoint()?,
                ObjectKind::Image2D(state) => state.validate_checkpoint()?,
                ObjectKind::KeyframeSequence(state) => state.validate_checkpoint()?,
                ObjectKind::Background(state) => super::BackgroundState::validate_crop(state.crop)?,
                ObjectKind::Sprite3D(state) => {
                    super::SpriteState::validate_crop(state.crop, max_sprite_crop)?;
                }
                ObjectKind::AnimationTrack { .. } => {
                    self.animation_track_shape(&object.kind)?;
                }
                _ => {}
            }
            self.validate_animation_tracks(object)?;
            if !object
                .guest_reference
                .into_iter()
                .chain(object.base.user_object)
                .all(&mut valid_guest)
            {
                return Err(invalid_guest_binding());
            }
            if let Some(guest) = object.guest_reference {
                if self.guest_handles.get(&guest) != Some(&handle) {
                    return Err(invalid_guest_binding());
                }
                bound_objects += 1;
            }
            object.extend_owned_references(&mut references);
            for reference in references.drain(..) {
                self.objects.get(reference)?;
            }
        }
        if bound_objects != self.guest_handles.len() {
            return Err(invalid_guest_binding());
        }
        Ok(())
    }
}

fn restored_object_bytes(object: &mut Object) -> usize {
    // Vec capacity is not serialized. Discard spare capacity introduced by
    // decoding so a valid checkpoint still fits its original byte budget.
    object.base.animation_tracks.shrink_to_fit();
    match &mut object.kind {
        ObjectKind::Group { children, .. } | ObjectKind::World { children, .. } => {
            children.shrink_to_fit();
        }
        ObjectKind::Mesh(mesh) => compact_mesh(mesh),
        ObjectKind::MorphingMesh {
            mesh,
            targets,
            weights,
        } => {
            compact_mesh(mesh);
            targets.shrink_to_fit();
            weights.shrink_to_fit();
        }
        ObjectKind::SkinnedMesh {
            mesh,
            bones,
            bind_transforms,
            influences,
            ..
        } => {
            compact_mesh(mesh);
            bones.shrink_to_fit();
            bind_transforms.shrink_to_fit();
            influences.shrink_to_fit();
        }
        ObjectKind::KeyframeSequence(state) => state.compact_checkpoint_buffers(),
        // Images and vertex/index arrays use Arc slices, with no spare capacity.
        _ => {}
    }
    estimated_object_bytes(object)
}

fn compact_mesh(mesh: &mut MeshState) {
    mesh.submeshes.shrink_to_fit();
    mesh.appearances.shrink_to_fit();
}

fn invalid_guest_binding() -> EmuError {
    graph_error(
        "checkpoint-arena",
        "Checkpoint M3G guest binding is invalid",
    )
}
