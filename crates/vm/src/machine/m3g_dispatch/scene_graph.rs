use super::{
    CallOutcome, EmuError, HeapValue, M3gSpritePickContext, Machine, Value, float_argument,
    int_argument, optional_reference_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn invoke_m3g_scene_graph_native(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let outcome = match (class, name, descriptor) {
            ("javax/microedition/m3g/Transform", "setIdentity", "()V") => {
                let handle = self.m3g_receiver(args)?;
                self.m3g
                    .runtime
                    .set_transform_value(handle, m3g::Mat4::IDENTITY)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Transform",
                "set" | "postMultiply",
                "(Ljavax/microedition/m3g/Transform;)V",
            ) => {
                let destination = self.m3g_receiver(args)?;
                let source = self.m3g_handle(reference_argument(args, 1)?)?;
                let right = self.m3g.runtime.transform_value(source)?;
                let value = if name == "postMultiply" {
                    self.m3g
                        .runtime
                        .transform_value(destination)?
                        .multiplied(right)
                } else {
                    right
                };
                self.m3g.runtime.set_transform_value(destination, value)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transform", "set", "([F)V") => {
                let destination = self.m3g_receiver(args)?;
                let mut array = [0.0; 16];
                self.m3g_read_float_array(reference_argument(args, 1)?, &mut array)?;
                self.m3g
                    .runtime
                    .set_transform_value(destination, m3g::Mat4::from_row_major(array)?)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transform", "get", "([F)V") => {
                let source = self.m3g_receiver(args)?;
                let destination = reference_argument(args, 1)?;
                let values = self.m3g.runtime.transform_value(source)?.to_row_major();
                self.m3g_write_float_array(destination, &values)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transform", "invert", "()V") => {
                let handle = self.m3g_receiver(args)?;
                let value = self.m3g.runtime.transform_value(handle)?.inverted()?;
                self.m3g.runtime.set_transform_value(handle, value)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transform", "transpose", "()V") => {
                let handle = self.m3g_receiver(args)?;
                let value = self.m3g.runtime.transform_value(handle)?.transposed();
                self.m3g.runtime.set_transform_value(handle, value)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transform", "postScale", "(FFF)V") => {
                let handle = self.m3g_receiver(args)?;
                let right = m3g::Mat4::scale(
                    float_argument(args, 1)?,
                    float_argument(args, 2)?,
                    float_argument(args, 3)?,
                )?;
                let value = self.m3g.runtime.transform_value(handle)?.multiplied(right);
                self.m3g.runtime.set_transform_value(handle, value)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transform", "postTranslate", "(FFF)V") => {
                let handle = self.m3g_receiver(args)?;
                let right = m3g::Mat4::translation(
                    float_argument(args, 1)?,
                    float_argument(args, 2)?,
                    float_argument(args, 3)?,
                )?;
                let value = self.m3g.runtime.transform_value(handle)?.multiplied(right);
                self.m3g.runtime.set_transform_value(handle, value)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transform", "postRotate", "(FFFF)V") => {
                let handle = self.m3g_receiver(args)?;
                let right = m3g::Mat4::rotation(
                    float_argument(args, 1)?,
                    m3g::Vec3::new(
                        float_argument(args, 2)?,
                        float_argument(args, 3)?,
                        float_argument(args, 4)?,
                    ),
                )?;
                let value = self.m3g.runtime.transform_value(handle)?.multiplied(right);
                self.m3g.runtime.set_transform_value(handle, value)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transform", "postRotateQuat", "(FFFF)V") => {
                let handle = self.m3g_receiver(args)?;
                let right = m3g::Mat4::from_quaternion(m3g::Quaternion {
                    x: float_argument(args, 1)?,
                    y: float_argument(args, 2)?,
                    z: float_argument(args, 3)?,
                    w: float_argument(args, 4)?,
                })?;
                let value = self.m3g.runtime.transform_value(handle)?.multiplied(right);
                self.m3g.runtime.set_transform_value(handle, value)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transform", "transform", "([F)V") => {
                let transform = self.m3g.runtime.transform_value(self.m3g_receiver(args)?)?;
                let array = reference_argument(args, 1)?;
                let values = self.m3g_float_array_elements_mut(array, 0)?;
                // Validate the entire input before changing any vector.
                if values
                    .iter()
                    .any(|value| !matches!(value, HeapValue::Float(_)))
                {
                    return Err(type_error());
                }
                if values.len() % 4 != 0 {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("Transform float array length must be a multiple of four"),
                    );
                }
                for vector in values.as_chunks_mut::<4>().0 {
                    let [
                        HeapValue::Float(x),
                        HeapValue::Float(y),
                        HeapValue::Float(z),
                        HeapValue::Float(w),
                    ] = *vector
                    else {
                        return Err(type_error());
                    };
                    let output = transform.transform(m3g::Vec4::new(x, y, z, w));
                    *vector = [output.x, output.y, output.z, output.w].map(HeapValue::Float);
                }
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Transform",
                "transform",
                "(Ljavax/microedition/m3g/VertexArray;[FZ)V",
            ) => {
                let transform = self.m3g.runtime.transform_value(self.m3g_receiver(args)?)?;
                let input = self.m3g_handle(reference_argument(args, 1)?)?;
                let destination = reference_argument(args, 2)?;
                let w = f32::from(int_argument(args, 3)? != 0);
                let m3g::ObjectKind::VertexArray(input) = self.m3g.runtime.kind(input)? else {
                    return Err(type_error());
                };
                if input.component_count() == 4 {
                    return Err(vm_error(
                        "illegal-argument-exception",
                        "Transform requires two- or three-component input vertices",
                    ));
                }
                // Keep the immutable component storage while borrowing the output heap array.
                let input = input.clone();
                let components =
                    input.components(0, input.vertex_count(), input.component_type())?;
                let output =
                    self.m3g_float_array_elements_mut(destination, input.vertex_count() * 4)?;
                for (source, destination) in components
                    .chunks_exact(input.component_count())
                    .zip(output.chunks_exact_mut(4))
                {
                    let x = f32::from(source[0]);
                    let y = f32::from(source[1]);
                    let z = f32::from(source.get(2).copied().unwrap_or(0));
                    let value = transform.transform(m3g::Vec4::new(x, y, z, w));
                    for (slot, value) in destination
                        .iter_mut()
                        .zip([value.x, value.y, value.z, value.w])
                    {
                        *slot = HeapValue::Float(value);
                    }
                }
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Transformable",
                "setTranslation" | "translate" | "setScale" | "scale",
                "(FFF)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let value = m3g::Vec3::new(
                    float_argument(args, 1)?,
                    float_argument(args, 2)?,
                    float_argument(args, 3)?,
                );
                match name {
                    "setTranslation" => self.m3g.runtime.set_translation(handle, value),
                    "translate" => self.m3g.runtime.translate(handle, value),
                    "setScale" => self.m3g.runtime.set_scale(handle, value),
                    _ => self.m3g.runtime.scale_by(handle, value),
                }?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transformable", "setOrientation", "(FFFF)V") => {
                let handle = self.m3g_receiver(args)?;
                self.m3g.runtime.set_orientation(
                    handle,
                    float_argument(args, 1)?,
                    m3g::Vec3::new(
                        float_argument(args, 2)?,
                        float_argument(args, 3)?,
                        float_argument(args, 4)?,
                    ),
                )?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Transformable", "preRotate" | "postRotate", "(FFFF)V") => {
                let handle = self.m3g_receiver(args)?;
                self.m3g.runtime.rotate(
                    handle,
                    float_argument(args, 1)?,
                    m3g::Vec3::new(
                        float_argument(args, 2)?,
                        float_argument(args, 3)?,
                        float_argument(args, 4)?,
                    ),
                    name == "preRotate",
                )?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Transformable",
                "getTranslation" | "getScale" | "getOrientation",
                "([F)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let (translation, scale, orientation) =
                    self.m3g.runtime.transform_components(handle)?;
                let values: &[f32] = match name {
                    "getTranslation" => &[translation.x, translation.y, translation.z],
                    "getScale" => &[scale.x, scale.y, scale.z],
                    _ => {
                        let (angle, axis) = orientation.to_axis_angle()?;
                        &[angle, axis.x, axis.y, axis.z]
                    }
                };
                self.m3g_write_float_array(reference_argument(args, 1)?, values)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Transformable",
                "setTransform",
                "(Ljavax/microedition/m3g/Transform;)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let value = self.m3g_optional_transform(args, 1)?;
                self.m3g.runtime.set_transform(handle, value)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Transformable",
                "getTransform" | "getCompositeTransform",
                "(Ljavax/microedition/m3g/Transform;)V",
            ) => {
                let source = self.m3g_receiver(args)?;
                let destination = self.m3g_handle(reference_argument(args, 1)?)?;
                let value = if name == "getTransform" {
                    self.m3g.runtime.general_transform(source)?
                } else {
                    self.m3g.runtime.composite_transform(source)?
                };
                self.m3g.runtime.set_transform_value(destination, value)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Node", "setRenderingEnable", "(Z)V")
            | ("javax/microedition/m3g/Node", "setPickingEnable", "(Z)V")
            | ("javax/microedition/m3g/Node", "setScope", "(I)V")
            | ("javax/microedition/m3g/Node", "setAlphaFactor", "(F)V") => {
                let handle = self.m3g_receiver(args)?;
                let (mut rendering, mut picking, mut scope, mut alpha) =
                    self.m3g.runtime.node_state(handle)?;
                match name {
                    "setRenderingEnable" => rendering = int_argument(args, 1)? != 0,
                    "setPickingEnable" => picking = int_argument(args, 1)? != 0,
                    "setScope" => scope = int_argument(args, 1)? as u32,
                    _ => alpha = float_argument(args, 1)?,
                }
                self.m3g
                    .runtime
                    .set_node_state(handle, rendering, picking, scope, alpha)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Node", "isRenderingEnabled", "()Z")
            | ("javax/microedition/m3g/Node", "isPickingEnabled", "()Z")
            | ("javax/microedition/m3g/Node", "getScope", "()I")
            | ("javax/microedition/m3g/Node", "getAlphaFactor", "()F") => {
                let (rendering, picking, scope, alpha) =
                    self.m3g.runtime.node_state(self.m3g_receiver(args)?)?;
                CallOutcome::Return(Some(match name {
                    "isRenderingEnabled" => Value::Int(i32::from(rendering)),
                    "isPickingEnabled" => Value::Int(i32::from(picking)),
                    "getScope" => Value::Int(scope as i32),
                    _ => Value::Float(alpha),
                }))
            }
            ("javax/microedition/m3g/Node", "getParent", "()Ljavax/microedition/m3g/Node;") => {
                let parent = self.m3g.runtime.parent(self.m3g_receiver(args)?)?;
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(parent))))
            }
            (
                "javax/microedition/m3g/Node",
                "setAlignment",
                "(Ljavax/microedition/m3g/Node;ILjavax/microedition/m3g/Node;I)V",
            ) => {
                let node = self.m3g_receiver(args)?;
                let z_reference = optional_reference_argument(args, 1)?
                    .map(|value| self.m3g_handle(value))
                    .transpose()?;
                let z_target = int_argument(args, 2)?;
                let y_reference = optional_reference_argument(args, 3)?
                    .map(|value| self.m3g_handle(value))
                    .transpose()?;
                let y_target = int_argument(args, 4)?;
                self.m3g.runtime.set_alignments(
                    node,
                    z_reference,
                    z_target,
                    y_reference,
                    y_target,
                )?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Node", "getAlignmentTarget", "(I)I")
            | (
                "javax/microedition/m3g/Node",
                "getAlignmentReference",
                "(I)Ljavax/microedition/m3g/Node;",
            ) => {
                let axis = match int_argument(args, 1)? {
                    148 => 0,
                    147 => 1,
                    _ => {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("alignment axis must be Y_AXIS or Z_AXIS"),
                        );
                    }
                };
                let (reference, target) =
                    self.m3g.runtime.alignment(self.m3g_receiver(args)?, axis)?;
                let value = if name == "getAlignmentTarget" {
                    Value::Int(target)
                } else {
                    Value::Reference(self.m3g_guest_handle(reference))
                };
                CallOutcome::Return(Some(value))
            }
            ("javax/microedition/m3g/Node", "align", "(Ljavax/microedition/m3g/Node;)V") => {
                let node = self.m3g_receiver(args)?;
                let reference = optional_reference_argument(args, 1)?
                    .map(|value| self.m3g_handle(value))
                    .transpose()?;
                self.m3g.runtime.align_subtree(node, reference)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Node",
                "getTransformTo",
                "(Ljavax/microedition/m3g/Node;Ljavax/microedition/m3g/Transform;)Z",
            ) => {
                let source = self.m3g_receiver(args)?;
                let target = self.m3g_handle(reference_argument(args, 1)?)?;
                let output = self.m3g_handle(reference_argument(args, 2)?)?;
                match self.m3g.runtime.transform_to(source, target) {
                    Ok(transform) => {
                        self.m3g.runtime.set_transform_value(output, transform)?;
                        CallOutcome::Return(Some(Value::Int(1)))
                    }
                    Err(error) if error.code() == "no-path" => {
                        CallOutcome::Return(Some(Value::Int(0)))
                    }
                    Err(error) => return self.error_as_call_outcome(error, args),
                }
            }
            ("javax/microedition/m3g/Group", "addChild", "(Ljavax/microedition/m3g/Node;)V") => {
                let group = self.m3g_receiver(args)?;
                let child = self.m3g_handle(reference_argument(args, 1)?)?;
                self.m3g_allocate_native(args, |runtime| runtime.add_child(group, child))?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Group", "removeChild", "(Ljavax/microedition/m3g/Node;)V") => {
                let group = self.m3g_receiver(args)?;
                if let Some(child) = optional_reference_argument(args, 1)? {
                    let child = self.m3g_handle(child)?;
                    self.m3g.runtime.remove_child(group, child)?;
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Group", "getChildCount", "()I") => {
                let count = self.m3g.runtime.children(self.m3g_receiver(args)?)?.len();
                CallOutcome::Return(Some(Value::Int(i32::try_from(count).unwrap_or(i32::MAX))))
            }
            ("javax/microedition/m3g/Group", "getChild", "(I)Ljavax/microedition/m3g/Node;") => {
                let group = self.m3g_receiver(args)?;
                let index = usize::try_from(int_argument(args, 1)?).map_err(|_| {
                    vm_error(
                        "index-out-of-bounds-exception",
                        "negative Group child index",
                    )
                })?;
                let child = self.m3g.runtime.children(group)?.get(index).copied();
                let Some(child) = child else {
                    return self.thread_exception(
                        "java/lang/IndexOutOfBoundsException",
                        Some("Group child index out of range"),
                    );
                };
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(Some(child)))))
            }
            (
                "javax/microedition/m3g/Group",
                "pick",
                "(IFFFFFFLjavax/microedition/m3g/RayIntersection;)Z",
            ) => {
                let group = self.m3g_receiver(args)?;
                let scope = int_argument(args, 1)?.cast_unsigned();
                let origin = m3g::Vec3::new(
                    float_argument(args, 2)?,
                    float_argument(args, 3)?,
                    float_argument(args, 4)?,
                );
                let direction = m3g::Vec3::new(
                    float_argument(args, 5)?,
                    float_argument(args, 6)?,
                    float_argument(args, 7)?,
                );
                let result = optional_reference_argument(args, 8)?
                    .map(|guest| self.m3g_handle(guest))
                    .transpose()?;
                let hit = self.m3g_pick_ray(group, scope, origin, direction, result, None)?;
                CallOutcome::Return(Some(Value::Int(i32::from(hit))))
            }
            (
                "javax/microedition/m3g/Group",
                "pick",
                "(IFFLjavax/microedition/m3g/Camera;Ljavax/microedition/m3g/RayIntersection;)Z",
            ) => {
                let group = self.m3g_receiver(args)?;
                let scope = int_argument(args, 1)?.cast_unsigned();
                let x = float_argument(args, 2)?;
                let y = float_argument(args, 3)?;
                if !x.is_finite() || !y.is_finite() {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("pick coordinates must be finite"),
                    );
                }
                let camera = self.m3g_handle(reference_argument(args, 4)?)?;
                let projection = match self.m3g.runtime.kind(camera)? {
                    m3g::ObjectKind::Camera { projection, .. } => *projection,
                    _ => return Err(type_error()),
                };
                let projection = projection.render_matrix()?;
                let inverse_projection = projection.inverted()?;
                let camera_to_group = self.m3g.runtime.transform_to(camera, group)?;
                let unproject = |z| {
                    let value = inverse_projection.transform(m3g::Vec4::new(
                        x.mul_add(2.0, -1.0),
                        (-y).mul_add(2.0, 1.0),
                        z,
                        1.0,
                    ));
                    let value = camera_to_group.transform(value);
                    let inverse = value.w.recip();
                    m3g::Vec3::new(value.x * inverse, value.y * inverse, value.z * inverse)
                };
                let origin = unproject(-1.0);
                let far = unproject(1.0);
                let direction =
                    m3g::Vec3::new(far.x - origin.x, far.y - origin.y, far.z - origin.z);
                let result = optional_reference_argument(args, 5)?
                    .map(|guest| self.m3g_handle(guest))
                    .transpose()?;
                let hit = self.m3g_pick_ray(
                    group,
                    scope,
                    origin,
                    direction,
                    result,
                    Some(M3gSpritePickContext {
                        viewport_point: [x, y],
                        projection,
                        group_to_camera: camera_to_group.inverted()?,
                    }),
                )?;
                CallOutcome::Return(Some(Value::Int(i32::from(hit))))
            }
            _ => return self.m3g_unsupported_native(),
        };
        Ok(outcome)
    }
}
