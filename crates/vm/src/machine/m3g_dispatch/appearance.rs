use super::{
    CallOutcome, EmuError, Machine, Value, finite_non_negative, float_argument, int_argument,
    optional_reference_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn invoke_m3g_appearance_native(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let outcome = match (class, name, descriptor) {
            ("javax/microedition/m3g/Appearance", "setLayer", "(I)V") => {
                let handle = self.m3g_receiver(args)?;
                let layer = int_argument(args, 1)?;
                if !(-63..=63).contains(&layer) {
                    return self.thread_exception(
                        "java/lang/IndexOutOfBoundsException",
                        Some("Appearance layer is outside -63..63"),
                    );
                }
                let m3g::ObjectKind::Appearance(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.layer = layer;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Appearance", "getLayer", "()I") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Appearance(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Int(state.layer)))
            }
            (
                "javax/microedition/m3g/Appearance",
                "setFog" | "setPolygonMode" | "setCompositingMode" | "setMaterial",
                _,
            ) => {
                let handle = self.m3g_receiver(args)?;
                let value = optional_reference_argument(args, 1)?
                    .map(|guest| self.m3g_handle(guest))
                    .transpose()?;
                if let Some(value) = value {
                    let valid = matches!(
                        (name, self.m3g.runtime.kind(value)?),
                        ("setFog", m3g::ObjectKind::Fog(_))
                            | ("setPolygonMode", m3g::ObjectKind::PolygonMode(_))
                            | ("setCompositingMode", m3g::ObjectKind::CompositingMode(_))
                            | ("setMaterial", m3g::ObjectKind::Material(_))
                    );
                    if !valid {
                        return Err(type_error());
                    }
                }
                let m3g::ObjectKind::Appearance(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                match name {
                    "setFog" => state.fog = value,
                    "setPolygonMode" => state.polygon_mode = value,
                    "setCompositingMode" => state.compositing_mode = value,
                    _ => state.material = value,
                }
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Appearance",
                "getFog" | "getPolygonMode" | "getCompositingMode" | "getMaterial",
                _,
            ) => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Appearance(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                let value = match name {
                    "getFog" => state.fog,
                    "getPolygonMode" => state.polygon_mode,
                    "getCompositingMode" => state.compositing_mode,
                    _ => state.material,
                };
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(value))))
            }
            (
                "javax/microedition/m3g/Appearance",
                "setTexture",
                "(ILjavax/microedition/m3g/Texture2D;)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let unit = self.m3g_texture_unit(int_argument(args, 1)?)?;
                let value = optional_reference_argument(args, 2)?
                    .map(|guest| self.m3g_handle(guest))
                    .transpose()?;
                if let Some(value) = value
                    && !matches!(self.m3g.runtime.kind(value)?, m3g::ObjectKind::Texture2D(_))
                {
                    return Err(type_error());
                }
                let m3g::ObjectKind::Appearance(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                let slot = state.textures.get_mut(unit).ok_or_else(|| {
                    vm_error(
                        "index-out-of-bounds-exception",
                        "texture unit exceeds the active profile limit",
                    )
                })?;
                *slot = value;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Appearance",
                "getTexture",
                "(I)Ljavax/microedition/m3g/Texture2D;",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let unit = self.m3g_texture_unit(int_argument(args, 1)?)?;
                let m3g::ObjectKind::Appearance(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                let value = *state.textures.get(unit).ok_or_else(|| {
                    vm_error(
                        "index-out-of-bounds-exception",
                        "texture unit exceeds the active profile limit",
                    )
                })?;
                CallOutcome::Return(Some(Value::Reference(self.m3g_guest_handle(value))))
            }
            ("javax/microedition/m3g/CompositingMode", "setBlending", "(I)V") => {
                let mode = int_argument(args, 1)?;
                if !matches!(mode, 64..=68) {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid CompositingMode blending mode"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::CompositingMode(state) = self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.blending = mode;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/CompositingMode", "setAlphaThreshold", "(F)V") => {
                let threshold = float_argument(args, 1)?;
                if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("alpha threshold must be in 0..1"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::CompositingMode(state) = self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.alpha_threshold = threshold;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/CompositingMode",
                "setAlphaWriteEnable"
                | "setColorWriteEnable"
                | "setDepthWriteEnable"
                | "setDepthTestEnable",
                "(Z)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let enabled = int_argument(args, 1)? != 0;
                let m3g::ObjectKind::CompositingMode(state) = self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                match name {
                    "setAlphaWriteEnable" => state.alpha_write = enabled,
                    "setColorWriteEnable" => state.color_write = enabled,
                    "setDepthWriteEnable" => state.depth_write = enabled,
                    _ => state.depth_test = enabled,
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/CompositingMode", "setDepthOffset", "(FF)V") => {
                let factor = float_argument(args, 1)?;
                let units = float_argument(args, 2)?;
                if !factor.is_finite() || !units.is_finite() {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("depth offset must be finite"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::CompositingMode(state) = self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.depth_offset_factor = factor;
                state.depth_offset_units = units;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/CompositingMode",
                "getBlending"
                | "getAlphaThreshold"
                | "isAlphaWriteEnabled"
                | "isColorWriteEnabled"
                | "isDepthWriteEnabled"
                | "isDepthTestEnabled"
                | "getDepthOffsetFactor"
                | "getDepthOffsetUnits",
                _,
            ) => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::CompositingMode(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                match name {
                    "getAlphaThreshold" => {
                        CallOutcome::Return(Some(Value::Float(state.alpha_threshold)))
                    }
                    "getDepthOffsetFactor" => {
                        CallOutcome::Return(Some(Value::Float(state.depth_offset_factor)))
                    }
                    "getDepthOffsetUnits" => {
                        CallOutcome::Return(Some(Value::Float(state.depth_offset_units)))
                    }
                    "getBlending" => CallOutcome::Return(Some(Value::Int(state.blending))),
                    "isAlphaWriteEnabled" => {
                        CallOutcome::Return(Some(Value::Int(i32::from(state.alpha_write))))
                    }
                    "isColorWriteEnabled" => {
                        CallOutcome::Return(Some(Value::Int(i32::from(state.color_write))))
                    }
                    "isDepthWriteEnabled" => {
                        CallOutcome::Return(Some(Value::Int(i32::from(state.depth_write))))
                    }
                    _ => CallOutcome::Return(Some(Value::Int(i32::from(state.depth_test)))),
                }
            }
            (
                "javax/microedition/m3g/PolygonMode",
                "setCulling" | "setWinding" | "setShading",
                "(I)V",
            ) => {
                let value = int_argument(args, 1)?;
                let valid = match name {
                    "setCulling" => matches!(value, 160..=162),
                    "setWinding" => matches!(value, 168 | 169),
                    _ => matches!(value, 164 | 165),
                };
                if !valid {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid PolygonMode enum"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::PolygonMode(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                match name {
                    "setCulling" => state.culling = value,
                    "setWinding" => state.winding = value,
                    _ => state.shading = value,
                }
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/PolygonMode",
                "setTwoSidedLightingEnable"
                | "setLocalCameraLightingEnable"
                | "setPerspectiveCorrectionEnable",
                "(Z)V",
            ) => {
                let enabled = int_argument(args, 1)? != 0;
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::PolygonMode(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                match name {
                    "setTwoSidedLightingEnable" => state.two_sided_lighting = enabled,
                    "setLocalCameraLightingEnable" => state.local_camera_lighting = enabled,
                    _ => state.perspective_correction = enabled,
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/PolygonMode", _, "()I" | "()Z") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::PolygonMode(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                let value = match name {
                    "getCulling" => state.culling,
                    "getWinding" => state.winding,
                    "getShading" => state.shading,
                    "isTwoSidedLightingEnabled" => i32::from(state.two_sided_lighting),
                    "isLocalCameraLightingEnabled" => i32::from(state.local_camera_lighting),
                    "isPerspectiveCorrectionEnabled" => i32::from(state.perspective_correction),
                    _ => {
                        return self.thread_exception(
                            "java/lang/UnsupportedOperationException",
                            Some("unknown PolygonMode getter"),
                        );
                    }
                };
                CallOutcome::Return(Some(Value::Int(value)))
            }
            ("javax/microedition/m3g/Fog", "setMode", "(I)V") => {
                let mode = int_argument(args, 1)?;
                let mode = match mode {
                    80 => m3g::FogMode::Exponential,
                    81 => m3g::FogMode::Linear,
                    _ => {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("invalid Fog mode"),
                        );
                    }
                };
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Fog(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.mode = mode;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Fog", "setLinear", "(FF)V") => {
                let near = float_argument(args, 1)?;
                let far = float_argument(args, 2)?;
                if !near.is_finite() || !far.is_finite() {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid Fog linear distances"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Fog(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.near = near;
                state.far = far;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Fog", "setDensity", "(F)V") => {
                let density = finite_non_negative(args, 1)?;
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Fog(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.density = density;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Fog", "setColor", "(I)V") => {
                let color = int_argument(args, 1)?.cast_unsigned() & 0x00ff_ffff;
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Fog(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.color = color;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Fog", _, "()I" | "()F") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Fog(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                match name {
                    "getMode" => CallOutcome::Return(Some(Value::Int(match state.mode {
                        m3g::FogMode::Exponential => 80,
                        m3g::FogMode::Linear => 81,
                    }))),
                    "getColor" => CallOutcome::Return(Some(Value::Int(state.color.cast_signed()))),
                    "getNearDistance" => CallOutcome::Return(Some(Value::Float(state.near))),
                    "getFarDistance" => CallOutcome::Return(Some(Value::Float(state.far))),
                    "getDensity" => CallOutcome::Return(Some(Value::Float(state.density))),
                    _ => return Err(type_error()),
                }
            }
            ("javax/microedition/m3g/Material", "setColor", "(II)V") => {
                let target = int_argument(args, 1)?;
                if target == 0 || target & !(1024 | 2048 | 4096 | 8192) != 0 {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid Material color target mask"),
                    );
                }
                let color = int_argument(args, 2)?.cast_unsigned();
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Material(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                if target & 1024 != 0 {
                    state.material.ambient = color & 0x00ff_ffff;
                }
                if target & 2048 != 0 {
                    state.material.diffuse = color;
                }
                if target & 4096 != 0 {
                    state.material.emissive = color & 0x00ff_ffff;
                }
                if target & 8192 != 0 {
                    state.material.specular = color & 0x00ff_ffff;
                }
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Material", "getColor", "(I)I") => {
                let target = int_argument(args, 1)?;
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Material(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                let color = match target {
                    1024 => state.material.ambient,
                    2048 => state.material.diffuse,
                    4096 => state.material.emissive,
                    8192 => state.material.specular,
                    _ => {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("Material getColor requires one target"),
                        );
                    }
                };
                CallOutcome::Return(Some(Value::Int(color.cast_signed())))
            }
            ("javax/microedition/m3g/Material", "setShininess", "(F)V") => {
                let value = float_argument(args, 1)?;
                if !value.is_finite() || !(0.0..=128.0).contains(&value) {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("Material shininess must be in 0..128"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Material(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.material.shininess = value;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Material", "getShininess", "()F") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Material(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Float(state.material.shininess)))
            }
            ("javax/microedition/m3g/Material", "setVertexColorTrackingEnable", "(Z)V") => {
                let enabled = int_argument(args, 1)? != 0;
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Material(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.vertex_color_tracking = enabled;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Material", "isVertexColorTrackingEnabled", "()Z") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Material(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Int(i32::from(state.vertex_color_tracking))))
            }
            (
                "javax/microedition/m3g/Texture2D",
                "setImage",
                "(Ljavax/microedition/m3g/Image2D;)V",
            ) => {
                let image = self.m3g_handle(reference_argument(args, 1)?)?;
                self.m3g_validate_texture_image(image)?;
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Texture2D(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.image = image;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Texture2D",
                "getImage",
                "()Ljavax/microedition/m3g/Image2D;",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Texture2D(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                CallOutcome::Return(Some(Value::Reference(
                    self.m3g_guest_handle(Some(state.image)),
                )))
            }
            ("javax/microedition/m3g/Texture2D", "setFiltering", "(II)V") => {
                let level = int_argument(args, 1)?;
                let image = int_argument(args, 2)?;
                if !matches!(level, 208..=210) || !matches!(image, 209 | 210) {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid Texture2D filtering mode"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Texture2D(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.level_filter = level;
                state.image_filter = image;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Texture2D", "setWrapping", "(II)V") => {
                let s = int_argument(args, 1)?;
                let t = int_argument(args, 2)?;
                if !matches!(s, 240 | 241) || !matches!(t, 240 | 241) {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid Texture2D wrapping mode"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Texture2D(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.wrap_s = s;
                state.wrap_t = t;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Texture2D", "setBlending", "(I)V") => {
                let blending = int_argument(args, 1)?;
                if !(224..=228).contains(&blending) {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid Texture2D blend function"),
                    );
                }
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Texture2D(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.blending = blending;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Texture2D", "setBlendColor", "(I)V") => {
                let color = int_argument(args, 1)?.cast_unsigned() & 0x00ff_ffff;
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Texture2D(state) = self.m3g.runtime.kind_mut(handle)? else {
                    return Err(type_error());
                };
                state.blend_color = color;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Texture2D", _, "()I") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::Texture2D(state) = self.m3g.runtime.kind(handle)? else {
                    return Err(type_error());
                };
                let value = match name {
                    "getLevelFilter" => state.level_filter,
                    "getImageFilter" => state.image_filter,
                    "getWrappingS" => state.wrap_s,
                    "getWrappingT" => state.wrap_t,
                    "getBlending" => state.blending,
                    "getBlendColor" => state.blend_color.cast_signed(),
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Int(value)))
            }
            _ => return self.m3g_unsupported_native(),
        };
        Ok(outcome)
    }
}
