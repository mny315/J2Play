use super::{
    Arena, ArenaLimits, BTreeMap, BTreeSet, EmuError, Handle, Object, ObjectKind, ObjectState,
    Runtime, estimated_object_bytes, graph_error,
};

impl Runtime {
    #[must_use]
    pub fn new(limits: ArenaLimits) -> Self {
        Self::new_with_graph_depth(limits, usize::MAX)
    }

    /// Creates an isolated runtime with an explicit scene-graph depth budget.
    #[must_use]
    pub fn new_with_graph_depth(limits: ArenaLimits, max_graph_depth: usize) -> Self {
        Self {
            objects: Arena::new(limits),
            guest_handles: BTreeMap::new(),
            max_graph_depth: max_graph_depth.max(1),
        }
    }

    /// Creates a native object and optionally associates it with a guest reference.
    pub fn create(
        &mut self,
        guest_reference: Option<u64>,
        kind: ObjectKind,
    ) -> Result<Handle, EmuError> {
        if let Some(reference) = guest_reference
            && self.guest_handles.contains_key(&reference)
        {
            return Err(graph_error(
                "duplicate-guest-object",
                "guest object already owns M3G state",
            ));
        }
        match &kind {
            ObjectKind::AnimationTrack { .. } => {
                self.animation_track_shape(&kind)?;
            }
            ObjectKind::Background(state) => super::BackgroundState::validate_crop(state.crop)?,
            _ => {}
        }
        let object = Object {
            guest_reference,
            base: ObjectState::default(),
            kind,
        };
        let bytes = estimated_object_bytes(&object);
        let handle = self.objects.insert(object, bytes)?;
        if let Some(reference) = guest_reference {
            self.guest_handles.insert(reference, handle);
        }
        Ok(handle)
    }

    /// Returns the stable set copied by `Object3D.duplicate`.
    pub fn duplicate_sources(&self, root: Handle) -> Result<Vec<Handle>, EmuError> {
        let root_object = self.objects.get(root)?;
        if root_object.kind.node().is_none() {
            return Ok(vec![root]);
        }
        let mut output = Vec::new();
        let mut visited = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(handle) = pending.pop() {
            if !visited.insert(handle) {
                continue;
            }
            let object = self.objects.get(handle)?;
            if object.kind.node().is_none() {
                continue;
            }
            output.push(handle);
            if let ObjectKind::SkinnedMesh { skeleton, .. } = &object.kind {
                pending.push(*skeleton);
            }
            pending.extend(
                object
                    .kind
                    .children()
                    .unwrap_or_default()
                    .iter()
                    .rev()
                    .copied(),
            );
        }
        Ok(output)
    }

    /// Copies one object's complete native state before duplicate-reference remapping.
    pub fn create_duplicate(
        &mut self,
        source: Handle,
        guest_reference: u64,
    ) -> Result<Handle, EmuError> {
        if self.guest_handles.contains_key(&guest_reference) {
            return Err(graph_error(
                "duplicate-guest-object",
                "guest object already owns M3G state",
            ));
        }
        let mut object = self.objects.get(source)?.clone();
        object.guest_reference = Some(guest_reference);
        let bytes = estimated_object_bytes(&object);
        let duplicate = self.objects.insert(object, bytes)?;
        self.guest_handles.insert(guest_reference, duplicate);
        Ok(duplicate)
    }

    /// Rewrites copied Node references and parent links after an atomic duplicate allocation.
    pub fn finish_duplicate(
        &mut self,
        root: Handle,
        mapping: &BTreeMap<Handle, Handle>,
    ) -> Result<(), EmuError> {
        for (source, duplicate) in mapping {
            let source_parent = self
                .objects
                .get(*source)?
                .kind
                .node()
                .and_then(|node| node.parent);
            let object = self.objects.get_mut(*duplicate)?;
            if let Some(node) = object.kind.node_mut() {
                node.parent = if *source == root {
                    None
                } else {
                    source_parent.and_then(|parent| mapping.get(&parent).copied())
                };
                for (reference, _) in &mut node.alignment {
                    if let Some(mapped) = reference.and_then(|value| mapping.get(&value).copied()) {
                        *reference = Some(mapped);
                    }
                }
            }
            match &mut object.kind {
                ObjectKind::Group { children, .. } => {
                    for child in children {
                        *child = mapping.get(child).copied().unwrap_or(*child);
                    }
                }
                ObjectKind::World {
                    children, camera, ..
                } => {
                    for child in children {
                        *child = mapping.get(child).copied().unwrap_or(*child);
                    }
                    if let Some(mapped) = camera.and_then(|value| mapping.get(&value).copied()) {
                        *camera = Some(mapped);
                    }
                }
                ObjectKind::SkinnedMesh {
                    skeleton, bones, ..
                } => {
                    *skeleton = mapping.get(skeleton).copied().unwrap_or(*skeleton);
                    for bone in bones {
                        *bone = mapping.get(bone).copied().unwrap_or(*bone);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Resolves a guest reference only when it has a native binding.
    pub fn resolve_guest(&self, reference: u64) -> Result<Handle, EmuError> {
        self.guest_handles
            .get(&reference)
            .copied()
            .ok_or_else(|| graph_error("unbound-guest-object", "guest object has no M3G state"))
    }

    /// Resolves a native handle back to its guest object reference.
    #[must_use]
    pub fn guest_reference(&self, handle: Handle) -> Option<u64> {
        self.objects.get(handle).ok()?.guest_reference
    }

    /// Expands rooted guest objects through native M3G ownership edges.
    #[must_use]
    pub fn guest_closure(&self, roots: impl IntoIterator<Item = u64>) -> Vec<u64> {
        let mut pending = roots
            .into_iter()
            .filter_map(|reference| self.guest_handles.get(&reference).copied())
            .collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        let mut guests = BTreeSet::new();
        while let Some(handle) = pending.pop() {
            if !visited.insert(handle) {
                continue;
            }
            let Ok(object) = self.objects.get(handle) else {
                continue;
            };
            if let Some(reference) = object.guest_reference {
                guests.insert(reference);
            }
            if let Some(reference) = object.base.user_object {
                guests.insert(reference);
            }
            object.extend_owned_references(&mut pending);
        }
        guests.into_iter().collect()
    }

    /// Sweeps native objects after guest GC has invalidated their owner objects.
    pub fn sweep_guest_objects(&mut self, mut is_live: impl FnMut(u64) -> bool) {
        self.guest_handles.retain(|guest, handle| {
            if is_live(*guest) {
                return true;
            }
            let _ = self.objects.remove(*handle);
            false
        });
    }

    /// Returns the typed native state after handle validation.
    pub fn kind(&self, handle: Handle) -> Result<&ObjectKind, EmuError> {
        Ok(&self.objects.get(handle)?.kind)
    }

    /// Returns mutable state without changing its accounted retained size.
    /// Dynamic buffers must grow through runtime setters that reaccount them.
    pub fn kind_mut(&mut self, handle: Handle) -> Result<&mut ObjectKind, EmuError> {
        Ok(&mut self.objects.get_mut(handle)?.kind)
    }

    pub(super) fn reaccount_capacity_change(
        &mut self,
        handle: Handle,
        previous_allocation: usize,
        next_allocation: usize,
    ) -> Result<(), EmuError> {
        let current = estimated_object_bytes(self.objects.get(handle)?);
        let next = current
            .checked_sub(previous_allocation)
            .and_then(|bytes| bytes.checked_add(next_allocation))
            .ok_or_else(|| graph_error("resource-limit", "M3G allocation accounting overflow"))?;
        self.objects.reaccount(handle, next)
    }
}
