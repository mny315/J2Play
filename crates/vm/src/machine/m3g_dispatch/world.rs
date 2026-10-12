use super::{
    CallOutcome, EmuError, Machine, Value, finite_float_argument, finite_non_negative,
    int_argument, optional_reference_argument, reference_argument, type_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn invoke_m3g_world_native(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let outcome = match (class, name, descriptor) {
            (
                "javax/microedition/m3g/World",
                "setActiveCamera",
                "(Ljavax/microedition/m3g/Camera;)V",
            ) => {
                let world = self.m3g_receiver(args)?;
                let camera = self.m3g_handle(reference_argument(args, 1)?)?;
                self.m3g.runtime.set_world_camera(world, camera)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/World",
                "setBackground",
                "(Ljavax/microedition/m3g/Background;)V",
            ) => {
                let world = self.m3g_receiver(args)?;
                let background = optional_reference_argument(args, 1)?
                    .map(|value| self.m3g_handle(value))
                    .transpose()?;
                self.m3g.runtime.set_world_background(world, background)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/World",
                "getActiveCamera",
                "()Ljavax/microedition/m3g/Camera;",
            )
            | (
                "javax/microedition/m3g/World",
                "getBackground",
                "()Ljavax/microedition/m3g/Background;",
            ) => {
                let world = self.m3g_receiver(args)?;
                let reference = match self.m3g.runtime.kind(world)? {
                    m3g::ObjectKind::World {
                        camera, background, ..
                    } => {
                        if name == "getActiveCamera" {
                            *camera
                        } else {
                            *background
                        }
                    }
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(reference))))
            }
            ("javax/microedition/m3g/Light", "setMode", "(I)V")
            | ("javax/microedition/m3g/Light", "setColor", "(I)V")
            | ("javax/microedition/m3g/Light", "setIntensity", "(F)V")
            | ("javax/microedition/m3g/Light", "setSpotAngle", "(F)V")
            | ("javax/microedition/m3g/Light", "setSpotExponent", "(F)V") => {
                let handle = self.m3g_receiver(args)?;
                let light = match self.m3g.runtime.kind_mut(handle)? {
                    m3g::ObjectKind::Light { light, .. } => light,
                    _ => return Err(type_error()),
                };
                match name {
                    "setMode" => {
                        let mode = int_argument(args, 1)?;
                        if !(128..=131).contains(&mode) {
                            return self.thread_exception(
                                "java/lang/IllegalArgumentException",
                                Some("invalid Light mode"),
                            );
                        }
                        light.mode = mode;
                    }
                    "setColor" => light.color = int_argument(args, 1)? as u32 & 0x00ff_ffff,
                    "setIntensity" => light.intensity = finite_float_argument(args, 1)?,
                    "setSpotAngle" => {
                        let angle = finite_non_negative(args, 1)?;
                        if angle > 90.0 {
                            return self.thread_exception(
                                "java/lang/IllegalArgumentException",
                                Some("Light spot angle exceeds 90 degrees"),
                            );
                        }
                        light.spot_angle = angle;
                    }
                    _ => {
                        let exponent = finite_non_negative(args, 1)?;
                        if exponent > 128.0 {
                            return self.thread_exception(
                                "java/lang/IllegalArgumentException",
                                Some("Light spot exponent exceeds 128"),
                            );
                        }
                        light.spot_exponent = exponent;
                    }
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Light", "setAttenuation", "(FFF)V") => {
                let values = [
                    finite_non_negative(args, 1)?,
                    finite_non_negative(args, 2)?,
                    finite_non_negative(args, 3)?,
                ];
                if values == [0.0; 3] {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("Light attenuation cannot be all zero"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                match self.m3g.runtime.kind_mut(handle)? {
                    m3g::ObjectKind::Light { light, .. } => light.attenuation = values,
                    _ => return Err(type_error()),
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Light", getter, "()I")
                if matches!(getter, "getMode" | "getColor") =>
            {
                let handle = self.m3g_receiver(args)?;
                let light = match self.m3g.runtime.kind(handle)? {
                    m3g::ObjectKind::Light { light, .. } => light,
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Int(if getter == "getMode" {
                    light.mode
                } else {
                    light.color as i32
                })))
            }
            ("javax/microedition/m3g/Light", getter, "()F") => {
                let handle = self.m3g_receiver(args)?;
                let light = match self.m3g.runtime.kind(handle)? {
                    m3g::ObjectKind::Light { light, .. } => light,
                    _ => return Err(type_error()),
                };
                let value = match getter {
                    "getIntensity" => light.intensity,
                    "getSpotAngle" => light.spot_angle,
                    "getSpotExponent" => light.spot_exponent,
                    "getConstantAttenuation" => light.attenuation[0],
                    "getLinearAttenuation" => light.attenuation[1],
                    "getQuadraticAttenuation" => light.attenuation[2],
                    _ => {
                        return self.thread_exception(
                            "java/lang/UnsupportedOperationException",
                            Some("unsupported Light getter"),
                        );
                    }
                };
                CallOutcome::Return(Some(Value::Float(value)))
            }
            ("javax/microedition/m3g/Background", setter, "(Z)V")
                if matches!(setter, "setColorClearEnable" | "setDepthClearEnable") =>
            {
                let value = int_argument(args, 1)? != 0;
                let handle = self.m3g_receiver(args)?;
                let state = match self.m3g.runtime.kind_mut(handle)? {
                    m3g::ObjectKind::Background(state) => state,
                    _ => return Err(type_error()),
                };
                if setter == "setColorClearEnable" {
                    state.color_clear = value;
                } else {
                    state.depth_clear = value;
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Background", getter, "()Z") => {
                let handle = self.m3g_receiver(args)?;
                let state = match self.m3g.runtime.kind(handle)? {
                    m3g::ObjectKind::Background(state) => state,
                    _ => return Err(type_error()),
                };
                let value = if getter == "isColorClearEnabled" {
                    state.color_clear
                } else {
                    state.depth_clear
                };
                CallOutcome::Return(Some(Value::Int(i32::from(value))))
            }
            ("javax/microedition/m3g/Background", "setColor", "(I)V") => {
                let color = int_argument(args, 1)? as u32;
                let handle = self.m3g_receiver(args)?;
                match self.m3g.runtime.kind_mut(handle)? {
                    m3g::ObjectKind::Background(state) => state.color = color,
                    _ => return Err(type_error()),
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Background", "getColor", "()I") => {
                let handle = self.m3g_receiver(args)?;
                let color = match self.m3g.runtime.kind(handle)? {
                    m3g::ObjectKind::Background(state) => state.color,
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Int(color as i32)))
            }
            (
                "javax/microedition/m3g/Background",
                "setImage",
                "(Ljavax/microedition/m3g/Image2D;)V",
            ) => {
                let image = optional_reference_argument(args, 1)?
                    .map(|guest| self.m3g_handle(guest))
                    .transpose()?;
                let crop = if let Some(image) = image {
                    let m3g::ObjectKind::Image2D(image) = self.m3g.runtime.kind(image)? else {
                        return Err(type_error());
                    };
                    [
                        0,
                        0,
                        i32::try_from(image.width()).unwrap_or(i32::MAX),
                        i32::try_from(image.height()).unwrap_or(i32::MAX),
                    ]
                } else {
                    [0; 4]
                };
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Background(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                // JSR-184 defines the crop rectangle as undefined until an
                // image is assigned. Assigning (or replacing) an image resets
                // it to the complete image; leaving the old/default zero crop
                // makes a valid background silently render no pixels.
                state.image = image;
                state.crop = crop;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Background",
                "getImage",
                "()Ljavax/microedition/m3g/Image2D;",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Background(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(state.image))))
            }
            ("javax/microedition/m3g/Background", "setCrop", "(IIII)V") => {
                let crop = [
                    int_argument(args, 1)?,
                    int_argument(args, 2)?,
                    int_argument(args, 3)?,
                    int_argument(args, 4)?,
                ];
                let handle = self.m3g_receiver(args)?;
                m3g::BackgroundState::validate_crop(crop)?;
                match self.m3g.runtime.kind_mut(handle)? {
                    m3g::ObjectKind::Background(state) => state.crop = crop,
                    _ => return Err(type_error()),
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Background", getter, "()I") => {
                let handle = self.m3g_receiver(args)?;
                let state = match self.m3g.runtime.kind(handle)? {
                    m3g::ObjectKind::Background(state) => state,
                    _ => return Err(type_error()),
                };
                let value = match getter {
                    "getImageModeX" => state.mode_x,
                    "getImageModeY" => state.mode_y,
                    "getCropX" => state.crop[0],
                    "getCropY" => state.crop[1],
                    "getCropWidth" => state.crop[2],
                    "getCropHeight" => state.crop[3],
                    _ => {
                        return self.thread_exception(
                            "java/lang/UnsupportedOperationException",
                            Some("unsupported Background getter"),
                        );
                    }
                };
                CallOutcome::Return(Some(Value::Int(value)))
            }
            ("javax/microedition/m3g/Background", "setImageMode", "(II)V") => {
                let modes = [int_argument(args, 1)?, int_argument(args, 2)?];
                if !modes.iter().all(|mode| matches!(mode, 32 | 33)) {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid Background image mode"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                match self.m3g.runtime.kind_mut(handle)? {
                    m3g::ObjectKind::Background(state) => {
                        state.mode_x = modes[0];
                        state.mode_y = modes[1];
                    }
                    _ => return Err(type_error()),
                }
                CallOutcome::Return(None)
            }
            _ => return self.m3g_unsupported_native(),
        };
        Ok(outcome)
    }
}
