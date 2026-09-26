//! Scene geometry snapshots shared by retained rendering and picking.

use super::{EmuError, GeometryWork, Machine, vm_error};

pub(super) enum MeshGeometry {
    Plain(m3g::VertexBufferState),
    Morphed(m3g::MorphedVertices),
    Skinned {
        vertices: m3g::VertexBufferState,
        bones: Vec<m3g::Mat4>,
        influences: Vec<m3g::SkinInfluence>,
        maximum: usize,
    },
}

pub(super) struct MeshRenderSource {
    pub node: m3g::Handle,
    pub model: m3g::Mat4,
    pub scope: u32,
    pub alpha: f32,
}

pub(super) struct PreparedMesh {
    pub source: usize,
    pub geometry: MeshGeometry,
    pub unlit: Option<Vec<m3g::Vertex>>,
    pub lit: Option<(m3g::MaterialObjectState, Vec<m3g::Vertex>)>,
}

pub(super) fn mesh_state(kind: &m3g::ObjectKind) -> Option<&m3g::MeshState> {
    match kind {
        m3g::ObjectKind::Mesh(mesh)
        | m3g::ObjectKind::MorphingMesh { mesh, .. }
        | m3g::ObjectKind::SkinnedMesh { mesh, .. } => Some(mesh),
        _ => None,
    }
}

impl MeshGeometry {
    pub(super) fn interpolated_normal(
        &self,
        indices: [usize; 3],
        weights: [f32; 3],
        work: &mut GeometryWork,
    ) -> Result<m3g::Vec3, EmuError> {
        if self.has_normals() {
            work.vertices(3)?;
        }
        match self {
            Self::Plain(vertices) => vertices.interpolated_normal(indices, weights),
            Self::Morphed(vertices) => vertices.interpolated_normal(indices, weights),
            Self::Skinned {
                vertices,
                bones,
                influences,
                maximum,
            } => {
                vertices.interpolated_skinned_normal(bones, influences, *maximum, indices, weights)
            }
        }
    }

    fn vertex_count(&self) -> usize {
        match self {
            Self::Plain(vertices) | Self::Skinned { vertices, .. } => vertices.vertex_count(),
            Self::Morphed(vertices) => vertices.vertex_count(),
        }
    }
    pub(super) fn has_normals(&self) -> bool {
        match self {
            Self::Plain(vertices) | Self::Skinned { vertices, .. } => vertices.has_normals(),
            Self::Morphed(vertices) => vertices.has_normals(),
        }
    }

    pub(super) fn transformed_vertices(
        &self,
        transform: m3g::Mat4,
        work: &mut GeometryWork,
    ) -> Result<Vec<m3g::Vertex>, EmuError> {
        work.vertices(self.vertex_count())?;
        match self {
            Self::Plain(vertices) => vertices.transformed_vertices(transform),
            Self::Morphed(vertices) => Ok(vertices.transformed_vertices(transform)),
            Self::Skinned {
                vertices,
                bones,
                influences,
                maximum,
            } => vertices.transformed_skinned_vertices(bones, influences, *maximum, transform),
        }
    }

    pub(super) fn transformed_lit_vertices(
        &self,
        model: m3g::Mat4,
        view_transform: m3g::Mat4,
        camera_position: m3g::Vec3,
        material: m3g::MaterialObjectState,
        lights: &[m3g::LightSource],
        alpha: f32,
        work: &mut GeometryWork,
    ) -> Result<Vec<m3g::Vertex>, EmuError> {
        work.vertices(self.vertex_count())?;
        match self {
            Self::Plain(vertices) => vertices.transformed_lit_vertices(
                model,
                view_transform,
                camera_position,
                material.material,
                material.vertex_color_tracking,
                lights,
                alpha,
            ),
            Self::Morphed(vertices) => vertices.transformed_lit_vertices(
                model,
                view_transform,
                camera_position,
                material.material,
                material.vertex_color_tracking,
                lights,
                alpha,
            ),
            Self::Skinned {
                vertices,
                bones,
                influences,
                maximum,
            } => vertices.transformed_lit_skinned_vertices(
                bones,
                influences,
                *maximum,
                model,
                view_transform,
                camera_position,
                material.material,
                material.vertex_color_tracking,
                lights,
                alpha,
            ),
        }
    }
}

impl Machine<'_, '_> {
    pub(super) fn m3g_retained_geometry(
        &self,
        node: m3g::Handle,
        work: &mut GeometryWork,
    ) -> Result<Option<(&m3g::MeshState, MeshGeometry)>, EmuError> {
        self.m3g_check_geometry_cancellation()?;
        let kind = self.m3g.runtime.kind(node)?;
        let Some(mesh) =
            mesh_state(kind).filter(|mesh| mesh.appearances.iter().any(Option::is_some))
        else {
            return Ok(None);
        };
        let vertices = self.m3g.runtime.resolved_vertex_buffer(mesh.vertices)?;
        let geometry = match kind {
            m3g::ObjectKind::MorphingMesh {
                targets, weights, ..
            } => {
                work.vertices(vertices.vertex_count())?;
                let targets = targets
                    .iter()
                    .map(|target| {
                        self.m3g_check_geometry_cancellation()?;
                        self.m3g.runtime.resolved_vertex_buffer(*target)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                MeshGeometry::Morphed(vertices.morphed_vertices(&targets, weights)?)
            }
            m3g::ObjectKind::SkinnedMesh {
                bones,
                bind_transforms,
                influences,
                ..
            } => {
                if bones.len() != bind_transforms.len() {
                    return Err(vm_error(
                        "illegal-state-exception",
                        "SkinnedMesh bone and bind-pose tables differ in length",
                    ));
                }
                let bones = bones
                    .iter()
                    .zip(bind_transforms)
                    .map(|(bone, bind)| {
                        self.m3g_check_geometry_cancellation()?;
                        Ok(self
                            .m3g
                            .runtime
                            .transform_to(*bone, node)?
                            .multiplied(*bind))
                    })
                    .collect::<Result<Vec<_>, EmuError>>()?;
                MeshGeometry::Skinned {
                    vertices,
                    bones,
                    influences: influences.clone(),
                    maximum: self.limits.m3g_max_transforms_per_vertex as usize,
                }
            }
            _ => MeshGeometry::Plain(vertices),
        };
        Ok(Some((mesh, geometry)))
    }
}
