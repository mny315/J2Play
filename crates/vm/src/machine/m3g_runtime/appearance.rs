use super::{EmuError, Machine, type_error, vm_error};

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_apply_appearance(
        &mut self,
        appearance: m3g::Handle,
    ) -> Result<(), EmuError> {
        self.m3g_apply_appearance_components(appearance, false)
    }

    pub(in crate::machine) fn m3g_apply_sprite_appearance(
        &mut self,
        appearance: m3g::Handle,
    ) -> Result<(), EmuError> {
        self.m3g_apply_appearance_components(appearance, true)
    }

    fn m3g_apply_appearance_components(
        &mut self,
        appearance: m3g::Handle,
        sprite: bool,
    ) -> Result<(), EmuError> {
        let mut textures = [None, None];
        let mut fog = None;
        let mut compositing = m3g::CompositingModeState::default();
        let mut polygon = m3g::PolygonModeState::default();
        let m3g::ObjectKind::Appearance(state) = self.m3g.runtime.kind(appearance)? else {
            return Err(type_error());
        };
        let texture_handles = if sprite { [None; 2] } else { state.textures };
        let fog_handle = state.fog;
        let compositing_handle = state.compositing_mode;
        let polygon_handle = if sprite { None } else { state.polygon_mode };
        if let Some(handle) = fog_handle {
            let m3g::ObjectKind::Fog(state) = self.m3g.runtime.kind(handle)? else {
                return Err(type_error());
            };
            fog = Some(*state);
        }
        if let Some(handle) = compositing_handle {
            let m3g::ObjectKind::CompositingMode(state) = self.m3g.runtime.kind(handle)? else {
                return Err(type_error());
            };
            compositing = *state;
        }
        if let Some(handle) = polygon_handle {
            let m3g::ObjectKind::PolygonMode(state) = self.m3g.runtime.kind(handle)? else {
                return Err(type_error());
            };
            polygon = *state;
        }
        for (unit, handle) in texture_handles.into_iter().enumerate() {
            let Some(handle) = handle else { continue };
            if unit >= self.limits.m3g_num_texture_units {
                return Err(vm_error(
                    "index-out-of-bounds-exception",
                    "appearance uses a texture unit unavailable in the active profile",
                ));
            }
            let m3g::ObjectKind::Texture2D(state) = self.m3g.runtime.kind(handle)? else {
                return Err(type_error());
            };
            let m3g::ObjectKind::Image2D(image) = self.m3g.runtime.kind(state.image)? else {
                return Err(type_error());
            };
            let mut decoded = m3g::Texture2DState::new(image.clone());
            if state.level_filter != 208
                && let Some(previous) = self.m3g.graphics.renderer.texture_unit(unit)
            {
                decoded.reuse_mipmaps(previous);
            }
            decoded.set_wrapping(
                if state.wrap_s == 240 {
                    m3g::WrapMode::Clamp
                } else {
                    m3g::WrapMode::Repeat
                },
                if state.wrap_t == 240 {
                    m3g::WrapMode::Clamp
                } else {
                    m3g::WrapMode::Repeat
                },
            );
            decoded.set_linear_filter(state.image_filter == 209);
            decoded.set_mipmap_filter(state.level_filter != 208, state.level_filter == 209);
            decoded.set_blend_function(match state.blending {
                224 => m3g::BlendFunction::Add,
                225 => m3g::BlendFunction::Blend,
                226 => m3g::BlendFunction::Decal,
                228 => m3g::BlendFunction::Replace,
                _ => m3g::BlendFunction::Modulate,
            });
            decoded.set_blend_color(state.blend_color);
            decoded.set_transform(self.m3g.runtime.composite_transform(handle)?);
            textures[unit] = Some(decoded);
        }
        self.m3g.metrics.texture_binds =
            self.m3g.metrics.texture_binds.saturating_add(
                u64::try_from(textures.iter().flatten().count()).unwrap_or(u64::MAX),
            );
        self.m3g.graphics.renderer.set_texture_units(textures)?;
        self.m3g.graphics.renderer.set_fog(fog);
        self.m3g
            .graphics
            .renderer
            .set_alpha_threshold((compositing.alpha_threshold * 255.0).round() as u8);
        let blending = match compositing.blending {
            64 => m3g::FrameBlend::Alpha,
            65 => m3g::FrameBlend::AlphaAdd,
            66 => m3g::FrameBlend::Modulate,
            67 => m3g::FrameBlend::ModulateX2,
            _ => m3g::FrameBlend::Replace,
        };
        self.m3g.graphics.renderer.set_compositing_mode(
            compositing.depth_test && self.m3g.graphics.depth_enabled,
            compositing.depth_write && self.m3g.graphics.depth_enabled,
            compositing.color_write,
            compositing.alpha_write,
            blending,
            compositing.depth_offset_factor,
            compositing.depth_offset_units,
        )?;
        let cull = match polygon.culling {
            162 => m3g::CullMode::None,
            160 if polygon.winding == 168 => m3g::CullMode::CounterClockwise,
            160 => m3g::CullMode::Clockwise,
            161 if polygon.winding == 168 => m3g::CullMode::Clockwise,
            _ => m3g::CullMode::CounterClockwise,
        };
        self.m3g.graphics.renderer.set_cull_mode(cull);
        self.m3g
            .graphics
            .renderer
            .set_smooth_shading(polygon.shading == 165);
        Ok(())
    }

    pub(in crate::machine) fn m3g_material_state(
        &self,
        appearance: m3g::Handle,
    ) -> Result<Option<m3g::MaterialObjectState>, EmuError> {
        let m3g::ObjectKind::Appearance(appearance) = self.m3g.runtime.kind(appearance)? else {
            return Err(type_error());
        };
        let Some(material) = appearance.material else {
            return Ok(None);
        };
        match self.m3g.runtime.kind(material)? {
            m3g::ObjectKind::Material(material) => Ok(Some(*material)),
            _ => Err(type_error()),
        }
    }

    pub(in crate::machine) fn m3g_light_sources(
        &self,
        scope: u32,
    ) -> Result<Vec<m3g::LightSource>, EmuError> {
        let mut output = Vec::with_capacity(self.m3g.graphics.lights.len());
        for (guest, transform) in &self.m3g.graphics.lights {
            let handle = self.m3g_handle(*guest)?;
            let m3g::ObjectKind::Light { light, .. } = self.m3g.runtime.kind(handle)? else {
                return Err(type_error());
            };
            let (rendering, _, light_scope, _) = self.m3g.runtime.node_state(handle)?;
            if !rendering || light_scope & scope == 0 {
                continue;
            }
            let origin = transform.transform(m3g::Vec4::new(0.0, 0.0, 0.0, 1.0));
            let forward = transform.transform(m3g::Vec4::new(0.0, 0.0, -1.0, 0.0));
            let position = m3g::Vec3::new(origin.x, origin.y, origin.z);
            let direction = m3g::Vec3::new(forward.x, forward.y, forward.z);
            output.push(match light.mode {
                128 => m3g::LightSource::Ambient {
                    color: light.color,
                    intensity: light.intensity,
                },
                129 => m3g::LightSource::Directional {
                    direction: m3g::Vec3::new(-direction.x, -direction.y, -direction.z),
                    color: light.color,
                    intensity: light.intensity,
                },
                130 => m3g::LightSource::Omni {
                    position,
                    color: light.color,
                    intensity: light.intensity,
                    attenuation: light.attenuation,
                },
                131 => m3g::LightSource::Spot {
                    position,
                    direction,
                    color: light.color,
                    intensity: light.intensity,
                    attenuation: light.attenuation,
                    angle_degrees: light.spot_angle,
                    exponent: light.spot_exponent,
                },
                _ => return Err(vm_error("illegal-state-exception", "invalid Light mode")),
            });
        }
        Ok(output)
    }
}
