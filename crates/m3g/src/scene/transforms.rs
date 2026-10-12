use super::{
    EmuError, Handle, Mat4, ObjectKind, Quaternion, Runtime, SkinInfluence, Vec3,
    clone_slice_preserving_capacity, graph_error, skin_vector_bytes,
};

impl Runtime {
    /// Sets translation after rejecting non-finite values.
    pub fn set_translation(&mut self, handle: Handle, value: Vec3) -> Result<(), EmuError> {
        Mat4::translation(value.x, value.y, value.z)?;
        self.transformable_mut(handle)?.translation = value;
        Ok(())
    }

    /// Sets scale after rejecting non-finite values.
    pub fn set_scale(&mut self, handle: Handle, value: Vec3) -> Result<(), EmuError> {
        Mat4::scale(value.x, value.y, value.z)?;
        self.transformable_mut(handle)?.scale = value;
        Ok(())
    }

    /// Sets orientation from an angle in degrees and axis.
    pub fn set_orientation(
        &mut self,
        handle: Handle,
        angle: f32,
        axis: Vec3,
    ) -> Result<(), EmuError> {
        self.transformable_mut(handle)?.orientation = Quaternion::from_axis_angle(angle, axis)?;
        Ok(())
    }

    /// Sets a normalized quaternion orientation for animation tracks.
    pub fn set_orientation_quaternion(
        &mut self,
        handle: Handle,
        value: Quaternion,
    ) -> Result<(), EmuError> {
        self.transformable_mut(handle)?.orientation = value.normalized()?;
        Ok(())
    }

    /// Prepends or appends an axis-angle rotation to current orientation.
    pub fn rotate(
        &mut self,
        handle: Handle,
        angle: f32,
        axis: Vec3,
        prepend: bool,
    ) -> Result<(), EmuError> {
        let rotation = Quaternion::from_axis_angle(angle, axis)?;
        let state = self.transformable_mut(handle)?;
        state.orientation = if prepend {
            rotation.multiplied(state.orientation)?
        } else {
            state.orientation.multiplied(rotation)?
        };
        Ok(())
    }

    /// Adds a relative translation.
    pub fn translate(&mut self, handle: Handle, value: Vec3) -> Result<(), EmuError> {
        Mat4::translation(value.x, value.y, value.z)?;
        let state = self.transformable_mut(handle)?;
        let next = Vec3::new(
            state.translation.x + value.x,
            state.translation.y + value.y,
            state.translation.z + value.z,
        );
        Mat4::translation(next.x, next.y, next.z)?;
        state.translation = next;
        Ok(())
    }

    /// Multiplies current scale component-wise.
    pub fn scale_by(&mut self, handle: Handle, value: Vec3) -> Result<(), EmuError> {
        Mat4::scale(value.x, value.y, value.z)?;
        let state = self.transformable_mut(handle)?;
        let next = Vec3::new(
            state.scale.x * value.x,
            state.scale.y * value.y,
            state.scale.z * value.z,
        );
        Mat4::scale(next.x, next.y, next.z)?;
        state.scale = next;
        Ok(())
    }

    /// Returns translation, scale and normalized orientation.
    pub fn transform_components(
        &self,
        handle: Handle,
    ) -> Result<(Vec3, Vec3, Quaternion), EmuError> {
        let state = self.transformable(handle)?;
        Ok((state.translation, state.scale, state.orientation))
    }

    /// Returns the general transform component only.
    pub fn general_transform(&self, handle: Handle) -> Result<Mat4, EmuError> {
        Ok(self.transformable(handle)?.transform)
    }

    /// Sets the general transform component, requiring an affine matrix for Nodes.
    pub fn set_transform(&mut self, handle: Handle, transform: Mat4) -> Result<(), EmuError> {
        if self.objects.get(handle)?.kind.node().is_some()
            && transform.to_row_major()[12..] != [0.0, 0.0, 0.0, 1.0]
        {
            return Err(graph_error(
                "invalid-transform",
                "Node transform must have a (0, 0, 0, 1) bottom row",
            ));
        }
        self.transformable_mut(handle)?.transform = transform;
        Ok(())
    }

    /// Computes `translation * rotation * scale * general`.
    pub fn composite_transform(&self, handle: Handle) -> Result<Mat4, EmuError> {
        self.transformable(handle)?.composite()
    }

    /// Returns a standalone `Transform` matrix.
    pub fn transform_value(&self, handle: Handle) -> Result<Mat4, EmuError> {
        match self.objects.get(handle)?.kind {
            ObjectKind::Transform(value) => Ok(value),
            _ => Err(graph_error("not-transform", "object is not a Transform")),
        }
    }

    /// Replaces a standalone `Transform` matrix.
    pub fn set_transform_value(&mut self, handle: Handle, value: Mat4) -> Result<(), EmuError> {
        match &mut self.objects.get_mut(handle)?.kind {
            ObjectKind::Transform(current) => *current = value,
            _ => return Err(graph_error("not-transform", "object is not a Transform")),
        }
        Ok(())
    }

    /// Computes a node-to-node transform without recursion.
    pub fn transform_to(&self, source: Handle, target: Handle) -> Result<Mat4, EmuError> {
        let source = self.node_chain(source)?;
        let target = self.node_chain(target)?;
        let shared = source
            .iter()
            .rev()
            .zip(target.iter().rev())
            .take_while(|(source, target)| source == target)
            .count();
        if shared == 0 {
            return Err(graph_error(
                "no-path",
                "nodes do not belong to the same scene graph",
            ));
        }
        // Ancestors above the shared coordinate system cancel. Their matrices
        // may be singular even when the requested relative transform is valid.
        let source = self.transform_chain(&source[..source.len() - shared])?;
        let target = self.transform_chain(&target[..target.len() - shared])?;
        Ok(target.inverted()?.multiplied(source))
    }

    /// Attaches the implicit skeleton child and captures every bone's at-rest transform.
    pub fn bind_skin_skeleton(&mut self, skin: Handle) -> Result<(), EmuError> {
        let (skeleton, bones) = match &self.objects.get(skin)?.kind {
            ObjectKind::SkinnedMesh {
                skeleton, bones, ..
            } => (*skeleton, bones),
            _ => {
                return Err(graph_error(
                    "not-skinned-mesh",
                    "object is not a SkinnedMesh",
                ));
            }
        };
        if !matches!(self.objects.get(skeleton)?.kind, ObjectKind::Group { .. }) {
            return Err(graph_error(
                "invalid-skeleton",
                "SkinnedMesh skeleton is not a Group",
            ));
        }
        if self.parent(skeleton)?.is_some() {
            return Err(graph_error(
                "already-parented",
                "SkinnedMesh skeleton already has a parent",
            ));
        }
        self.validate_parent_link(skin, skeleton)?;

        // The skeleton is an implicit child of the SkinnedMesh. Compute the
        // bind pose before installing that edge: the skeleton's local matrix
        // is its transform into the mesh coordinate system.
        let skeleton_to_skin = self.composite_transform(skeleton)?;
        let mut bind_transforms = Vec::with_capacity(bones.len());
        for bone in bones {
            if !self.contains_node(skeleton, *bone)? {
                return Err(graph_error(
                    "invalid-skeleton",
                    "skin bone is not in the skeleton group",
                ));
            }
            let bone_to_skeleton = self.transform_to(*bone, skeleton)?;
            bind_transforms.push(skeleton_to_skin.multiplied(bone_to_skeleton).inverted()?);
        }

        let previous_allocation = match &self.objects.get(skin)?.kind {
            ObjectKind::SkinnedMesh {
                bind_transforms, ..
            } => bind_transforms
                .capacity()
                .saturating_mul(std::mem::size_of::<Mat4>()),
            _ => unreachable!("validated SkinnedMesh changed kind"),
        };
        let next_allocation = bind_transforms
            .capacity()
            .saturating_mul(std::mem::size_of::<Mat4>());
        self.reaccount_capacity_change(skin, previous_allocation, next_allocation)?;
        let ObjectKind::SkinnedMesh {
            bind_transforms: stored,
            ..
        } = &mut self
            .objects
            .get_mut(skin)
            .expect("reaccounted SkinnedMesh remains live")
            .kind
        else {
            unreachable!("validated SkinnedMesh changed kind");
        };
        *stored = bind_transforms;
        self.objects
            .get_mut(skeleton)?
            .kind
            .node_mut()
            .expect("validated skeleton group")
            .parent = Some(skin);
        Ok(())
    }

    /// Adds one validated skin range through a single transactional candidate.
    pub fn add_skin_transform(
        &mut self,
        skin: Handle,
        bone: Handle,
        at_rest: Mat4,
        first_vertex: usize,
        vertex_count: usize,
        weight: u32,
    ) -> Result<(), EmuError> {
        if weight == 0 || vertex_count == 0 {
            return Err(graph_error(
                "invalid-skinning",
                "skin weight and vertex count must be positive",
            ));
        }
        if first_vertex
            .checked_add(vertex_count)
            .is_none_or(|end| end > usize::from(u16::MAX))
        {
            return Err(graph_error(
                "vertex-bounds",
                "skin vertex range exceeds 65535",
            ));
        }
        let (bone_index, append_bone, has_spare_capacity) = match &self.objects.get(skin)?.kind {
            ObjectKind::SkinnedMesh {
                bones,
                bind_transforms,
                influences,
                ..
            } => {
                let existing = bones.iter().position(|value| *value == bone);
                let bone_index = existing.unwrap_or(bones.len());
                let append_bone = existing.is_none();
                let bone_capacity = !append_bone
                    || bones.len() < bones.capacity()
                        && bind_transforms.len() < bind_transforms.capacity();
                (
                    bone_index,
                    append_bone,
                    bone_capacity && influences.len() < influences.capacity(),
                )
            }
            _ => {
                return Err(graph_error(
                    "not-skinned-mesh",
                    "object is not a SkinnedMesh",
                ));
            }
        };
        let influence = SkinInfluence {
            bone: bone_index,
            first_vertex,
            vertex_count,
            weight,
        };
        if has_spare_capacity {
            let ObjectKind::SkinnedMesh {
                bones,
                bind_transforms,
                influences,
                ..
            } = &mut self.objects.get_mut(skin)?.kind
            else {
                unreachable!("validated SkinnedMesh changed kind");
            };
            if append_bone {
                bones.push(bone);
                bind_transforms.push(at_rest);
            }
            influences.push(influence);
            return Ok(());
        }

        let (mut bones, mut bind_transforms, mut influences, previous_allocation) =
            match &self.objects.get(skin)?.kind {
                ObjectKind::SkinnedMesh {
                    bones,
                    bind_transforms,
                    influences,
                    ..
                } => (
                    clone_slice_preserving_capacity(bones, bones.capacity()),
                    clone_slice_preserving_capacity(bind_transforms, bind_transforms.capacity()),
                    clone_slice_preserving_capacity(influences, influences.capacity()),
                    skin_vector_bytes(
                        bones.capacity(),
                        bind_transforms.capacity(),
                        influences.capacity(),
                    ),
                ),
                _ => {
                    return Err(graph_error(
                        "not-skinned-mesh",
                        "object is not a SkinnedMesh",
                    ));
                }
            };
        if append_bone {
            bones.push(bone);
            bind_transforms.push(at_rest);
        }
        influences.push(influence);
        let next_allocation = skin_vector_bytes(
            bones.capacity(),
            bind_transforms.capacity(),
            influences.capacity(),
        );
        self.reaccount_capacity_change(skin, previous_allocation, next_allocation)?;
        let ObjectKind::SkinnedMesh {
            bones: stored_bones,
            bind_transforms: stored_bind_transforms,
            influences: stored_influences,
            ..
        } = &mut self
            .objects
            .get_mut(skin)
            .expect("reaccounted SkinnedMesh remains live")
            .kind
        else {
            unreachable!("validated SkinnedMesh changed kind");
        };
        *stored_bones = bones;
        *stored_bind_transforms = bind_transforms;
        *stored_influences = influences;
        Ok(())
    }
}
