//! `Micro3D` primitive validation and rendering.

use super::{
    AffineTrans, CullMode, EffectState, EmuError, FigureLayoutState, LightState, Lighting,
    ObjectKind, PreparedProjection, Runtime, Vector3D, Vertex, object_kind,
    point_sprite_parameters, point_sprite_vertices, primitive_color, primitive_normal,
    projected_vertex, runtime_error,
};

/// Borrowed immediate-mode geometry submitted by `Graphics3D.renderPrimitives`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimitiveData<'a> {
    pub command: i32,
    pub count: usize,
    pub coordinates: &'a [i32],
    pub normals: &'a [i32],
    pub texture_coordinates: &'a [i32],
    pub colors: &'a [i32],
}

impl<'a> PrimitiveData<'a> {
    #[must_use]
    pub const fn new(
        command: i32,
        count: usize,
        coordinates: &'a [i32],
        normals: &'a [i32],
        texture_coordinates: &'a [i32],
        colors: &'a [i32],
    ) -> Self {
        Self {
            command,
            count,
            coordinates,
            normals,
            texture_coordinates,
            colors,
        }
    }
}

/// Exact array element counts required by one immediate-mode primitive command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimitivePayloadCounts {
    pub vertex_count: usize,
    pub normal_count: usize,
    pub texture_coordinate_count: usize,
    pub color_count: usize,
}

/// Validates a primitive command and returns its bounded payload shape.
pub fn primitive_payload_counts(
    command: i32,
    primitive_count: usize,
) -> Result<PrimitivePayloadCounts, EmuError> {
    if command < 0 {
        return Err(runtime_error(
            "primitive-command",
            "primitive command must not be negative",
        ));
    }
    if primitive_count == 0 || primitive_count > 255 {
        return Err(runtime_error(
            "primitive-count",
            "primitive count must be between 1 and 255",
        ));
    }
    let primitive_type = command.cast_unsigned() & 0x0f00_0000;
    let vertices_per_primitive = match primitive_type {
        0x0100_0000 | 0x0500_0000 => 1,
        0x0200_0000 => 2,
        0x0300_0000 => 3,
        0x0400_0000 => 4,
        value => {
            return Err(runtime_error(
                "primitive-type",
                format!("unsupported Micro3D primitive type 0x{value:08x}"),
            ));
        }
    };
    let vertex_count = primitive_count * vertices_per_primitive;
    let polygon = matches!(primitive_type, 0x0300_0000 | 0x0400_0000);
    let normal_count = if polygon {
        match command & 0x0300 {
            0 => 0,
            0x0200 => primitive_count * 3,
            0x0300 => vertex_count * 3,
            _ => {
                return Err(runtime_error(
                    "primitive-normals",
                    "primitive command contains a reserved normal-data selector",
                ));
            }
        }
    } else {
        0
    };
    let texture_coordinate_count = if primitive_type == 0x0500_0000 {
        match command & 0x3000 {
            0x1000 => 8,
            0x2000 | 0x3000 => primitive_count * 8,
            _ => {
                return Err(runtime_error(
                    "point-sprite-parameters",
                    "point sprites require a parameter layout",
                ));
            }
        }
    } else if polygon {
        match command & 0x3000 {
            0 => 0,
            0x3000 => vertex_count * 2,
            _ => {
                return Err(runtime_error(
                    "primitive-uv",
                    "primitive command contains a reserved texture-data selector",
                ));
            }
        }
    } else {
        0
    };
    let color_count = if primitive_type == 0x0500_0000 {
        0
    } else {
        match command & 0x0c00 {
            0 => 0,
            0x0400 => 1,
            0x0800 => primitive_count,
            _ => {
                return Err(runtime_error(
                    "primitive-color",
                    "primitive command contains a reserved color-data selector",
                ));
            }
        }
    };
    Ok(PrimitivePayloadCounts {
        vertex_count,
        normal_count,
        texture_coordinate_count,
        color_count,
    })
}

/// State overridden by commands while one display list is being processed.
///
/// `None` attributes preserve the supplied `Effect3D` switches. A present
/// value replaces those switches for the remainder of the command list while
/// primitive-local lighting and sphere-map bits can still opt individual
/// polygons in.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PrimitiveEnvironment {
    pub attributes: Option<u8>,
    pub light_override: Option<LightState>,
}

impl Runtime {
    #[allow(clippy::needless_pass_by_value)]
    pub fn render_primitives(
        &mut self,
        texture_guest: Option<u64>,
        x: i32,
        y: i32,
        layout: FigureLayoutState,
        affine: AffineTrans,
        effect: EffectState,
        data: PrimitiveData<'_>,
    ) -> Result<(), EmuError> {
        self.render_primitives_with_environment(
            texture_guest,
            x,
            y,
            layout,
            affine,
            effect,
            data,
            PrimitiveEnvironment::default(),
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn render_primitives_with_environment(
        &mut self,
        texture_guest: Option<u64>,
        x: i32,
        y: i32,
        layout: FigureLayoutState,
        affine: AffineTrans,
        mut effect: EffectState,
        data: PrimitiveData<'_>,
        environment: PrimitiveEnvironment,
    ) -> Result<(), EmuError> {
        let PrimitiveData {
            command,
            count: primitive_count,
            coordinates,
            normals,
            texture_coordinates,
            colors,
        } = data;
        // Immediate-mode primitives are two-sided. Do not inherit Figure
        // material state from an earlier renderFigure call.
        self.renderer.set_cull_mode(CullMode::None);
        let payload = primitive_payload_counts(command, primitive_count)?;
        let primitive_type = command.cast_unsigned() & 0x0f00_0000;
        let vertices_per_primitive = payload.vertex_count / primitive_count;
        let vertex_count = payload.vertex_count;
        if coordinates.len() < vertex_count * 3 {
            return Err(runtime_error(
                "primitive-coordinates",
                "vertex coordinate array is too short",
            ));
        }
        let polygon = matches!(primitive_type, 0x0300_0000 | 0x0400_0000);
        if normals.len() < payload.normal_count {
            return Err(runtime_error(
                "primitive-normals",
                "normal array is too short for the primitive command",
            ));
        }
        let texture = texture_guest
            .map(|guest| match object_kind(&self.objects, guest)? {
                ObjectKind::Texture(texture) if texture.for_model => Ok(texture),
                ObjectKind::Texture(_) => Err(runtime_error(
                    "texture-kind",
                    "renderPrimitives requires a model texture",
                )),
                _ => Err(runtime_error(
                    "type",
                    "renderPrimitives texture has the wrong type",
                )),
            })
            .transpose()?;
        let inline_attributes = command.cast_unsigned() as u8 & 0x0f;
        let attributes = environment
            .attributes
            .map_or(inline_attributes, |value| value | inline_attributes);
        if environment.attributes.is_some() {
            effect.shading = i32::from(attributes & 0x04 != 0);
            effect.transparency = attributes & 0x08 != 0;
        }
        let lighting = polygon && attributes & 0x01 != 0;
        let sphere_mapping = polygon && attributes & 0x02 != 0;
        let light = if lighting {
            match (environment.light_override, effect.light) {
                (Some(light), _) => Some(light),
                (None, Some(guest)) => match object_kind(&self.objects, guest)? {
                    ObjectKind::Light(light) => Some(*light),
                    _ => return Err(runtime_error("type", "Effect3D light requires a Light")),
                },
                (None, None) => None,
            }
        } else {
            None
        };
        let light = light.map(|light| Lighting::new(light, effect));
        let sphere_texture = if sphere_mapping {
            super::sphere_texture(&self.objects, effect.sphere_texture)?
        } else {
            None
        };
        let point_sprites = (primitive_type == 0x0500_0000)
            .then(|| point_sprite_parameters(command, primitive_count, texture_coordinates))
            .transpose()?;
        let has_uv = polygon && payload.texture_coordinate_count != 0;
        if texture_coordinates.len() < payload.texture_coordinate_count {
            return Err(runtime_error(
                "primitive-uv",
                "texture coordinate array is too short",
            ));
        }
        if colors.len() < payload.color_count {
            return Err(runtime_error(
                "primitive-color",
                "color array is too short for the primitive command",
            ));
        }
        Self::configure_renderer(
            &mut self.renderer,
            (primitive_type != 0x0100_0000).then_some(texture).flatten(),
            sphere_texture,
            effect,
            Some(command),
            false,
        )?;
        self.renderer.reset_stats();
        if let Some(parameters) = point_sprites {
            let mut projection = None;
            for (source, parameters) in coordinates.chunks_exact(3).zip(parameters) {
                let source = Vector3D::new(source[0], source[1], source[2]);
                // Zero-area point sprites are inactive batch slots.
                if parameters[0] == 0 || parameters[1] == 0 {
                    continue;
                }
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
                let vertices = match point_sprite_vertices(
                    source,
                    parameters,
                    projection,
                    affine,
                    texture,
                    self.renderer.width(),
                    self.renderer.height(),
                ) {
                    Ok(vertices) => vertices,
                    // Clip an undefined perspective projection without aborting the batch.
                    Err(error) if error.code() == "point-sprite-projection" => continue,
                    Err(error) => return Err(error),
                };
                self.renderer
                    .draw_triangle([vertices[0], vertices[1], vertices[2]])?;
                self.renderer
                    .draw_triangle([vertices[0], vertices[2], vertices[3]])?;
            }
            self.record_render_stats();
            return Ok(());
        }
        let projection = PreparedProjection::new(
            &layout,
            x.saturating_add(layout.center[0]),
            y.saturating_add(layout.center[1]),
            self.renderer.width(),
            self.renderer.height(),
        )?;
        if primitive_type == 0x0100_0000 {
            for primitive in 0..primitive_count {
                let base = primitive * 3;
                let source = affine.transform(Vector3D::new(
                    coordinates[base],
                    coordinates[base + 1],
                    coordinates[base + 2],
                ));
                let position = projection.project(source);
                let color = primitive_color(command, primitive, colors);
                self.renderer.draw_point(Vertex::new(position, color))?;
            }
            self.record_render_stats();
            return Ok(());
        }
        if primitive_type == 0x0200_0000 {
            for primitive in 0..primitive_count {
                let mut vertices = [Vertex::default(); 2];
                for (slot, destination) in vertices.iter_mut().enumerate() {
                    let index = primitive * 2 + slot;
                    let base = index * 3;
                    let source = affine.transform(Vector3D::new(
                        coordinates[base],
                        coordinates[base + 1],
                        coordinates[base + 2],
                    ));
                    let position = projection.project(source);
                    let color = primitive_color(command, primitive, colors);
                    *destination = Vertex::new(position, color);
                }
                self.renderer.draw_line(vertices)?;
            }
            self.record_render_stats();
            return Ok(());
        }
        let normal_transform = affine.normal_transform();
        let uses_normals = light.is_some() || sphere_texture.is_some();
        for primitive in 0..primitive_count {
            let base_color = primitive_color(command, primitive, colors);
            let shade = |index| {
                let normal = if uses_normals {
                    primitive_normal(command, primitive, index, normals).map(&normal_transform)
                } else {
                    None
                };
                let color = match (light.as_ref(), normal) {
                    (Some(light), Some(normal)) => light.shade(base_color, normal),
                    _ => base_color,
                };
                let sphere = normal
                    .filter(|_| sphere_texture.is_some())
                    .map(super::rendering::sphere_uv);
                (color, sphere)
            };
            // Per-primitive normals and colors produce identical shading at
            // every corner, including the reflection texture coordinates.
            let shared = (command & 0x0300 != 0x0300 || !uses_normals).then(|| shade(0));
            let mut polygon = [Vertex::default(); 4];
            for (slot, destination) in polygon.iter_mut().enumerate().take(vertices_per_primitive) {
                let index = primitive * vertices_per_primitive + slot;
                let base = index * 3;
                let source = affine.transform(Vector3D::new(
                    coordinates[base],
                    coordinates[base + 1],
                    coordinates[base + 2],
                ));
                let uv = if has_uv {
                    [
                        texture_coordinates[index * 2].cast_unsigned(),
                        texture_coordinates[index * 2 + 1].cast_unsigned(),
                    ]
                } else {
                    [0, 0]
                };
                let (color, sphere_coordinates) = shared.unwrap_or_else(|| shade(index));
                *destination =
                    projected_vertex(source, uv, &projection, texture, sphere_coordinates, color);
            }
            self.renderer
                .draw_triangle([polygon[0], polygon[1], polygon[2]])?;
            if vertices_per_primitive == 4 {
                self.renderer
                    .draw_triangle([polygon[0], polygon[2], polygon[3]])?;
            }
        }
        self.record_render_stats();
        Ok(())
    }
}
