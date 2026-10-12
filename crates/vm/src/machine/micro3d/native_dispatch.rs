use super::{
    CallOutcome, EmuError, Handle, Machine, Method, Value, display_key, int_argument,
    optional_reference_argument, reference_argument, vm_error,
};

impl Machine<'_, '_> {
    #[allow(clippy::too_many_lines)]
    pub(in crate::machine) fn invoke_micro3d_native(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let class = method.key.class.as_str();
        let name = method.key.name.as_str();
        let descriptor = method.key.descriptor.as_str();

        if class == "com/mascotcapsule/micro3d/v3/Util3D" {
            let input = int_argument(args, 0)?;
            let result = match name {
                "sqrt" => micro3d::sqrt(input),
                "sin" => micro3d::sin(input),
                "cos" => micro3d::cos(input),
                _ => return Err(vm_error("method-not-found", display_key(&method.key))),
            };
            return Ok(CallOutcome::Return(Some(Value::Int(result))));
        }

        if name == "<init>" {
            let receiver = reference_argument(args, 0)?;
            match class {
                "com/mascotcapsule/micro3d/v3/Vector3D" => {
                    let value = match descriptor {
                        "()V" => micro3d::Vector3D::default(),
                        "(III)V" => micro3d::Vector3D::new(
                            int_argument(args, 1)?,
                            int_argument(args, 2)?,
                            int_argument(args, 3)?,
                        ),
                        _ => self.micro3d_vector(reference_argument(args, 1)?)?,
                    };
                    self.micro3d_set_vector(receiver, value)?;
                }
                "com/mascotcapsule/micro3d/v3/AffineTrans" => {
                    let value = self.micro3d_affine_arguments(method, args)?;
                    self.micro3d_set_affine(receiver, value)?;
                }
                "com/mascotcapsule/micro3d/v3/ActionTable"
                | "com/mascotcapsule/micro3d/v3/Figure"
                | "com/mascotcapsule/micro3d/v3/Texture" => {
                    let bytes = if descriptor.starts_with("([B") {
                        self.m3g_byte_array(reference_argument(args, 1)?)?
                    } else {
                        let resource = reference_argument(args, 1)?;
                        let Some(bytes) = self.micro3d_resource_bytes(resource)? else {
                            return self.thread_exception(
                                "java/io/IOException",
                                Some("Micro3D suite resource does not exist"),
                            );
                        };
                        bytes
                    };
                    let guest = receiver.to_raw();
                    let limits = self.limits.micro3d_loader;
                    let for_model = class == "com/mascotcapsule/micro3d/v3/Texture"
                        && int_argument(args, 2)? != 0;
                    let result = self.micro3d_allocate_native(args, |runtime| match class {
                        "com/mascotcapsule/micro3d/v3/ActionTable" => {
                            runtime.create_action(guest, &bytes, limits)
                        }
                        "com/mascotcapsule/micro3d/v3/Figure" => {
                            runtime.create_figure(guest, &bytes, limits)
                        }
                        _ => runtime.create_texture(guest, &bytes, for_model, limits),
                    });
                    if let Err(error) = result {
                        if descriptor.starts_with("(Ljava/lang/String;") {
                            return self
                                .thread_exception("java/io/IOException", Some(error.message()));
                        }
                        return self.micro3d_failure(error, args);
                    }
                }
                "com/mascotcapsule/micro3d/v3/Light" => {
                    let state = if descriptor == "()V" {
                        micro3d::LightState::default()
                    } else {
                        micro3d::LightState {
                            direction: self.micro3d_vector(reference_argument(args, 1)?)?,
                            directional_intensity: int_argument(args, 2)?,
                            ambient_intensity: int_argument(args, 3)?,
                        }
                    };
                    self.micro3d_allocate_native(args, |runtime| {
                        runtime.create(receiver.to_raw(), micro3d::ObjectKind::Light(state))
                    })?;
                }
                "com/mascotcapsule/micro3d/v3/Effect3D" => {
                    let state = if descriptor == "()V" {
                        micro3d::EffectState::default()
                    } else {
                        micro3d::EffectState {
                            light: optional_reference_argument(args, 1)?.map(Handle::to_raw),
                            shading: int_argument(args, 2)?,
                            transparency: int_argument(args, 3)? != 0,
                            sphere_texture: optional_reference_argument(args, 4)?
                                .map(Handle::to_raw),
                            ..micro3d::EffectState::default()
                        }
                    };
                    if let Err(error) = self.micro3d_validate_effect_state(state) {
                        return self.micro3d_failure(error, args);
                    }
                    self.micro3d_allocate_native(args, |runtime| {
                        runtime.create(receiver.to_raw(), micro3d::ObjectKind::Effect(state))
                    })?;
                }
                "com/mascotcapsule/micro3d/v3/FigureLayout" => {
                    let (state, constructor_roots) = if descriptor == "()V" {
                        let affine =
                            self.micro3d_allocate_affine(micro3d::AffineTrans::IDENTITY, args)?;
                        let mut roots = args.to_vec();
                        roots.push(Value::Reference(Some(affine)));
                        (
                            micro3d::FigureLayoutState {
                                affines: vec![affine.to_raw()],
                                ..micro3d::FigureLayoutState::default()
                            },
                            roots,
                        )
                    } else {
                        let affine = optional_reference_argument(args, 1)?;
                        (
                            micro3d::FigureLayoutState {
                                affines: affine.into_iter().map(Handle::to_raw).collect(),
                                scale: [int_argument(args, 2)?, int_argument(args, 3)?],
                                center: [int_argument(args, 4)?, int_argument(args, 5)?],
                                ..micro3d::FigureLayoutState::default()
                            },
                            args.to_vec(),
                        )
                    };
                    self.micro3d_allocate_native(&constructor_roots, |runtime| {
                        runtime.create(
                            receiver.to_raw(),
                            micro3d::ObjectKind::Layout(state.clone()),
                        )
                    })?;
                }
                "com/mascotcapsule/micro3d/v3/Graphics3D" => {
                    self.micro3d_allocate_native(args, |runtime| {
                        runtime.create(receiver.to_raw(), micro3d::ObjectKind::Graphics)
                    })?;
                }
                _ => return Err(vm_error("method-not-found", display_key(&method.key))),
            }
            return Ok(CallOutcome::Return(None));
        }

        if class == "com/mascotcapsule/micro3d/v3/Vector3D"
            && name == "innerProduct"
            && descriptor
                == "(Lcom/mascotcapsule/micro3d/v3/Vector3D;Lcom/mascotcapsule/micro3d/v3/Vector3D;)I"
        {
            let left = self.micro3d_vector(reference_argument(args, 0)?)?;
            let right = self.micro3d_vector(reference_argument(args, 1)?)?;
            return Ok(CallOutcome::Return(Some(Value::Int(left.inner(right)))));
        }
        if class == "com/mascotcapsule/micro3d/v3/Vector3D"
            && name == "outerProduct"
            && descriptor.ends_with("Lcom/mascotcapsule/micro3d/v3/Vector3D;")
        {
            let left = self.micro3d_vector(reference_argument(args, 0)?)?;
            let right = self.micro3d_vector(reference_argument(args, 1)?)?;
            let result = self.micro3d_allocate_vector(left.outer(right), args)?;
            return Ok(CallOutcome::Return(Some(Value::Reference(Some(result)))));
        }

        if class == "com/mascotcapsule/micro3d/v3/Vector3D" {
            let receiver = reference_argument(args, 0)?;
            let mut value = self.micro3d_vector(receiver)?;
            let outcome = match (name, descriptor) {
                ("getX", _) => Some(Value::Int(value.x)),
                ("getY", _) => Some(Value::Int(value.y)),
                ("getZ", _) => Some(Value::Int(value.z)),
                ("setX", _) => {
                    value.x = int_argument(args, 1)?;
                    self.micro3d_set_vector(receiver, value)?;
                    None
                }
                ("setY", _) => {
                    value.y = int_argument(args, 1)?;
                    self.micro3d_set_vector(receiver, value)?;
                    None
                }
                ("setZ", _) => {
                    value.z = int_argument(args, 1)?;
                    self.micro3d_set_vector(receiver, value)?;
                    None
                }
                ("set", "(III)V") => {
                    self.micro3d_set_vector(
                        receiver,
                        micro3d::Vector3D::new(
                            int_argument(args, 1)?,
                            int_argument(args, 2)?,
                            int_argument(args, 3)?,
                        ),
                    )?;
                    None
                }
                ("set", _) => {
                    let source = self.micro3d_vector(reference_argument(args, 1)?)?;
                    self.micro3d_set_vector(receiver, source)?;
                    None
                }
                ("unit", _) => {
                    value.unit();
                    self.micro3d_set_vector(receiver, value)?;
                    None
                }
                ("innerProduct", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)I") => Some(Value::Int(
                    value.inner(self.micro3d_vector(reference_argument(args, 1)?)?),
                )),
                ("outerProduct", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)V") => {
                    let result = value.outer(self.micro3d_vector(reference_argument(args, 1)?)?);
                    self.micro3d_set_vector(receiver, result)?;
                    None
                }
                _ => return Err(vm_error("method-not-found", display_key(&method.key))),
            };
            return Ok(CallOutcome::Return(outcome));
        }

        if class == "com/mascotcapsule/micro3d/v3/AffineTrans" {
            return self.invoke_micro3d_affine(method, args);
        }

        self.invoke_micro3d_object(method, args)
    }
}
