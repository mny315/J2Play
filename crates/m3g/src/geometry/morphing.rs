//! Validated morph targets and reusable floating-point vertex attributes.

use super::{
    EmuError, LightSource, Mat4, MaterialState, Vec3, Vec4, Vertex, VertexArrayState,
    VertexBufferState, geometry_error, geometry_limit,
    vertices::{VertexLighting, interpolate_normal},
};

// Bound the product of targets, vertices and components, including repeated
// references to shared arrays that consume little scene-graph storage.
const MAX_MORPH_COMPONENTS: usize = 16_777_216;

/// Morphed local attributes shared by submeshes, lighting and picking.
/// Fractional components remain floats until the renderer consumes them.
#[derive(Debug)]
pub struct MorphedVertices {
    vertices: Vec<Vertex>,
    normals: Option<Vec<Vec4>>,
}

impl MorphedVertices {
    /// Number of prepared vertices.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Whether the base mesh supplies normals, morphed or unchanged.
    #[must_use]
    pub const fn has_normals(&self) -> bool {
        self.normals.is_some()
    }

    /// Interpolates prepared normals in mesh coordinates, using +Z when undefined.
    pub fn interpolated_normal(
        &self,
        indices: [usize; 3],
        weights: [f32; 3],
    ) -> Result<Vec3, EmuError> {
        let Some(normals) = &self.normals else {
            return Ok(Vec3::new(0.0, 0.0, 1.0));
        };
        let normal = |index| {
            normals.get(index).copied().ok_or_else(|| {
                geometry_error("invalid-index", "normal index exceeds morphed vertex count")
            })
        };
        let [a, b, c] = indices;
        interpolate_normal([normal(a)?, normal(b)?, normal(c)?], weights)
    }

    /// Transforms the prepared positions without repeating the morph calculation.
    #[must_use]
    pub fn transformed_vertices(&self, transform: Mat4) -> Vec<Vertex> {
        self.vertices
            .iter()
            .map(|vertex| {
                let mut vertex = *vertex;
                vertex.position = transform.transform(vertex.position);
                vertex
            })
            .collect()
    }

    /// Lights the prepared normals and positions, producing camera-space vertices.
    pub fn transformed_lit_vertices(
        &self,
        model: Mat4,
        view_transform: Mat4,
        camera_position: Vec3,
        material: MaterialState,
        vertex_color_tracking: bool,
        lights: &[LightSource],
        alpha: f32,
    ) -> Result<Vec<Vertex>, EmuError> {
        let normals = self.normals.as_ref().ok_or_else(|| {
            geometry_error("missing-normals", "lit morph base has no normal array")
        })?;
        let lighting = VertexLighting::new(
            model,
            view_transform,
            camera_position,
            material,
            vertex_color_tracking,
            lights,
            alpha,
        )?;
        self.vertices
            .iter()
            .zip(normals)
            .map(|(vertex, normal)| lighting.apply(*vertex, *normal))
            .collect()
    }
}

struct MorphArray<'a> {
    base: &'a VertexArrayState,
    targets: Vec<(&'a VertexArrayState, f64)>,
}

impl MorphArray<'_> {
    // The caller selects its output width. RGB colors and 2D texture
    // coordinates leave their unused trailing component at zero.
    fn vertex<const N: usize>(&self, vertex: usize, unsigned: bool) -> Result<[f32; N], EmuError> {
        let read = |value: i16| {
            if unsigned {
                f64::from(value as u8)
            } else {
                f64::from(value)
            }
        };
        let components = self.base.vertex_components(vertex)?;
        let base: [f64; N] =
            std::array::from_fn(|slot| read(components.get(slot).copied().unwrap_or(0)));
        let mut difference = [0.0; N];
        for &(target, weight) in &self.targets {
            let components = target.vertex_components(vertex)?;
            for slot in 0..N {
                difference[slot] +=
                    weight * (read(components.get(slot).copied().unwrap_or(0)) - base[slot]);
            }
        }
        // Accumulate differences before adding the base. Equal targets then
        // remain exact even when weights or their sum are very large.
        let values = std::array::from_fn(|index| (base[index] + difference[index]) as f32);
        if !values.iter().all(|value| value.is_finite()) {
            return Err(geometry_error(
                "invalid-object-graph",
                "morphed attribute exceeds the numeric range",
            ));
        }
        Ok(values)
    }
}

impl VertexBufferState {
    /// Validates all target arrays, then morphs raw components using base scale/bias.
    pub fn morphed_vertices(
        &self,
        targets: &[Self],
        weights: &[f32],
    ) -> Result<MorphedVertices, EmuError> {
        let count = self.position_count()?;
        let Some(first) = targets.first() else {
            return Err(geometry_error(
                "invalid-object-graph",
                "morph target array is empty",
            ));
        };
        if targets.len() != weights.len() || !weights.iter().all(|weight| weight.is_finite()) {
            return Err(geometry_error(
                "invalid-object-graph",
                "morph target and weight arrays are incompatible",
            ));
        }
        if targets.len() > u16::MAX as usize {
            return Err(geometry_limit());
        }
        let base_arrays = self.attribute_arrays();
        let target_arrays = first.attribute_arrays();
        for target in targets {
            for ((array, expected), base) in target
                .attribute_arrays()
                .into_iter()
                .zip(target_arrays)
                .zip(base_arrays)
            {
                let shape = |array: &VertexArrayState| {
                    (
                        array.vertex_count(),
                        array.component_count(),
                        array.component_type(),
                    )
                };
                if array.map(shape) != expected.map(shape)
                    || array.is_some_and(|array| Some(shape(array)) != base.map(shape))
                {
                    return Err(geometry_error(
                        "invalid-object-graph",
                        "morph targets must have matching arrays that are a subset of the base",
                    ));
                }
            }
        }
        let active = weights.iter().filter(|weight| **weight != 0.0).count();
        let components = target_arrays
            .into_iter()
            .flatten()
            .map(VertexArrayState::component_count)
            .sum::<usize>();
        if (components * count + 4)
            .checked_mul(active)
            .is_none_or(|work| work > MAX_MORPH_COMPONENTS)
        {
            return Err(geometry_limit());
        }
        let arrays: [Option<MorphArray<'_>>; 5] = std::array::from_fn(|slot| {
            base_arrays[slot].map(|base| MorphArray {
                base,
                targets: if target_arrays[slot].is_some() {
                    targets
                        .iter()
                        .zip(weights)
                        .filter(|(_, weight)| **weight != 0.0)
                        .map(|(target, weight)| {
                            (
                                target.attribute_arrays()[slot].expect("validated morph attribute"),
                                f64::from(*weight),
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                },
            })
        });
        let default_color = [24, 16, 8, 0].into_iter().fold(0, |color, shift| {
            let base = f64::from((self.default_color >> shift) & 0xff);
            let difference = targets
                .iter()
                .zip(weights)
                .map(|(target, weight)| {
                    f64::from(*weight) * (f64::from((target.default_color >> shift) & 0xff) - base)
                })
                .sum::<f64>();
            color | ((base + difference).clamp(0.0, 255.0).round() as u32) << shift
        });
        let positions = arrays[0].as_ref().expect("required base positions");
        let (_, scale, bias) = self.positions.as_ref().expect("required base positions");
        let mut vertices = Vec::with_capacity(count);
        let mut normals = arrays[1].as_ref().map(|_| Vec::with_capacity(count));
        for index in 0..count {
            let [x, y, z] = positions.vertex(index, false)?;
            let position = Vec4::new(
                x * *scale + bias[0],
                y * *scale + bias[1],
                z * *scale + bias[2],
                1.0,
            );
            if let (Some(array), Some(normals)) = (&arrays[1], &mut normals) {
                let [x, y, z] = array
                    .vertex(index, false)?
                    .map(|value| array.base.component_type().normal(value));
                normals.push(Vec4::new(x, y, z, 0.0));
            }
            let color = if let Some(colors) = &arrays[2] {
                let [red, green, blue, alpha] = colors
                    .vertex(index, true)?
                    .map(|value| value.clamp(0.0, 255.0).round() as u32);
                let alpha = if colors.base.component_count() == 4 {
                    alpha
                } else {
                    255
                };
                alpha << 24 | red << 16 | green << 8 | blue
            } else {
                default_color
            };
            let mut textures = [[0.0, 0.0, 1.0]; 2];
            for (unit, texture) in textures.iter_mut().enumerate() {
                if let (Some(array), Some((_, scale, bias))) =
                    (&arrays[3 + unit], &self.texture_coordinates[unit])
                {
                    *texture = [0.0; 3];
                    let values = array.vertex::<3>(index, false)?;
                    for component in 0..array.base.component_count() {
                        texture[component] = values[component] * *scale + bias[component];
                    }
                }
            }
            vertices.push(Vertex::with_textures(position, color, textures));
        }
        Ok(MorphedVertices { vertices, normals })
    }
}
