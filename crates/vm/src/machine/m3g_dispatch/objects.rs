use super::{
    Allocation, ArrayKind, CallOutcome, EmuError, Handle, HeapValue, Machine, Value,
    float_argument, heap_error, int_argument, m3g_non_negative_u32, m3g_non_negative_usize,
    optional_reference_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    fn m3g_loader_outcome(
        &mut self,
        result: Result<Handle, EmuError>,
    ) -> Result<CallOutcome, EmuError> {
        match result {
            Ok(array) => Ok(CallOutcome::Return(Some(Value::Reference(Some(array))))),
            Err(error) if error.code() == "execution-cancelled" => Err(error),
            Err(error) if error.code() == "security-exception" => {
                self.thread_exception("java/lang/SecurityException", Some(error.message()))
            }
            Err(error) => {
                let message = format!("{}: {}", error.code(), error.message());
                self.thread_exception("java/io/IOException", Some(&message))
            }
        }
    }

    pub(in crate::machine) fn invoke_m3g_object_native(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let outcome = match (class, name, descriptor) {
            (
                "javax/microedition/m3g/Loader",
                "load",
                "([BI)[Ljavax/microedition/m3g/Object3D;",
            ) => {
                self.m3g_check_load_cancellation()?;
                let source = self
                    .m3g_primitive_array_elements(reference_argument(args, 0)?, &ArrayKind::Byte)?;
                let offset = int_argument(args, 1)?;
                let Some(source) = usize::try_from(offset)
                    .ok()
                    .and_then(|offset| source.get(offset..))
                    .filter(|source| !source.is_empty())
                else {
                    return self.thread_exception(
                        "java/lang/IndexOutOfBoundsException",
                        Some("Loader byte offset is outside the array"),
                    );
                };
                let bytes = source
                    .iter()
                    .map(|value| match value {
                        HeapValue::Int(value) => Ok(*value as u8),
                        _ => Err(type_error()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let result = self.m3g_load_bytes(&bytes, None, args);
                return self.m3g_loader_outcome(result);
            }
            (
                "javax/microedition/m3g/Loader",
                "load",
                "(Ljava/lang/String;)[Ljavax/microedition/m3g/Object3D;",
            ) => {
                self.m3g_check_load_cancellation()?;
                let name = reference_argument(args, 0)?;
                let name = self
                    .heap
                    .string_values
                    .get(&name)
                    .map(|value| String::from_utf16_lossy(value))
                    .ok_or_else(type_error)?;
                let normalized = name.strip_prefix('/').unwrap_or(&name);
                let bytes = self.native_context.read_resource(normalized)?;
                let Some(bytes) = bytes else {
                    return self.thread_exception(
                        "java/io/IOException",
                        Some("M3G resource does not exist in the suite"),
                    );
                };
                let result = self.m3g_load_bytes(&bytes, Some(normalized), args);
                return self.m3g_loader_outcome(result);
            }
            ("javax/microedition/m3g/Object3D", "setUserID", "(I)V") => {
                let handle = self.m3g_receiver(args)?;
                self.m3g
                    .runtime
                    .set_user_id(handle, int_argument(args, 1)?)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Object3D", "getUserID", "()I") => {
                let handle = self.m3g_receiver(args)?;
                CallOutcome::Return(Some(Value::Int(self.m3g.runtime.user_id(handle)?)))
            }
            ("javax/microedition/m3g/Object3D", "setUserObject", "(Ljava/lang/Object;)V") => {
                let handle = self.m3g_receiver(args)?;
                let user_object = optional_reference_argument(args, 1)?.map(Handle::to_raw);
                self.m3g.runtime.set_user_object(handle, user_object)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Object3D", "getUserObject", "()Ljava/lang/Object;") => {
                let handle = self.m3g_receiver(args)?;
                let value = self
                    .m3g
                    .runtime
                    .user_object(handle)?
                    .map(Handle::from_raw)
                    .filter(|candidate| self.heap.managed.get(*candidate).is_ok());
                CallOutcome::Return(Some(Value::Reference(value)))
            }
            ("javax/microedition/m3g/Object3D", "find", "(I)Ljavax/microedition/m3g/Object3D;") => {
                let root = self.m3g_receiver(args)?;
                let found = self.m3g.runtime.find(root, int_argument(args, 1)?)?;
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(found))))
            }
            (
                "javax/microedition/m3g/Object3D",
                "addAnimationTrack",
                "(Ljavax/microedition/m3g/AnimationTrack;)V",
            ) => {
                let object = self.m3g_receiver(args)?;
                let track = self.m3g_handle(reference_argument(args, 1)?)?;
                self.m3g_allocate_native(args, |runtime| {
                    runtime.add_animation_track(object, track)
                })?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Object3D",
                "removeAnimationTrack",
                "(Ljavax/microedition/m3g/AnimationTrack;)V",
            ) => {
                let object = self.m3g_receiver(args)?;
                if let Some(track) = optional_reference_argument(args, 1)? {
                    let track = self.m3g_handle(track)?;
                    self.m3g.runtime.remove_animation_track(object, track)?;
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Object3D", "getAnimationTrackCount", "()I") => {
                let object = self.m3g_receiver(args)?;
                let count = self.m3g.runtime.animation_tracks(object)?.len();
                CallOutcome::Return(Some(Value::Int(i32::try_from(count).unwrap_or(i32::MAX))))
            }
            (
                "javax/microedition/m3g/Object3D",
                "getAnimationTrack",
                "(I)Ljavax/microedition/m3g/AnimationTrack;",
            ) => {
                let object = self.m3g_receiver(args)?;
                let index = m3g_non_negative_usize(int_argument(args, 1)?)?;
                let track = *self
                    .m3g
                    .runtime
                    .animation_tracks(object)?
                    .get(index)
                    .ok_or_else(|| {
                        vm_error(
                            "index-out-of-bounds-exception",
                            "animation track index is out of bounds",
                        )
                    })?;
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(Some(track)))))
            }
            (
                "javax/microedition/m3g/Object3D",
                "getReferences",
                "([Ljavax/microedition/m3g/Object3D;)I",
            ) => {
                let object = self.m3g_receiver(args)?;
                let references = self.m3g.runtime.references(object)?;
                let destination = optional_reference_argument(args, 1)?;
                if let Some(destination) = destination {
                    let guests = references
                        .iter()
                        .map(|handle| self.m3g_guest_handle(Some(*handle)))
                        .collect::<Vec<_>>();
                    let Allocation::Array { kind, elements } =
                        self.heap.managed.get_mut(destination).map_err(heap_error)?
                    else {
                        return Err(type_error());
                    };
                    if !matches!(kind, ArrayKind::Reference(_)) || elements.len() < guests.len() {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("Object3D reference array is too short or has the wrong type"),
                        );
                    }
                    for (slot, guest) in elements.iter_mut().zip(guests) {
                        *slot = HeapValue::Reference(guest);
                    }
                }
                CallOutcome::Return(Some(Value::Int(
                    i32::try_from(references.len()).unwrap_or(i32::MAX),
                )))
            }
            (
                "javax/microedition/m3g/Object3D",
                "duplicate",
                "()Ljavax/microedition/m3g/Object3D;",
            ) => {
                let source_guest = reference_argument(args, 0)?;
                let source = self.m3g_handle(source_guest)?;
                let sources = self.m3g.runtime.duplicate_sources(source)?;
                let mut mapping = std::collections::BTreeMap::new();
                let mut created_native = Vec::new();
                let temporary_roots = self.heap.temporary_roots.len();
                let result = (|| {
                    for original in sources {
                        let original_guest =
                            self.m3g_guest_handle(Some(original)).ok_or_else(|| {
                                vm_error(
                                    "illegal-state-exception",
                                    "duplicate source lost guest binding",
                                )
                            })?;
                        let class = self.object_class(original_guest)?.into_owned();
                        let duplicate = self.allocate_native_instance(&class, args)?;
                        self.heap.temporary_roots.push(duplicate);
                        let native = self.m3g_allocate_native(args, |runtime| {
                            runtime.create_duplicate(original, duplicate.to_raw())
                        })?;
                        created_native.push(native);
                        mapping.insert(original, native);
                    }
                    self.m3g.runtime.finish_duplicate(source, &mapping)?;
                    let duplicate = mapping.get(&source).copied().ok_or_else(|| {
                        vm_error("illegal-state-exception", "duplicate root was not copied")
                    })?;
                    self.m3g_guest_handle(Some(duplicate)).ok_or_else(|| {
                        vm_error(
                            "illegal-state-exception",
                            "duplicate root lost guest binding",
                        )
                    })
                })();
                self.heap.temporary_roots.truncate(temporary_roots);
                let duplicate = match result {
                    Ok(duplicate) => duplicate,
                    Err(error) => {
                        self.m3g.runtime.rollback_created(&created_native);
                        return Err(error);
                    }
                };
                CallOutcome::Return(Some(Value::Reference(Some(duplicate))))
            }
            ("javax/microedition/m3g/Object3D", "animate", "(I)I") => {
                let object = self.m3g_receiver(args)?;
                let validity = self.m3g_animate(object, int_argument(args, 1)?)?;
                CallOutcome::Return(Some(Value::Int(validity)))
            }
            ("javax/microedition/m3g/VertexArray", "getVertexCount", "()I")
            | ("javax/microedition/m3g/VertexArray", "getComponentCount", "()I")
            | ("javax/microedition/m3g/VertexArray", "getComponentType", "()I") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::VertexArray(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                let value = match name {
                    "getVertexCount" => i32::try_from(state.vertex_count()).unwrap_or(i32::MAX),
                    "getComponentCount" => {
                        i32::try_from(state.component_count()).unwrap_or(i32::MAX)
                    }
                    _ => match state.component_type() {
                        m3g::VertexComponent::Byte => 1,
                        m3g::VertexComponent::Short => 2,
                    },
                };
                CallOutcome::Return(Some(Value::Int(value)))
            }
            ("javax/microedition/m3g/VertexArray", "set" | "get", "(II[B)V" | "(II[S)V") => {
                let handle = self.m3g_receiver(args)?;
                let first = usize::try_from(int_argument(args, 1)?).map_err(|_| {
                    vm_error(
                        "index-out-of-bounds-exception",
                        "first vertex must be non-negative",
                    )
                })?;
                let count = m3g_non_negative_usize(int_argument(args, 2)?)?;
                let transfer = reference_argument(args, 3)?;
                let (component_type, array_type) = if descriptor == "(II[B)V" {
                    (m3g::VertexComponent::Byte, ArrayKind::Byte)
                } else {
                    (m3g::VertexComponent::Short, ArrayKind::Short)
                };
                let m3g::ObjectKind::VertexArray(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                if name == "get" {
                    let state = state.clone();
                    self.m3g_write_primitive_int_array(
                        transfer,
                        &array_type,
                        state.components(first, count, component_type)?,
                    )?;
                } else {
                    let length = state.components(first, count, component_type)?.len();
                    let Allocation::Array { kind, elements } =
                        self.heap.managed.get(transfer).map_err(heap_error)?
                    else {
                        return Err(type_error());
                    };
                    if *kind != array_type {
                        return Err(type_error());
                    }
                    let values = elements.get(..length).ok_or_else(|| {
                        vm_error(
                            "illegal-argument-exception",
                            "VertexArray transfer array is too short",
                        )
                    })?;
                    // Validate the whole consumed prefix before mutating native
                    // storage, including when it is shared with another object.
                    if values
                        .iter()
                        .any(|value| !matches!(value, HeapValue::Int(_)))
                    {
                        return Err(type_error());
                    }
                    let m3g::ObjectKind::VertexArray(state) = self.m3g.runtime.kind_mut(handle)?
                    else {
                        return Err(type_error());
                    };
                    state.set_integers(
                        first,
                        count,
                        values.iter().map(|value| {
                            let HeapValue::Int(value) = value else {
                                unreachable!("validated integer prefix");
                            };
                            *value
                        }),
                    )?;
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/IndexBuffer", "getIndexCount", "()I") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::TriangleStripArray(state) = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Int(
                    i32::try_from(state.index_count()).unwrap_or(i32::MAX),
                )))
            }
            ("javax/microedition/m3g/IndexBuffer", "getIndices", "([I)V") => {
                let handle = self.m3g_receiver(args)?;
                let destination = reference_argument(args, 1)?;
                let m3g::ObjectKind::TriangleStripArray(state) = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                let values = state
                    .indices()
                    .iter()
                    .map(|value| i32::try_from(*value).unwrap_or(i32::MAX))
                    .collect::<Vec<_>>();
                self.m3g_write_int_array(destination, &values)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/VertexBuffer", "getVertexCount", "()I") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::VertexBuffer { state, .. } = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Int(
                    i32::try_from(state.vertex_count()).unwrap_or(i32::MAX),
                )))
            }
            ("javax/microedition/m3g/VertexBuffer", "setDefaultColor", "(I)V") => {
                let handle = self.m3g_receiver(args)?;
                let color = int_argument(args, 1)?.cast_unsigned();
                let m3g::ObjectKind::VertexBuffer { state, .. } =
                    self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.set_default_color(color);
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/VertexBuffer", "getDefaultColor", "()I") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::VertexBuffer { state, .. } = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Int(state.default_color().cast_signed())))
            }
            (
                "javax/microedition/m3g/VertexBuffer",
                "setPositions",
                "(Ljavax/microedition/m3g/VertexArray;F[F)V",
            )
            | (
                "javax/microedition/m3g/VertexBuffer",
                "setTexCoords",
                "(ILjavax/microedition/m3g/VertexArray;F[F)V",
            ) => {
                let buffer = self.m3g_receiver(args)?;
                let (unit, array_index) = if name == "setPositions" {
                    (None, 1)
                } else {
                    (Some(self.m3g_texture_unit(int_argument(args, 1)?)?), 2)
                };
                let native = optional_reference_argument(args, array_index)?
                    .map(|guest| self.m3g_handle(guest))
                    .transpose()?;
                let array = native
                    .map(|handle| match self.m3g.runtime.kind(handle)? {
                        m3g::ObjectKind::VertexArray(array) => Ok(array.clone()),
                        _ => Err(type_error()),
                    })
                    .transpose()?;
                let scale = float_argument(args, array_index + 1)?;
                let mut bias = [0.0; 3];
                if let (Some(array), Some(source)) =
                    (&array, optional_reference_argument(args, array_index + 2)?)
                {
                    let components = if unit.is_some() {
                        array.component_count()
                    } else {
                        3
                    };
                    if components > bias.len() {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "texture coordinates require two or three components",
                        ));
                    }
                    self.m3g_read_float_array(source, &mut bias[..components])?;
                }
                self.m3g_allocate_native(args, |runtime| {
                    if let Some(unit) = unit {
                        runtime.set_vertex_buffer_texture_coordinates(
                            buffer,
                            unit,
                            array.clone(),
                            native,
                            scale,
                            bias,
                        )
                    } else {
                        runtime.set_vertex_buffer_positions(
                            buffer,
                            array.clone(),
                            native,
                            scale,
                            bias,
                        )
                    }
                })?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/VertexBuffer",
                "setNormals" | "setColors",
                "(Ljavax/microedition/m3g/VertexArray;)V",
            ) => {
                let buffer = self.m3g_receiver(args)?;
                let guest = optional_reference_argument(args, 1)?;
                let native = guest.map(|guest| self.m3g_handle(guest)).transpose()?;
                let array = native
                    .map(|handle| match self.m3g.runtime.kind(handle)? {
                        m3g::ObjectKind::VertexArray(array) => Ok(array.clone()),
                        _ => Err(type_error()),
                    })
                    .transpose()?;
                self.m3g_allocate_native(args, |runtime| {
                    if name == "setNormals" {
                        runtime.set_vertex_buffer_normals(buffer, array.clone(), native)
                    } else {
                        runtime.set_vertex_buffer_colors(buffer, array.clone(), native)
                    }
                })?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/VertexBuffer",
                "getNormals" | "getColors",
                "()Ljavax/microedition/m3g/VertexArray;",
            ) => {
                let buffer = self.m3g_receiver(args)?;
                let m3g::ObjectKind::VertexBuffer { arrays, .. } = self.m3g.runtime.kind(buffer)?
                else {
                    return Err(type_error());
                };
                let slot = if name == "getNormals" { 1 } else { 2 };
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(arrays[slot]))))
            }
            (
                "javax/microedition/m3g/VertexBuffer",
                "getPositions",
                "([F)Ljavax/microedition/m3g/VertexArray;",
            )
            | (
                "javax/microedition/m3g/VertexBuffer",
                "getTexCoords",
                "(I[F)Ljavax/microedition/m3g/VertexArray;",
            ) => {
                let buffer = self.m3g_receiver(args)?;
                let (unit, destination, slot) = if name == "getPositions" {
                    (None, optional_reference_argument(args, 1)?, 0)
                } else {
                    let unit = self.m3g_texture_unit(int_argument(args, 1)?)?;
                    (Some(unit), optional_reference_argument(args, 2)?, 3 + unit)
                };
                let m3g::ObjectKind::VertexBuffer { state, arrays } =
                    self.m3g.runtime.kind(buffer)?
                else {
                    return Err(type_error());
                };
                let attribute =
                    unit.map_or_else(|| state.positions(), |unit| state.texture_coordinates(unit));
                let count = if unit.is_none() {
                    4
                } else {
                    attribute.map_or(0, |(array, ..)| array.component_count() + 1)
                };
                let (scale, bias) =
                    attribute.map_or((1.0, [0.0; 3]), |(_, scale, bias)| (scale, bias));
                let scale_bias = [scale, bias[0], bias[1], bias[2]];
                let result = self.m3g_guest_handle(arrays[slot]);
                if let Some(destination) = destination {
                    self.m3g_write_float_array(destination, &scale_bias[..count])?;
                }
                CallOutcome::Return(Some(Value::Reference(result)))
            }
            ("javax/microedition/m3g/Image2D", "isMutable", "()Z")
            | ("javax/microedition/m3g/Image2D", "getFormat", "()I")
            | ("javax/microedition/m3g/Image2D", "getWidth", "()I")
            | ("javax/microedition/m3g/Image2D", "getHeight", "()I") => {
                let image = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Image2D(state) = self.m3g.runtime.kind(image)? else {
                    return Err(type_error());
                };
                let value = match name {
                    "isMutable" => i32::from(state.is_mutable()),
                    "getFormat" => match state.format() {
                        m3g::ImageFormat::Alpha => 96,
                        m3g::ImageFormat::Luminance => 97,
                        m3g::ImageFormat::LuminanceAlpha => 98,
                        m3g::ImageFormat::Rgb => 99,
                        m3g::ImageFormat::Rgba => 100,
                    },
                    "getWidth" => i32::try_from(state.width()).unwrap_or(i32::MAX),
                    _ => i32::try_from(state.height()).unwrap_or(i32::MAX),
                };
                CallOutcome::Return(Some(Value::Int(value)))
            }
            ("javax/microedition/m3g/Image2D", "set", "(IIII[B)V") => {
                let image = self.m3g_receiver(args)?;
                let x = m3g_non_negative_u32(int_argument(args, 1)?)?;
                let y = m3g_non_negative_u32(int_argument(args, 2)?)?;
                let width = m3g_non_negative_u32(int_argument(args, 3)?)?;
                let height = m3g_non_negative_u32(int_argument(args, 4)?)?;
                let source = reference_argument(args, 5)?;
                let m3g::ObjectKind::Image2D(state) = self.m3g.runtime.kind(image)? else {
                    return Err(type_error());
                };
                let required = state.validate_update(x, y, width, height)?;
                let bytes = self.m3g_byte_array_prefix(source, required)?;
                let m3g::ObjectKind::Image2D(state) = self.m3g.runtime.kind_mut(image)? else {
                    return Err(type_error());
                };
                state.set(x, y, width, height, &bytes)?;
                CallOutcome::Return(None)
            }
            _ => return self.m3g_unsupported_native(),
        };
        Ok(outcome)
    }
}
