use super::{
    CallOutcome, Category, EmuError, Machine, Method, Value, display_key, int_argument,
    reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    #[allow(clippy::too_many_lines)]
    pub(in crate::machine) fn invoke_micro3d_object(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let class = method.key.class.as_str();
        let name = method.key.name.as_str();
        let descriptor = method.key.descriptor.as_str();
        let receiver = reference_argument(args, 0)?;
        if name == "dispose" {
            if let Err(error) = self.micro3d.runtime.dispose(receiver.to_raw()) {
                return self.micro3d_failure(error, args);
            }
            return Ok(CallOutcome::Return(None));
        }
        let outcome = match class {
            "com/mascotcapsule/micro3d/v3/ActionTable" => {
                let micro3d::ObjectKind::Action(action) =
                    self.micro3d.runtime.kind(receiver.to_raw())?
                else {
                    return Err(type_error());
                };
                match name {
                    "getNumAction" | "getNumActions" => Some(Value::Int(
                        i32::try_from(action.frame_counts.len()).unwrap_or(i32::MAX),
                    )),
                    "getNumFrame" | "getNumFrames" => {
                        let index = usize::try_from(int_argument(args, 1)?).unwrap_or(usize::MAX);
                        let Some(frames) = action.frame_counts.get(index).copied() else {
                            return self.thread_exception(
                                "java/lang/IllegalArgumentException",
                                Some("action index is out of bounds"),
                            );
                        };
                        // Action lengths use the same signed 16.16 frame
                        // domain accepted by Figure.setPosture.
                        Some(Value::Int(i32::from(frames) << 16))
                    }
                    _ => return Err(vm_error("method-not-found", display_key(&method.key))),
                }
            }
            "com/mascotcapsule/micro3d/v3/Light" => {
                let mut state = match self.micro3d.runtime.kind(receiver.to_raw())? {
                    micro3d::ObjectKind::Light(state) => *state,
                    _ => return Err(type_error()),
                };
                match name {
                    "getDirIntensity" | "getParallelLightIntensity" => {
                        Some(Value::Int(state.directional_intensity))
                    }
                    "getAmbIntensity" | "getAmbientIntensity" => {
                        Some(Value::Int(state.ambient_intensity))
                    }
                    "getDirection" | "getParallelLightDirection" => {
                        let vector = self.micro3d_allocate_vector(state.direction, args)?;
                        Some(Value::Reference(Some(vector)))
                    }
                    "setDirIntensity" | "setParallelLightIntensity" => {
                        state.directional_intensity = int_argument(args, 1)?;
                        if let Err(error) = self
                            .micro3d
                            .runtime
                            .update_light(receiver.to_raw(), |current| *current = state)
                        {
                            return self.micro3d_failure(error, args);
                        }
                        None
                    }
                    "setAmbIntensity" | "setAmbientIntensity" => {
                        state.ambient_intensity = int_argument(args, 1)?;
                        if let Err(error) = self
                            .micro3d
                            .runtime
                            .update_light(receiver.to_raw(), |current| *current = state)
                        {
                            return self.micro3d_failure(error, args);
                        }
                        None
                    }
                    "setDirection" | "setParallelLightDirection" => {
                        state.direction = self.micro3d_vector(reference_argument(args, 1)?)?;
                        if let Err(error) = self
                            .micro3d
                            .runtime
                            .update_light(receiver.to_raw(), |current| *current = state)
                        {
                            return self.micro3d_failure(error, args);
                        }
                        None
                    }
                    _ => return Err(vm_error("method-not-found", display_key(&method.key))),
                }
            }
            "com/mascotcapsule/micro3d/v3/Effect3D" => {
                match self.micro3d_effect_call(receiver, name, args) {
                    Ok(outcome) => outcome,
                    Err(error) if error.category() == Category::Micro3d => {
                        return self.micro3d_failure(error, args);
                    }
                    Err(error) => return Err(error),
                }
            }
            "com/mascotcapsule/micro3d/v3/FigureLayout" => {
                match self.micro3d_layout_call(receiver, name, descriptor, args) {
                    Ok(outcome) => outcome,
                    Err(error) if error.category() == Category::Micro3d => {
                        return self.micro3d_failure(error, args);
                    }
                    Err(error) => return Err(error),
                }
            }
            "com/mascotcapsule/micro3d/v3/Figure" => {
                match self.micro3d_figure_call(receiver, name, args) {
                    Ok(outcome) => outcome,
                    Err(error) if error.category() == Category::Micro3d => {
                        return self.micro3d_failure(error, args);
                    }
                    Err(error) => return Err(error),
                }
            }
            "com/mascotcapsule/micro3d/v3/Graphics3D" => {
                return self.micro3d_graphics_call(receiver, name, descriptor, args);
            }
            _ => return Err(vm_error("method-not-found", display_key(&method.key))),
        };
        Ok(CallOutcome::Return(outcome))
    }
}
