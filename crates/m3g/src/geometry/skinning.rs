//! Bone range selection and weighted deformation shared by rendering and picking.

use super::{
    EmuError, LightSource, Mat4, MaterialState, Vec3, Vec4, Vertex, VertexBufferState,
    geometry_error, geometry_limit,
    vertices::{VertexLighting, interpolate_normal},
};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

// Bound scratch storage independently of vertex count: many ranges may
// legally overlap even a single vertex.
const MAX_SKIN_INFLUENCES: usize = 1_048_576;

struct SkinningWeights {
    changes: Vec<(usize, usize, i64)>,
    next_change: usize,
    weights: BTreeMap<usize, u64>,
    ranked: BTreeSet<(Reverse<u64>, usize)>,
    selected: Vec<(usize, f32)>,
    maximum: usize,
}

impl SkinningWeights {
    fn new(influences: &[SkinInfluence], maximum: usize) -> Result<Self, EmuError> {
        if influences.len() > MAX_SKIN_INFLUENCES {
            return Err(geometry_limit());
        }
        if maximum == 0 {
            return Err(geometry_error(
                "invalid-skinning",
                "skin transform limit must be positive",
            ));
        }
        let mut changes = Vec::with_capacity(influences.len() * 2);
        for influence in influences {
            let end = influence
                .first_vertex
                .checked_add(influence.vertex_count)
                .filter(|end| u16::try_from(*end).is_ok())
                .ok_or_else(|| {
                    geometry_error("invalid-skinning", "skin vertex range exceeds 65535")
                })?;
            if influence.vertex_count == 0 || influence.weight == 0 {
                return Err(geometry_error(
                    "invalid-skinning",
                    "skin range and weight must be positive",
                ));
            }
            changes.push((
                influence.first_vertex,
                influence.bone,
                i64::from(influence.weight),
            ));
            changes.push((end, influence.bone, -i64::from(influence.weight)));
        }
        changes.sort_unstable();
        Ok(Self {
            changes,
            next_change: 0,
            weights: BTreeMap::new(),
            ranked: BTreeSet::new(),
            selected: Vec::new(),
            maximum,
        })
    }

    /// Called in ascending vertex order; range boundaries are consumed once.
    fn at(&mut self, vertex: usize) -> &[(usize, f32)] {
        let before = self.next_change;
        while let Some(&(index, bone, change)) = self.changes.get(self.next_change) {
            if index > vertex {
                break;
            }
            let previous = self.weights.get(&bone).copied().unwrap_or(0);
            if previous != 0 {
                self.ranked.remove(&(Reverse(previous), bone));
            }
            // The range count and u32 weights bound every sum well below i64::MAX.
            let weight = (previous as i64 + change) as u64;
            if weight == 0 {
                self.weights.remove(&bone);
            } else {
                self.weights.insert(bone, weight);
                self.ranked.insert((Reverse(weight), bone));
            }
            self.next_change += 1;
        }
        if before != self.next_change {
            let total = self
                .ranked
                .iter()
                .take(self.maximum)
                .map(|(Reverse(weight), _)| weight)
                .sum::<u64>();
            self.selected.clear();
            self.selected.extend(
                self.ranked
                    .iter()
                    .take(self.maximum)
                    .map(|(Reverse(weight), bone)| (*bone, *weight as f32 / total as f32)),
            );
            // Equal weights select the earliest bone binding. Deformation also
            // uses a stable bone order when ranking changes across a range.
            self.selected.sort_unstable_by_key(|(bone, _)| *bone);
        }
        &self.selected
    }
}

fn deform(
    local: Vec4,
    weights: &[(usize, f32)],
    mut transform: impl FnMut(usize) -> Result<Mat4, EmuError>,
) -> Result<Vec4, EmuError> {
    if weights.is_empty() {
        return Ok(local);
    }
    let mut value = [0.0_f32; 4];
    for &(bone, weight) in weights {
        let transformed = transform(bone)?.transform(local);
        for (value, component) in
            value
                .iter_mut()
                .zip([transformed.x, transformed.y, transformed.z, transformed.w])
        {
            *value = weight.mul_add(component, *value);
        }
    }
    Ok(Vec4::new(value[0], value[1], value[2], value[3]))
}

impl VertexBufferState {
    /// Deforms only the hit triangle's normals before interpolation in mesh space.
    pub fn interpolated_skinned_normal(
        &self,
        bone_transforms: &[Mat4],
        influences: &[SkinInfluence],
        max_transforms_per_vertex: usize,
        indices: [usize; 3],
        barycentric: [f32; 3],
    ) -> Result<Vec3, EmuError> {
        let Some(normals) = &self.normals else {
            return Ok(Vec3::new(0.0, 0.0, 1.0));
        };
        SkinInfluence::validate_all(influences, bone_transforms.len(), self.position_count()?)?;
        let mut weights = SkinningWeights::new(influences, max_transforms_per_vertex)?;
        let mut order: [(usize, usize); 3] = std::array::from_fn(|slot| (slot, indices[slot]));
        order.sort_unstable_by_key(|(_, vertex)| *vertex);
        let mut deformed = [Vec4::default(); 3];
        for (slot, vertex) in order {
            deformed[slot] = deform(normals.normal(vertex)?, weights.at(vertex), |bone| {
                bone_transforms[bone].inverted().map(Mat4::transposed)
            })?;
        }
        interpolate_normal(deformed, barycentric)
    }

    /// Produces weighted skinned positions using normalized integer influences.
    pub fn transformed_skinned_vertices(
        &self,
        bone_transforms: &[Mat4],
        influences: &[SkinInfluence],
        max_transforms_per_vertex: usize,
        transform: Mat4,
    ) -> Result<Vec<Vertex>, EmuError> {
        let count = self.position_count()?;
        SkinInfluence::validate_all(influences, bone_transforms.len(), count)?;
        let mut weights = SkinningWeights::new(influences, max_transforms_per_vertex)?;
        let mut output = Vec::with_capacity(count);
        for index in 0..count {
            let mut vertex = self.vertex(index)?;
            vertex.position =
                transform.transform(deform(vertex.position, weights.at(index), |bone| {
                    Ok(bone_transforms[bone])
                })?);
            output.push(vertex);
        }
        Ok(output)
    }

    /// Produces camera-space skinned vertices with normals deformed by the same
    /// bones before applying the fixed-function lighting model.
    #[allow(clippy::too_many_arguments)]
    pub fn transformed_lit_skinned_vertices(
        &self,
        bone_transforms: &[Mat4],
        influences: &[SkinInfluence],
        max_transforms_per_vertex: usize,
        model: Mat4,
        view_transform: Mat4,
        camera_position: Vec3,
        material: MaterialState,
        vertex_color_tracking: bool,
        lights: &[LightSource],
        alpha: f32,
    ) -> Result<Vec<Vertex>, EmuError> {
        let count = self.position_count()?;
        let normals = self.normals.as_ref().ok_or_else(|| {
            geometry_error(
                "missing-normals",
                "lit skinned VertexBuffer has no normal array",
            )
        })?;
        SkinInfluence::validate_all(influences, bone_transforms.len(), count)?;
        let mut weights = SkinningWeights::new(influences, max_transforms_per_vertex)?;
        let lighting = VertexLighting::new(
            model,
            view_transform,
            camera_position,
            material,
            vertex_color_tracking,
            lights,
            alpha,
        )?;
        let mut bone_normals = lighting
            .transforms_normals()
            .then(|| vec![None; bone_transforms.len()]);
        let mut output = Vec::with_capacity(count);
        for index in 0..count {
            let mut vertex = self.vertex(index)?;
            let local_normal = normals.normal(index)?;
            let selected = weights.at(index);
            vertex.position = deform(vertex.position, selected, |bone| Ok(bone_transforms[bone]))?;
            let normal = if let Some(bones) = bone_normals.as_mut() {
                deform(local_normal, selected, |bone| {
                    if let Some(transform) = bones[bone] {
                        return Ok(transform);
                    }
                    let transform = bone_transforms[bone].inverted()?.transposed();
                    bones[bone] = Some(transform);
                    Ok(transform)
                })?
            } else {
                local_normal
            };
            output.push(lighting.apply(vertex, normal)?);
        }
        Ok(output)
    }
}

/// One bounded bone influence range.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkinInfluence {
    /// Bone index.
    pub bone: usize,
    /// First affected vertex.
    pub first_vertex: usize,
    /// Number of affected vertices.
    pub vertex_count: usize,
    /// Positive integer weight.
    pub weight: u32,
}

impl SkinInfluence {
    /// Returns this bone's selected, normalized weights, independent of the
    /// current `VertexBuffer` length. Rendering and picking use the same selection.
    pub fn bone_vertex_weights(
        influences: &[Self],
        bone: usize,
        max_transforms_per_vertex: usize,
    ) -> Result<impl Iterator<Item = (usize, f32)> + use<>, EmuError> {
        let mut weights = SkinningWeights::new(influences, max_transforms_per_vertex)?;
        let first = weights.changes.first().map_or(0, |change| change.0);
        let last = weights.changes.last().map_or(0, |change| change.0);
        Ok((first..last).filter_map(move |vertex| {
            weights
                .at(vertex)
                .iter()
                .find(|(candidate, _)| *candidate == bone)
                .map(|(_, weight)| (vertex, *weight))
        }))
    }

    /// Validates a set of skin influences before any deformation.
    pub fn validate_all(
        influences: &[Self],
        bones: usize,
        vertices: usize,
    ) -> Result<(), EmuError> {
        if influences.len() > MAX_SKIN_INFLUENCES {
            return Err(geometry_limit());
        }
        if influences.iter().any(|influence| {
            influence.bone >= bones
                || influence.vertex_count == 0
                || influence.weight == 0
                || influence
                    .first_vertex
                    .checked_add(influence.vertex_count)
                    .is_none_or(|end| end > vertices || u16::try_from(end).is_err())
        }) {
            return Err(geometry_error(
                "invalid-object-graph",
                "bone range or weight is invalid",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/geometry/skinning.rs"]
mod tests;
