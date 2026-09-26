use super::{
    ArrayKind, CallOutcome, EmuError, Machine, Value, int_argument, m3g_non_negative_usize,
    m3g_positive_usize, optional_reference_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn invoke_m3g_mesh_native(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let outcome = match (class, name, descriptor) {
            ("javax/microedition/m3g/Mesh", "getSubmeshCount", "()I") => {
                let handle = self.m3g_receiver(args)?;
                let count = match self.m3g.runtime.kind(handle)? {
                    m3g::ObjectKind::Mesh(mesh)
                    | m3g::ObjectKind::MorphingMesh { mesh, .. }
                    | m3g::ObjectKind::SkinnedMesh { mesh, .. } => mesh.submeshes.len(),
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Int(i32::try_from(count).unwrap_or(i32::MAX))))
            }
            (
                "javax/microedition/m3g/Mesh",
                "getVertexBuffer",
                "()Ljavax/microedition/m3g/VertexBuffer;",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let vertices = match self.m3g.runtime.kind(handle)? {
                    m3g::ObjectKind::Mesh(mesh)
                    | m3g::ObjectKind::MorphingMesh { mesh, .. }
                    | m3g::ObjectKind::SkinnedMesh { mesh, .. } => mesh.vertices,
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Reference(
                    self.m3g_guest_handle(Some(vertices)),
                )))
            }
            ("javax/microedition/m3g/Mesh", "getAppearance" | "getIndexBuffer", _) => {
                let handle = self.m3g_receiver(args)?;
                let index = m3g_non_negative_usize(int_argument(args, 1)?)?;
                let mesh = match self.m3g.runtime.kind(handle)? {
                    m3g::ObjectKind::Mesh(mesh)
                    | m3g::ObjectKind::MorphingMesh { mesh, .. }
                    | m3g::ObjectKind::SkinnedMesh { mesh, .. } => mesh,
                    _ => return Err(type_error()),
                };
                let value = if name == "getAppearance" {
                    *mesh.appearances.get(index).ok_or_else(|| {
                        vm_error(
                            "index-out-of-bounds-exception",
                            "submesh index is out of bounds",
                        )
                    })?
                } else {
                    Some(*mesh.submeshes.get(index).ok_or_else(|| {
                        vm_error(
                            "index-out-of-bounds-exception",
                            "submesh index is out of bounds",
                        )
                    })?)
                };
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(value))))
            }
            (
                "javax/microedition/m3g/Mesh",
                "setAppearance",
                "(ILjavax/microedition/m3g/Appearance;)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let index = m3g_non_negative_usize(int_argument(args, 1)?)?;
                let appearance = optional_reference_argument(args, 2)?
                    .map(|guest| self.m3g_handle(guest))
                    .transpose()?;
                let mesh = match self.m3g.runtime.kind_mut(handle)? {
                    m3g::ObjectKind::Mesh(mesh)
                    | m3g::ObjectKind::MorphingMesh { mesh, .. }
                    | m3g::ObjectKind::SkinnedMesh { mesh, .. } => mesh,
                    _ => return Err(type_error()),
                };
                *mesh.appearances.get_mut(index).ok_or_else(|| {
                    vm_error(
                        "index-out-of-bounds-exception",
                        "submesh index is out of bounds",
                    )
                })? = appearance;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/MorphingMesh", "getMorphTargetCount", "()I") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::MorphingMesh { targets, .. } =
                    self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Int(
                    i32::try_from(targets.len()).unwrap_or(i32::MAX),
                )))
            }
            (
                "javax/microedition/m3g/MorphingMesh",
                "getMorphTarget",
                "(I)Ljavax/microedition/m3g/VertexBuffer;",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let index = m3g_non_negative_usize(int_argument(args, 1)?)?;
                let m3g::ObjectKind::MorphingMesh { targets, .. } =
                    self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                let target = *targets.get(index).ok_or_else(|| {
                    vm_error(
                        "index-out-of-bounds-exception",
                        "morph target index is out of bounds",
                    )
                })?;
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(Some(target)))))
            }
            ("javax/microedition/m3g/MorphingMesh", "setWeights", "([F)V") => {
                let handle = self.m3g_receiver(args)?;
                let source = reference_argument(args, 1)?;
                let m3g::ObjectKind::MorphingMesh { weights, .. } =
                    self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                let mut weights = vec![0.0; weights.len()];
                self.m3g_read_float_array(source, &mut weights)?;
                if !weights.iter().all(|value| value.is_finite()) {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("morph weights must be finite"),
                    );
                }
                let m3g::ObjectKind::MorphingMesh { weights: state, .. } =
                    self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.copy_from_slice(&weights);
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/MorphingMesh", "getWeights", "([F)V") => {
                let handle = self.m3g_receiver(args)?;
                let destination = reference_argument(args, 1)?;
                let m3g::ObjectKind::MorphingMesh { weights, .. } =
                    self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                let weights = weights.clone();
                self.m3g_write_float_array(destination, &weights)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Sprite3D", "isScaled", "()Z") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Sprite3D(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Int(i32::from(state.scaled))))
            }
            (
                "javax/microedition/m3g/Sprite3D",
                "setAppearance",
                "(Ljavax/microedition/m3g/Appearance;)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let appearance = optional_reference_argument(args, 1)?
                    .map(|guest| self.m3g_handle(guest))
                    .transpose()?;
                let m3g::ObjectKind::Sprite3D(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.appearance = appearance;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Sprite3D",
                "getAppearance",
                "()Ljavax/microedition/m3g/Appearance;",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Sprite3D(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Reference(
                    self.m3g_guest_handle(state.appearance),
                )))
            }
            (
                "javax/microedition/m3g/Sprite3D",
                "setImage",
                "(Ljavax/microedition/m3g/Image2D;)V",
            ) => {
                let image = self.m3g_handle(reference_argument(args, 1)?)?;
                let m3g::ObjectKind::Image2D(image_state) = self.m3g.runtime.kind(image)? else {
                    return Err(type_error());
                };
                let crop = m3g::SpriteState::image_crop(
                    image_state,
                    self.limits.m3g_max_sprite_crop_dimension,
                );
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Sprite3D(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.image = image;
                state.crop = crop;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Sprite3D",
                "getImage",
                "()Ljavax/microedition/m3g/Image2D;",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Sprite3D(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Reference(
                    self.m3g_guest_handle(Some(state.image)),
                )))
            }
            ("javax/microedition/m3g/Sprite3D", "setCrop", "(IIII)V") => {
                let crop = [
                    int_argument(args, 1)?,
                    int_argument(args, 2)?,
                    int_argument(args, 3)?,
                    int_argument(args, 4)?,
                ];
                let handle = self.m3g_receiver(args)?;
                m3g::SpriteState::validate_crop(crop, self.limits.m3g_max_sprite_crop_dimension)?;
                let m3g::ObjectKind::Sprite3D(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.crop = crop;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Sprite3D", _, "()I") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Sprite3D(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                let value = match name {
                    "getCropX" => state.crop[0],
                    "getCropY" => state.crop[1],
                    "getCropWidth" => state.crop[2],
                    "getCropHeight" => state.crop[3],
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Int(value)))
            }
            (
                "javax/microedition/m3g/SkinnedMesh",
                "getSkeleton",
                "()Ljavax/microedition/m3g/Group;",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::SkinnedMesh { skeleton, .. } =
                    self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Reference(
                    self.m3g_guest_handle(Some(*skeleton)),
                )))
            }
            (
                "javax/microedition/m3g/SkinnedMesh",
                "addTransform",
                "(Ljavax/microedition/m3g/Node;III)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let bone = self.m3g_handle(reference_argument(args, 1)?)?;
                let weight = u32::try_from(int_argument(args, 2)?).map_err(|_| {
                    vm_error("illegal-argument-exception", "skin weight must be positive")
                })?;
                let first_vertex = usize::try_from(int_argument(args, 3)?).map_err(|_| {
                    vm_error(
                        "index-out-of-bounds-exception",
                        "skin vertex index is negative",
                    )
                })?;
                let vertex_count = m3g_positive_usize(int_argument(args, 4)?)?;
                let skeleton = match self.m3g.runtime.kind(handle)? {
                    m3g::ObjectKind::SkinnedMesh { skeleton, .. } => *skeleton,
                    _ => return Err(type_error()),
                };
                if !self.m3g.runtime.contains_node(skeleton, bone)? {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("bone is not in this mesh skeleton"),
                    );
                }
                let at_rest = self.m3g.runtime.transform_to(handle, bone)?;
                self.m3g_allocate_native(args, |runtime| {
                    runtime.add_skin_transform(
                        handle,
                        bone,
                        at_rest,
                        first_vertex,
                        vertex_count,
                        weight,
                    )
                })?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/SkinnedMesh",
                "getBoneTransform",
                "(Ljavax/microedition/m3g/Node;Ljavax/microedition/m3g/Transform;)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let bone = self.m3g_handle(reference_argument(args, 1)?)?;
                let destination = self.m3g_handle(reference_argument(args, 2)?)?;
                let m3g::ObjectKind::SkinnedMesh {
                    skeleton,
                    bones,
                    bind_transforms,
                    ..
                } = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                if !self.m3g.runtime.contains_node(*skeleton, bone)? {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("node is not in this mesh skeleton"),
                    );
                }
                let transform = bones
                    .iter()
                    .position(|candidate| *candidate == bone)
                    .and_then(|index| bind_transforms.get(index))
                    .copied()
                    .unwrap_or(m3g::Mat4::IDENTITY);
                self.m3g
                    .runtime
                    .set_transform_value(destination, transform)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/SkinnedMesh",
                "getBoneVertices",
                "(Ljavax/microedition/m3g/Node;[I[F)I",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let bone = self.m3g_handle(reference_argument(args, 1)?)?;
                let indices_array = optional_reference_argument(args, 2)?;
                let weights_array = optional_reference_argument(args, 3)?;
                let m3g::ObjectKind::SkinnedMesh {
                    skeleton,
                    bones,
                    influences,
                    ..
                } = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                if !self.m3g.runtime.contains_node(*skeleton, bone)? {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("node is not in this mesh skeleton"),
                    );
                }
                let Some(bone_index) = bones.iter().position(|candidate| *candidate == bone) else {
                    return Ok(CallOutcome::Return(Some(Value::Int(0))));
                };
                let pairs = m3g::SkinInfluence::bone_vertex_weights(
                    influences,
                    bone_index,
                    self.limits.m3g_max_transforms_per_vertex as usize,
                )?;
                let (Some(indices_array), Some(weights_array)) = (indices_array, weights_array)
                else {
                    return Ok(CallOutcome::Return(Some(Value::Int(pairs.count() as i32))));
                };
                let pairs = pairs.collect::<Vec<_>>();
                // Both destinations are validated before either is mutated.
                if self
                    .m3g_primitive_array_elements(indices_array, &ArrayKind::Int)?
                    .len()
                    < pairs.len()
                    || self.m3g_float_array_elements(weights_array, 0)?.len() < pairs.len()
                {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("bone vertex destination array is too short"),
                    );
                }
                let indices = pairs
                    .iter()
                    .map(|(vertex, _)| *vertex as i32)
                    .collect::<Vec<_>>();
                let weights = pairs.iter().map(|(_, weight)| *weight).collect::<Vec<_>>();
                self.m3g_write_int_array(indices_array, &indices)?;
                self.m3g_write_float_array(weights_array, &weights)?;
                CallOutcome::Return(Some(Value::Int(pairs.len() as i32)))
            }
            (
                "javax/microedition/m3g/RayIntersection",
                "getIntersected",
                "()Ljavax/microedition/m3g/Node;",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::RayIntersection(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Reference(
                    self.m3g_guest_handle(state.intersected),
                )))
            }
            ("javax/microedition/m3g/RayIntersection", "getRay", "([F)V") => {
                let handle = self.m3g_receiver(args)?;
                let destination = reference_argument(args, 1)?;
                let m3g::ObjectKind::RayIntersection(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                let ray = state.ray;
                self.m3g_write_float_array(destination, &ray)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/RayIntersection", _, "()F" | "()I" | "(I)F") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::RayIntersection(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                match name {
                    "getDistance" => CallOutcome::Return(Some(Value::Float(state.distance))),
                    "getSubmeshIndex" => CallOutcome::Return(Some(Value::Int(state.submesh))),
                    "getTextureS" | "getTextureT" => {
                        let unit = self.m3g_texture_unit(int_argument(args, 1)?)?;
                        let texture = state.texture.get(unit).ok_or_else(|| {
                            vm_error(
                                "index-out-of-bounds-exception",
                                "texture unit exceeds the active profile limit",
                            )
                        })?;
                        CallOutcome::Return(Some(Value::Float(if name == "getTextureS" {
                            texture[0]
                        } else {
                            texture[1]
                        })))
                    }
                    "getNormalX" => CallOutcome::Return(Some(Value::Float(state.normal.x))),
                    "getNormalY" => CallOutcome::Return(Some(Value::Float(state.normal.y))),
                    "getNormalZ" => CallOutcome::Return(Some(Value::Float(state.normal.z))),
                    _ => return Err(type_error()),
                }
            }
            _ => return self.m3g_unsupported_native(),
        };
        Ok(outcome)
    }
}
