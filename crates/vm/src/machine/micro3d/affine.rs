use super::{
    Allocation, ArrayKind, CallOutcome, EmuError, Handle, HeapValue, Machine, Method, Value,
    display_key, heap_error, int_argument, micro3d_preserve_translation, reference_argument,
    type_error, vm_error,
};

impl Machine<'_, '_> {
    #[allow(clippy::too_many_lines)]
    pub(in crate::machine) fn invoke_micro3d_affine(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let name = method.key.name.as_str();
        let descriptor = method.key.descriptor.as_str();
        let receiver = reference_argument(args, 0)?;
        let mut affine = self.micro3d_affine(receiver)?;
        match (name, descriptor) {
            ("setIdentity", _) => {
                self.micro3d_set_affine(receiver, micro3d::AffineTrans::IDENTITY)?
            }
            ("setRotationX" | "rotationX", _) => self.micro3d_set_affine(
                receiver,
                micro3d_preserve_translation(
                    micro3d::AffineTrans::rotation_x(int_argument(args, 1)?),
                    affine,
                ),
            )?,
            ("setRotationY" | "rotationY", _) => self.micro3d_set_affine(
                receiver,
                micro3d_preserve_translation(
                    micro3d::AffineTrans::rotation_y(int_argument(args, 1)?),
                    affine,
                ),
            )?,
            ("setRotationZ" | "rotationZ", _) => self.micro3d_set_affine(
                receiver,
                micro3d_preserve_translation(
                    micro3d::AffineTrans::rotation_z(int_argument(args, 1)?),
                    affine,
                ),
            )?,
            ("rotationV" | "setRotation", _) => self.micro3d_set_affine(
                receiver,
                micro3d_preserve_translation(
                    micro3d::AffineTrans::rotation(
                        self.micro3d_vector(reference_argument(args, 1)?)?,
                        int_argument(args, 2)?,
                    ),
                    affine,
                ),
            )?,
            ("setViewTrans" | "lookAt", _) => self.micro3d_set_affine(
                receiver,
                micro3d::AffineTrans::look_at(
                    self.micro3d_vector(reference_argument(args, 1)?)?,
                    self.micro3d_vector(reference_argument(args, 2)?)?,
                    self.micro3d_vector(reference_argument(args, 3)?)?,
                ),
            )?,
            ("multiply" | "mul", "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V") => {
                affine = affine.multiplied(self.micro3d_affine(reference_argument(args, 1)?)?);
                self.micro3d_set_affine(receiver, affine)?;
            }
            ("multiply" | "mul", _) => self.micro3d_set_affine(
                receiver,
                self.micro3d_affine(reference_argument(args, 1)?)?
                    .multiplied(self.micro3d_affine(reference_argument(args, 2)?)?),
            )?,
            ("transPoint" | "transform", _) => {
                let value = affine.transform(self.micro3d_vector(reference_argument(args, 1)?)?);
                let result = self.micro3d_allocate_vector(value, args)?;
                return Ok(CallOutcome::Return(Some(Value::Reference(Some(result)))));
            }
            ("get", "([I)V") | ("get", "([II)V") => {
                let destination = reference_argument(args, 1)?;
                let offset = if descriptor == "([II)V" {
                    usize::try_from(int_argument(args, 2)?).unwrap_or(usize::MAX)
                } else {
                    0
                };
                let target = self
                    .graphics_int_array_mut(destination)?
                    .get_mut(offset..offset.saturating_add(12))
                    .ok_or_else(|| {
                        vm_error(
                            "illegal-argument-exception",
                            "AffineTrans destination is too short",
                        )
                    })?;
                for (destination, value) in target.iter_mut().zip(affine.values) {
                    *destination = HeapValue::Int(value);
                }
            }
            (
                "set",
                "(IIIIIIIIIIII)V"
                | "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"
                | "([I)V"
                | "([II)V"
                | "([[I)V",
            ) => {
                let value = self.micro3d_affine_arguments(method, args)?;
                self.micro3d_set_affine(receiver, value)?;
            }
            _ => return Err(vm_error("method-not-found", display_key(&method.key))),
        }
        Ok(CallOutcome::Return(None))
    }
    pub(in crate::machine) fn micro3d_affine_arguments(
        &self,
        method: &Method,
        args: &[Value],
    ) -> Result<micro3d::AffineTrans, EmuError> {
        match method.key.descriptor.as_str() {
            "()V" => Ok(micro3d::AffineTrans::ZERO),
            "(IIIIIIIIIIII)V" => {
                let mut values = [0; 12];
                for (index, value) in values.iter_mut().enumerate() {
                    *value = int_argument(args, index + 1)?;
                }
                Ok(micro3d::AffineTrans::new(values))
            }
            "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V" => {
                self.micro3d_affine(reference_argument(args, 1)?)
            }
            descriptor @ ("([I)V" | "([II)V") => {
                let offset = if descriptor == "([II)V" {
                    usize::try_from(int_argument(args, 2)?).unwrap_or(usize::MAX)
                } else {
                    0
                };
                let mut values = [0; 12];
                self.micro3d_read_matrix_elements(
                    reference_argument(args, 1)?,
                    offset,
                    &mut values,
                )?;
                Ok(micro3d::AffineTrans::new(values))
            }
            "([[I)V" => {
                let array = reference_argument(args, 1)?;
                let Allocation::Array {
                    kind: ArrayKind::Reference(_),
                    elements,
                } = self.heap.managed.get(array).map_err(heap_error)?
                else {
                    return Err(type_error());
                };
                let rows = elements.get(..3).ok_or_else(|| {
                    vm_error(
                        "illegal-argument-exception",
                        "AffineTrans matrix has fewer than three rows",
                    )
                })?;
                let mut values = [0; 12];
                for (row, destination) in rows.iter().zip(values.chunks_exact_mut(4)) {
                    let source = match row {
                        HeapValue::Reference(Some(handle)) => *handle,
                        HeapValue::Reference(None) => {
                            return Err(vm_error("null-pointer-exception", "null AffineTrans row"));
                        }
                        _ => return Err(type_error()),
                    };
                    self.micro3d_read_matrix_elements(source, 0, destination)?;
                }
                Ok(micro3d::AffineTrans::new(values))
            }
            _ => Err(vm_error("method-not-found", display_key(&method.key))),
        }
    }

    fn micro3d_read_matrix_elements(
        &self,
        array: Handle,
        offset: usize,
        destination: &mut [i32],
    ) -> Result<(), EmuError> {
        let elements = self.m3g_primitive_array_elements(array, &ArrayKind::Int)?;
        let source = elements
            .get(offset..offset.saturating_add(destination.len()))
            .ok_or_else(|| {
                vm_error(
                    "illegal-argument-exception",
                    "AffineTrans source is too short",
                )
            })?;
        for (destination, source) in destination.iter_mut().zip(source) {
            let HeapValue::Int(value) = source else {
                return Err(type_error());
            };
            *destination = *value;
        }
        Ok(())
    }
}
