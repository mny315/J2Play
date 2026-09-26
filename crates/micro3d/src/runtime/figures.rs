//! `Micro3D` figure rendering.

use super::{
    AffineTrans, CullMode, EffectState, EmuError, FIGURE_ATTR_BLEND_ADD, FIGURE_ATTR_BLEND_HALF,
    FIGURE_ATTR_BLEND_MASK, FIGURE_ATTR_BLEND_SUBTRACT, FIGURE_ATTR_DOUBLE_FACE,
    FIGURE_ATTR_TRANSPARENT, FigureLayoutState, FrameBlend, Lighting, ObjectKind,
    PreparedProjection, Runtime, Vertex, figure_geometry, figure_pattern_visible, object_kind,
    projected_vertex, runtime_error, sphere_texture,
};

impl Runtime {
    #[allow(clippy::needless_pass_by_value)]
    pub fn render_figure(
        &mut self,
        figure: u64,
        x: i32,
        y: i32,
        layout: FigureLayoutState,
        affine: AffineTrans,
        effect: EffectState,
    ) -> Result<(), EmuError> {
        let ObjectKind::Figure(state) = object_kind(&self.objects, figure)? else {
            return Err(runtime_error("type", "renderFigure requires a Figure"));
        };
        let textures = if state.textures.is_empty() && state.data.material_count != 0 {
            // An unbound Figure can use the sole model Texture in the suite.
            // Multiple candidates leave it untextured; explicit bindings win.
            let mut candidates = self
                .objects
                .values()
                .filter_map(|object| match &object.kind {
                    Some(ObjectKind::Texture(texture)) if texture.for_model => Some(texture),
                    _ => None,
                });
            match (candidates.next(), candidates.next()) {
                (Some(texture), None) => vec![texture],
                _ => Vec::new(),
            }
        } else {
            state
                .textures
                .iter()
                .map(|guest| match object_kind(&self.objects, *guest)? {
                    ObjectKind::Texture(texture) if texture.for_model => Ok(texture),
                    ObjectKind::Texture(_) => Err(runtime_error(
                        "texture-kind",
                        "Figure requires model textures",
                    )),
                    _ => Err(runtime_error(
                        "type",
                        "Figure texture has the wrong object type",
                    )),
                })
                .collect::<Result<Vec<_>, EmuError>>()?
        };
        let light = match effect.light {
            Some(guest) => match object_kind(&self.objects, guest)? {
                ObjectKind::Light(light) => Some(*light),
                _ => return Err(runtime_error("type", "Effect3D light requires a Light")),
            },
            None => None,
        }
        .map(|light| Lighting::new(light, effect));
        let sphere_texture = sphere_texture(&self.objects, effect.sphere_texture)?;
        let transform_normals = light.is_some() || sphere_texture.is_some();
        let (mut posed_vertices, mut posed_normals) = match state.posture {
            Some((action_guest, action_index, frame)) => {
                let ObjectKind::Action(action) = object_kind(&self.objects, action_guest)? else {
                    return Err(runtime_error(
                        "type",
                        "Figure posture requires an ActionTable",
                    ));
                };
                let action = action.actions.get(action_index).ok_or_else(|| {
                    runtime_error("action-index", "Figure posture action index is invalid")
                })?;
                figure_geometry(&state.data, Some((action, frame)), transform_normals)?
            }
            None => figure_geometry(&state.data, None, transform_normals)?,
        };
        for position in &mut posed_vertices {
            *position = affine.transform(*position);
        }
        let normal_transform = affine.normal_transform();
        // Adjacent faces share normals. Apply the layout transform once per
        // posed vertex, preserving the bone-then-layout rounding order.
        for normal in &mut posed_normals {
            *normal = normal_transform(*normal);
        }
        // Reflection coordinates depend only on the posed normal, not on the
        // face material. Normalize once per shared vertex instead of per face.
        let sphere_coordinates: Vec<_> = if sphere_texture.is_some() {
            posed_normals
                .iter()
                .copied()
                .map(super::rendering::sphere_uv)
                .collect()
        } else {
            Vec::new()
        };
        self.renderer.reset_stats();
        let mut configured_texture = None;
        let mut projection = None;
        for face in &state.data.faces {
            if !figure_pattern_visible(state.pattern, face.pattern) {
                continue;
            }
            let texture_index = face
                .material
                .and_then(|material| state.selected_texture.checked_add(usize::from(material)));
            let texture = texture_index.and_then(|index| textures.get(index)).copied();
            let color_key = face.attributes & FIGURE_ATTR_TRANSPARENT != 0;
            let renderer_texture = (texture_index, color_key);
            if configured_texture != Some(renderer_texture) {
                Self::configure_renderer(
                    &mut self.renderer,
                    texture,
                    sphere_texture,
                    effect,
                    None,
                    color_key,
                )?;
                configured_texture = Some(renderer_texture);
            }
            // Disabling Effect3D transparency draws all materials opaquely.
            let blending = if effect.transparency {
                match face.attributes & FIGURE_ATTR_BLEND_MASK {
                    FIGURE_ATTR_BLEND_HALF => FrameBlend::Half,
                    FIGURE_ATTR_BLEND_ADD => FrameBlend::Add,
                    FIGURE_ATTR_BLEND_SUBTRACT => FrameBlend::Subtract,
                    _ => FrameBlend::Replace,
                }
            } else {
                FrameBlend::Replace
            };
            self.renderer.set_compositing_mode(
                true,
                blending == FrameBlend::Replace,
                true,
                true,
                blending,
                0.0,
                0.0,
            )?;
            let projection = match &mut projection {
                Some(value) => value,
                empty => empty.insert(PreparedProjection::new(
                    &layout,
                    x.saturating_add(layout.center[0]),
                    y.saturating_add(layout.center[1]),
                    self.renderer.width(),
                    self.renderer.height(),
                )?),
            };
            let mut vertices = [Vertex::default(); 3];
            for (slot, (index, uv)) in face.indices.iter().zip(face.uv).enumerate() {
                let index = usize::try_from(*index).unwrap_or(usize::MAX);
                let source = posed_vertices.get(index).copied().ok_or_else(|| {
                    runtime_error("figure-index", "Figure face index became invalid")
                })?;
                let normal = posed_normals.get(index).copied();
                let base_color = face.color.unwrap_or(0xffff_ffff);
                let color = match (light.as_ref(), normal) {
                    (Some(light), Some(normal)) => light.shade(base_color, normal),
                    _ => base_color,
                };
                vertices[slot] = projected_vertex(
                    source,
                    uv,
                    projection,
                    texture,
                    sphere_coordinates.get(index).copied(),
                    color,
                );
            }
            // Clockwise faces are front-facing unless the material is two-sided.
            self.renderer
                .set_cull_mode(if face.attributes & FIGURE_ATTR_DOUBLE_FACE != 0 {
                    CullMode::None
                } else {
                    CullMode::Clockwise
                });
            self.renderer.draw_triangle(vertices)?;
        }
        self.renderer.set_cull_mode(CullMode::None);
        self.record_render_stats();
        Ok(())
    }
}
