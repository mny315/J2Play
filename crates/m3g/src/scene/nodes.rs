use super::{
    BTreeSet, EmuError, Handle, Object, ObjectKind, Quaternion, Runtime, Vec3, graph_error,
    mesh_references, shortest_alignment_rotation,
};

impl Runtime {
    /// Applies validated node flags and alpha.
    pub fn set_node_state(
        &mut self,
        handle: Handle,
        rendering_enabled: bool,
        picking_enabled: bool,
        scope: u32,
        alpha: f32,
    ) -> Result<(), EmuError> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(graph_error(
                "invalid-alpha",
                "alpha must be finite and in [0, 1]",
            ));
        }
        let node = self
            .objects
            .get_mut(handle)?
            .kind
            .node_mut()
            .ok_or_else(|| graph_error("not-node", "object is not a Node"))?;
        node.rendering_enabled = rendering_enabled;
        node.picking_enabled = picking_enabled;
        node.scope = scope;
        node.alpha = alpha;
        Ok(())
    }

    /// Returns node state `(rendering, picking, scope, alpha)`.
    pub fn node_state(&self, handle: Handle) -> Result<(bool, bool, u32, f32), EmuError> {
        let node = self
            .objects
            .get(handle)?
            .kind
            .node()
            .ok_or_else(|| graph_error("not-node", "object is not a Node"))?;
        Ok((
            node.rendering_enabled,
            node.picking_enabled,
            node.scope,
            node.alpha,
        ))
    }

    /// Sets one alignment constraint. `axis` is `0` for Z and `1` for Y.
    pub fn set_alignment(
        &mut self,
        handle: Handle,
        axis: usize,
        reference: Option<Handle>,
        target: i32,
    ) -> Result<(), EmuError> {
        if axis > 1 || !(144..=148).contains(&target) {
            return Err(graph_error(
                "invalid-alignment",
                "alignment axis or target is outside the JSR range",
            ));
        }
        if let Some(reference) = reference
            && self.objects.get(reference)?.kind.node().is_none()
        {
            return Err(graph_error("not-node", "alignment reference is not a Node"));
        }
        let node = self
            .objects
            .get_mut(handle)?
            .kind
            .node_mut()
            .ok_or_else(|| graph_error("not-node", "object is not a Node"))?;
        if reference == Some(handle) {
            return Err(graph_error(
                "invalid-alignment",
                "a node cannot use itself as an alignment reference",
            ));
        }
        node.alignment[axis] = (reference, target);
        Ok(())
    }

    /// Atomically replaces both alignment constraints after pair validation.
    pub fn set_alignments(
        &mut self,
        handle: Handle,
        z_reference: Option<Handle>,
        z_target: i32,
        y_reference: Option<Handle>,
        y_target: i32,
    ) -> Result<(), EmuError> {
        if !(144..=148).contains(&z_target) || !(144..=148).contains(&y_target) {
            return Err(graph_error(
                "invalid-alignment",
                "alignment target is outside the JSR range",
            ));
        }
        if z_reference == Some(handle) || y_reference == Some(handle) {
            return Err(graph_error(
                "invalid-alignment",
                "a node cannot use itself as an alignment reference",
            ));
        }
        if z_target != 144 && z_reference == y_reference && z_target == y_target {
            return Err(graph_error(
                "invalid-alignment",
                "the Y and Z axes cannot share the same active alignment target",
            ));
        }
        for reference in [z_reference, y_reference].into_iter().flatten() {
            if self.objects.get(reference)?.kind.node().is_none() {
                return Err(graph_error("not-node", "alignment reference is not a Node"));
            }
        }
        let node = self
            .objects
            .get_mut(handle)?
            .kind
            .node_mut()
            .ok_or_else(|| graph_error("not-node", "object is not a Node"))?;
        node.alignment = [(z_reference, z_target), (y_reference, y_target)];
        Ok(())
    }

    /// Returns one alignment constraint. `axis` is `0` for Z and `1` for Y.
    pub fn alignment(
        &self,
        handle: Handle,
        axis: usize,
    ) -> Result<(Option<Handle>, i32), EmuError> {
        if axis > 1 {
            return Err(graph_error(
                "invalid-alignment",
                "alignment axis is outside the JSR range",
            ));
        }
        self.objects
            .get(handle)?
            .kind
            .node()
            .map(|node| node.alignment[axis])
            .ok_or_else(|| graph_error("not-node", "object is not a Node"))
    }

    /// Applies this node's alignment constraints and those of its descendants.
    /// Traversal and all ancestry checks are iterative and arena-bounded.
    pub fn align_subtree(
        &mut self,
        handle: Handle,
        runtime_reference: Option<Handle>,
    ) -> Result<(), EmuError> {
        let root = self.graph_root(handle)?;
        if let Some(reference) = runtime_reference
            && self.graph_root(reference)? != root
        {
            return Err(graph_error(
                "invalid-alignment",
                "runtime alignment reference is in a different scene graph",
            ));
        }
        let common_reference = runtime_reference.unwrap_or(handle);
        let mut stack = vec![handle];
        let mut visited = BTreeSet::new();
        let mut updates = Vec::new();
        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                return Err(graph_error(
                    "graph-cycle",
                    "alignment subtree contains a repeated node",
                ));
            }
            let node = self
                .objects
                .get(current)?
                .kind
                .node()
                .ok_or_else(|| graph_error("not-node", "object is not a Node"))?;
            if let Some(children) = self.objects.get(current)?.kind.children() {
                stack.extend(children.iter().rev().copied());
            }
            let constraints = node.alignment;
            if constraints[0].1 == 144 && constraints[1].1 == 144 {
                continue;
            }
            let resolve_reference = |constraint: (Option<Handle>, i32)| {
                (constraint.1 != 144).then_some(constraint.0.unwrap_or(common_reference))
            };
            let z_reference = resolve_reference(constraints[0]);
            let y_reference = resolve_reference(constraints[1]);
            for reference in [z_reference, y_reference].into_iter().flatten() {
                if self.graph_root(reference)? != root
                    || self.is_descendant_or_self(current, reference)?
                {
                    return Err(graph_error(
                        "invalid-alignment-state",
                        "alignment reference is self, a descendant, or in another graph",
                    ));
                }
            }
            // Alignment starts from translation alone and replaces the old orientation.
            let translation = node.transformable.translation;
            let direction = |reference: Handle, target: i32| -> Result<Vec3, EmuError> {
                // Shared ancestors cancel before any matrix arithmetic. Going
                // through world space would require their inverse and lose
                // local distances under large common translations.
                let reference_to_parent = match node.parent {
                    Some(parent) => self.transform_to(reference, parent)?,
                    None => self.world_transform(reference)?,
                };
                let local = if target == 145 {
                    let origin =
                        reference_to_parent.transform(crate::Vec4::new(0.0, 0.0, 0.0, 1.0));
                    crate::Vec4::new(
                        origin.x - translation.x,
                        origin.y - translation.y,
                        origin.z - translation.z,
                        0.0,
                    )
                } else {
                    let axis = match target {
                        146 => crate::Vec4::new(1.0, 0.0, 0.0, 0.0),
                        147 => crate::Vec4::new(0.0, 1.0, 0.0, 0.0),
                        148 => crate::Vec4::new(0.0, 0.0, 1.0, 0.0),
                        _ => unreachable!("validated alignment target"),
                    };
                    reference_to_parent.transform(axis)
                };
                Vec3::new(local.x, local.y, local.z).normalized()
            };
            let desired_z = z_reference
                .map(|reference| direction(reference, constraints[0].1))
                .transpose()?;
            let desired_y = y_reference
                .map(|reference| direction(reference, constraints[1].1))
                .transpose()?;
            let orientation = if let (Some(z), Some(seed)) = (desired_z, desired_y) {
                let y = Vec3::new(
                    seed.x - z.x * seed.dot(z),
                    seed.y - z.y * seed.dot(z),
                    seed.z - z.z * seed.dot(z),
                )
                .normalized()?;
                let x = y.cross(z).normalized()?;
                Quaternion::from_basis(x, z.cross(x).normalized()?, z)?
            } else if let Some(z) = desired_z {
                shortest_alignment_rotation(Vec3::new(0.0, 0.0, 1.0), z)?
            } else {
                let y = desired_y.expect("at least one active alignment");
                shortest_alignment_rotation(Vec3::new(0.0, 1.0, 0.0), y)?
            };
            updates.push((current, orientation));
        }
        for (node, orientation) in updates {
            self.transformable_mut(node)?.orientation = orientation;
        }
        Ok(())
    }

    pub(super) fn graph_root(&self, mut node: Handle) -> Result<Handle, EmuError> {
        for _ in 0..=self.objects.live_objects() {
            match self.parent(node)? {
                Some(parent) => node = parent,
                None => return Ok(node),
            }
        }
        Err(graph_error("cycle", "cycle in Node parent chain"))
    }

    pub(super) fn is_descendant_or_self(
        &self,
        ancestor: Handle,
        mut candidate: Handle,
    ) -> Result<bool, EmuError> {
        for _ in 0..=self.objects.live_objects() {
            if candidate == ancestor {
                return Ok(true);
            }
            match self.parent(candidate)? {
                Some(parent) => candidate = parent,
                None => return Ok(false),
            }
        }
        Err(graph_error("cycle", "cycle in Node parent chain"))
    }

    /// Returns whether `candidate` is `ancestor` or one of its descendants.
    pub fn contains_node(&self, ancestor: Handle, candidate: Handle) -> Result<bool, EmuError> {
        if self.objects.get(ancestor)?.kind.node().is_none()
            || self.objects.get(candidate)?.kind.node().is_none()
        {
            return Err(graph_error("not-node", "containment requires Node objects"));
        }
        self.is_descendant_or_self(ancestor, candidate)
    }

    /// Returns direct references visible to `Object3D.getReferences`.
    pub fn references(&self, handle: Handle) -> Result<Vec<Handle>, EmuError> {
        let mut references = Vec::new();
        self.extend_references(handle, &mut references)?;
        Ok(references)
    }

    /// Appends public references in their stable order to an existing traversal buffer.
    pub fn extend_references(
        &self,
        handle: Handle,
        references: &mut Vec<Handle>,
    ) -> Result<(), EmuError> {
        self.objects.get(handle)?.extend_references(references);
        Ok(())
    }

    /// Finds the first reachable object with the requested user ID using stable DFS order.
    pub fn find(&self, root: Handle, user_id: i32) -> Result<Option<Handle>, EmuError> {
        let mut pending = vec![root];
        let mut seen = BTreeSet::new();
        while let Some(handle) = pending.pop() {
            if !seen.insert(handle) {
                continue;
            }
            if self.user_id(handle)? == user_id {
                return Ok(Some(handle));
            }
            let start = pending.len();
            self.extend_references(handle, &mut pending)?;
            pending[start..].reverse();
        }
        Ok(None)
    }
}

impl Object {
    pub(super) fn extend_owned_references(&self, references: &mut Vec<Handle>) {
        self.extend_references(references);
        // Public getReferences omits parent and alignment links, but their
        // getters still expose Java wrappers. GC and restore validate both.
        if let Some(node) = self.kind.node() {
            references.extend(node.parent);
            references.extend(
                node.alignment
                    .iter()
                    .filter_map(|(reference, _)| *reference),
            );
        }
    }

    fn extend_references(&self, references: &mut Vec<Handle>) {
        references.extend(&self.base.animation_tracks);
        match &self.kind {
            ObjectKind::Group { children, .. } => references.extend(children),
            ObjectKind::World {
                children,
                camera,
                background,
                ..
            } => {
                references.extend(children);
                references.extend(camera);
                references.extend(background);
            }
            ObjectKind::Background(background) => references.extend(background.image),
            ObjectKind::VertexBuffer { arrays, .. } => references.extend(arrays.iter().flatten()),
            ObjectKind::Appearance(appearance) => {
                references.extend(appearance.fog);
                references.extend(appearance.polygon_mode);
                references.extend(appearance.compositing_mode);
                references.extend(appearance.material);
                references.extend(appearance.textures.iter().flatten());
            }
            ObjectKind::Texture2D(texture) => references.push(texture.image),
            ObjectKind::Mesh(mesh) => mesh_references(mesh, references),
            ObjectKind::MorphingMesh { mesh, targets, .. } => {
                mesh_references(mesh, references);
                references.extend(targets);
            }
            ObjectKind::SkinnedMesh {
                mesh,
                skeleton,
                bones,
                ..
            } => {
                mesh_references(mesh, references);
                references.push(*skeleton);
                references.extend(bones);
            }
            ObjectKind::Sprite3D(sprite) => {
                references.push(sprite.image);
                references.extend(sprite.appearance);
            }
            ObjectKind::AnimationTrack {
                sequence,
                controller,
                ..
            } => {
                references.push(*sequence);
                references.extend(controller);
            }
            _ => {}
        }
    }
}
