//! Typed M3G object state and iterative scene-graph operations.

use crate::{
    AnimationControllerState, Arena, ArenaLimits, FogState, Handle, Image2DState,
    KeyframeSequenceState, Mat4, MaterialState, Quaternion, SkinInfluence, TriangleStripArrayState,
    Vec3, VertexArrayState, VertexBufferState,
};
use diagnostics::{Category, EmuError};
use std::collections::{BTreeMap, BTreeSet};

mod animation;
mod arena;
mod camera;
mod checkpoint;

pub use camera::CameraProjection;
mod hierarchy;
mod lifecycle;
mod nodes;
mod object_state;
mod transforms;
mod vertex_buffers;
mod world;

/// Common state inherited from `Object3D`.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ObjectState {
    user_id: i32,
    user_object: Option<u64>,
    animation_tracks: Vec<Handle>,
}

impl ObjectState {
    fn allocated_bytes(&self) -> usize {
        self.animation_tracks
            .capacity()
            .saturating_mul(std::mem::size_of::<Handle>())
    }
}

/// Common state inherited from `Transformable`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TransformableState {
    translation: Vec3,
    scale: Vec3,
    orientation: Quaternion,
    transform: Mat4,
}

impl Default for TransformableState {
    fn default() -> Self {
        Self {
            translation: Vec3::default(),
            scale: Vec3::new(1.0, 1.0, 1.0),
            orientation: Quaternion::IDENTITY,
            transform: Mat4::IDENTITY,
        }
    }
}

impl TransformableState {
    fn composite(&self) -> Result<Mat4, EmuError> {
        Ok(
            Mat4::from_components(self.translation, self.orientation, self.scale)?
                .multiplied(self.transform),
        )
    }
}

/// Common state inherited from `Node`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NodeState {
    transformable: TransformableState,
    parent: Option<Handle>,
    rendering_enabled: bool,
    picking_enabled: bool,
    scope: u32,
    alpha: f32,
    // Z then Y.  A null reference is meaningful when the target is not
    // `NONE`: it selects the run-time reference passed to `Node.align`.
    alignment: [(Option<Handle>, i32); 2],
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            transformable: TransformableState::default(),
            parent: None,
            rendering_enabled: true,
            picking_enabled: true,
            scope: u32::MAX,
            alpha: 1.0,
            alignment: [(None, 144), (None, 144)],
        }
    }
}

/// Light parameters independent from the renderer.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LightState {
    /// JSR light mode constant.
    pub mode: i32,
    /// ARGB/RGB light color.
    pub color: u32,
    /// Scalar intensity.
    pub intensity: f32,
    /// Constant, linear and quadratic attenuation.
    pub attenuation: [f32; 3],
    /// Spot cone angle in degrees.
    pub spot_angle: f32,
    /// Spot exponent.
    pub spot_exponent: f32,
}

impl Default for LightState {
    fn default() -> Self {
        Self {
            mode: 129,
            color: 0x00ff_ffff,
            intensity: 1.0,
            attenuation: [1.0, 0.0, 0.0],
            spot_angle: 45.0,
            spot_exponent: 0.0,
        }
    }
}

/// Background clear/image state independent from target binding.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BackgroundState {
    /// ARGB clear color.
    pub color: u32,
    /// Optional background `Image2D` reference.
    pub image: Option<Handle>,
    /// Crop rectangle `(x, y, width, height)`.
    pub crop: [i32; 4],
    /// Horizontal image mode.
    pub mode_x: i32,
    /// Vertical image mode.
    pub mode_y: i32,
    /// Whether color clear is enabled.
    pub color_clear: bool,
    /// Whether depth clear is enabled.
    pub depth_clear: bool,
}

impl BackgroundState {
    /// Checks a crop rectangle; origins may be signed, dimensions must not be.
    pub fn validate_crop(crop: [i32; 4]) -> Result<(), EmuError> {
        if crop[2] < 0 || crop[3] < 0 {
            return Err(graph_error(
                "invalid-crop",
                "Background crop dimensions must be non-negative",
            ));
        }
        Ok(())
    }
}

/// Appearance references and render layer.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AppearanceState {
    /// Render layer in the JSR signed-byte range.
    pub layer: i32,
    /// Fog, polygon, compositing and material objects.
    pub fog: Option<Handle>,
    pub polygon_mode: Option<Handle>,
    pub compositing_mode: Option<Handle>,
    pub material: Option<Handle>,
    /// Texture slots available in the software backend.
    pub textures: [Option<Handle>; crate::MAX_TEXTURE_UNITS],
}

/// Fixed-function compositing state.
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CompositingModeState {
    pub blending: i32,
    pub alpha_threshold: f32,
    pub alpha_write: bool,
    pub color_write: bool,
    pub depth_write: bool,
    pub depth_test: bool,
    pub depth_offset_factor: f32,
    pub depth_offset_units: f32,
}

impl Default for CompositingModeState {
    fn default() -> Self {
        Self {
            // REPLACE keeps the default appearance in the opaque render pass.
            blending: 68,
            alpha_threshold: 0.0,
            alpha_write: true,
            color_write: true,
            depth_write: true,
            depth_test: true,
            depth_offset_factor: 0.0,
            depth_offset_units: 0.0,
        }
    }
}

/// Face orientation, shading and lighting flags.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct PolygonModeState {
    pub culling: i32,
    pub winding: i32,
    pub shading: i32,
    pub two_sided_lighting: bool,
    pub local_camera_lighting: bool,
    pub perspective_correction: bool,
}

/// Material colors plus the Object3D-level tracking switch.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaterialObjectState {
    pub material: MaterialState,
    pub vertex_color_tracking: bool,
}

impl Default for PolygonModeState {
    fn default() -> Self {
        Self {
            culling: 160,
            winding: 168,
            shading: 165,
            two_sided_lighting: false,
            local_camera_lighting: false,
            perspective_correction: true,
        }
    }
}

/// Texture object state in addition to inherited transformable state.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TextureObjectState {
    pub transformable: TransformableState,
    pub image: Handle,
    pub level_filter: i32,
    pub image_filter: i32,
    pub wrap_s: i32,
    pub wrap_t: i32,
    pub blending: i32,
    pub blend_color: u32,
}

/// Mesh geometry shared by plain, morphing and skinned meshes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MeshState {
    pub node: NodeState,
    pub vertices: Handle,
    pub submeshes: Vec<Handle>,
    pub appearances: Vec<Option<Handle>>,
}

/// Billboard sprite state.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SpriteState {
    pub node: NodeState,
    pub scaled: bool,
    pub image: Handle,
    pub appearance: Option<Handle>,
    pub crop: [i32; 4],
}

impl SpriteState {
    /// Initial crop after construction or image replacement, bounded by the profile.
    #[must_use]
    pub fn image_crop(image: &Image2DState, maximum: u32) -> [i32; 4] {
        let maximum = maximum.min(i32::MAX as u32);
        [
            0,
            0,
            image.width().min(maximum) as i32,
            image.height().min(maximum) as i32,
        ]
    }

    /// Signed dimensions mirror the sprite; their magnitudes are profile bounded.
    pub fn validate_crop(crop: [i32; 4], maximum: u32) -> Result<(), EmuError> {
        if crop[2].unsigned_abs() > maximum || crop[3].unsigned_abs() > maximum {
            return Err(graph_error(
                "invalid-crop",
                "Sprite3D crop dimensions exceed the profile limit",
            ));
        }
        Ok(())
    }
}

/// Last successful group-picking result.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RayIntersectionState {
    pub intersected: Option<Handle>,
    pub ray: [f32; 6],
    pub distance: f32,
    pub submesh: i32,
    pub texture: [[f32; 2]; 2],
    pub normal: Vec3,
}

impl Default for RayIntersectionState {
    fn default() -> Self {
        Self {
            intersected: None,
            ray: [0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            distance: 0.0,
            submesh: 0,
            texture: [[0.0; 2]; 2],
            normal: Vec3::new(0.0, 0.0, 1.0),
        }
    }
}

impl Default for BackgroundState {
    fn default() -> Self {
        Self {
            color: 0,
            image: None,
            crop: [0; 4],
            mode_x: 32,
            mode_y: 32,
            color_clear: true,
            depth_clear: true,
        }
    }
}

/// Native type and state attached to one guest M3G object.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ObjectKind {
    /// Standalone matrix object (`Transform` is not an `Object3D`).
    Transform(Mat4),
    /// Direct `Object3D` state used by non-node classes not yet specialized.
    Object,
    /// Direct `Transformable` state.
    Transformable(TransformableState),
    /// Abstract/concrete leaf node state.
    Node(NodeState),
    /// Group node with ordered children.
    Group {
        /// Common node state.
        node: NodeState,
        /// Children in insertion order.
        children: Vec<Handle>,
    },
    /// World root with ordered children and active state.
    World {
        /// Common node state.
        node: NodeState,
        /// Children in insertion order.
        children: Vec<Handle>,
        /// Active camera.
        camera: Option<Handle>,
        /// Optional background.
        background: Option<Handle>,
    },
    /// Camera node.
    Camera {
        /// Common node state.
        node: NodeState,
        /// Projection state.
        projection: CameraProjection,
    },
    /// Light node.
    Light {
        /// Common node state.
        node: NodeState,
        /// Light parameters.
        light: LightState,
    },
    /// Background state.
    Background(BackgroundState),
    /// Mutable vertex component storage.
    VertexArray(VertexArrayState),
    /// Aggregated vertex attributes.
    VertexBuffer {
        /// Decoded vertex attributes used by the renderer.
        state: VertexBufferState,
        /// Guest-visible source arrays: positions, normals, colors and two texture units.
        arrays: [Option<Handle>; 5],
    },
    /// Triangle strip index storage.
    TriangleStripArray(TriangleStripArrayState),
    /// Decoded two-dimensional image.
    Image2D(Image2DState),
    /// Render state aggregator.
    Appearance(AppearanceState),
    /// Compositing state.
    CompositingMode(CompositingModeState),
    /// Polygon state.
    PolygonMode(PolygonModeState),
    /// Fog state.
    Fog(FogState),
    /// Material state.
    Material(MaterialObjectState),
    /// Texture object with inherited transform.
    Texture2D(TextureObjectState),
    /// Plain retained mesh.
    Mesh(MeshState),
    /// Morphing mesh and weights.
    MorphingMesh {
        mesh: MeshState,
        targets: Vec<Handle>,
        weights: Vec<f32>,
    },
    /// Skinned mesh, skeleton and bone ranges.
    SkinnedMesh {
        mesh: MeshState,
        skeleton: Handle,
        bones: Vec<Handle>,
        bind_transforms: Vec<Mat4>,
        influences: Vec<SkinInfluence>,
    },
    /// Sprite node.
    Sprite3D(SpriteState),
    /// Animation controller.
    AnimationController(AnimationControllerState),
    /// Keyframe storage.
    KeyframeSequence(KeyframeSequenceState),
    /// Animation track references and target property.
    AnimationTrack {
        sequence: Handle,
        controller: Option<Handle>,
        property: i32,
    },
    /// Mutable picking result container.
    RayIntersection(RayIntersectionState),
}

impl ObjectKind {
    fn node(&self) -> Option<&NodeState> {
        match self {
            Self::Node(node)
            | Self::Group { node, .. }
            | Self::World { node, .. }
            | Self::Camera { node, .. }
            | Self::Light { node, .. } => Some(node),
            Self::Mesh(mesh) | Self::MorphingMesh { mesh, .. } | Self::SkinnedMesh { mesh, .. } => {
                Some(&mesh.node)
            }
            Self::Sprite3D(sprite) => Some(&sprite.node),
            Self::Transform(_)
            | Self::Object
            | Self::Transformable(_)
            | Self::Background(_)
            | Self::VertexArray(_)
            | Self::VertexBuffer { .. }
            | Self::TriangleStripArray(_)
            | Self::Image2D(_)
            | Self::Appearance(_)
            | Self::CompositingMode(_)
            | Self::PolygonMode(_)
            | Self::Fog(_)
            | Self::Material(_)
            | Self::Texture2D(_)
            | Self::AnimationController(_)
            | Self::KeyframeSequence(_)
            | Self::AnimationTrack { .. }
            | Self::RayIntersection(_) => None,
        }
    }

    fn node_mut(&mut self) -> Option<&mut NodeState> {
        match self {
            Self::Node(node)
            | Self::Group { node, .. }
            | Self::World { node, .. }
            | Self::Camera { node, .. }
            | Self::Light { node, .. } => Some(node),
            Self::Mesh(mesh) | Self::MorphingMesh { mesh, .. } | Self::SkinnedMesh { mesh, .. } => {
                Some(&mut mesh.node)
            }
            Self::Sprite3D(sprite) => Some(&mut sprite.node),
            Self::Transform(_)
            | Self::Object
            | Self::Transformable(_)
            | Self::Background(_)
            | Self::VertexArray(_)
            | Self::VertexBuffer { .. }
            | Self::TriangleStripArray(_)
            | Self::Image2D(_)
            | Self::Appearance(_)
            | Self::CompositingMode(_)
            | Self::PolygonMode(_)
            | Self::Fog(_)
            | Self::Material(_)
            | Self::Texture2D(_)
            | Self::AnimationController(_)
            | Self::KeyframeSequence(_)
            | Self::AnimationTrack { .. }
            | Self::RayIntersection(_) => None,
        }
    }

    fn transformable(&self) -> Option<&TransformableState> {
        match self {
            Self::Transformable(state) => Some(state),
            Self::Texture2D(state) => Some(&state.transformable),
            _ => self.node().map(|node| &node.transformable),
        }
    }

    fn transformable_mut(&mut self) -> Option<&mut TransformableState> {
        match self {
            Self::Transformable(state) => Some(state),
            Self::Texture2D(state) => Some(&mut state.transformable),
            _ => self.node_mut().map(|node| &mut node.transformable),
        }
    }

    fn children(&self) -> Option<&[Handle]> {
        match self {
            Self::Group { children, .. } | Self::World { children, .. } => Some(children),
            _ => None,
        }
    }

    fn children_mut(&mut self) -> Option<&mut Vec<Handle>> {
        match self {
            Self::Group { children, .. } | Self::World { children, .. } => Some(children),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Object {
    guest_reference: Option<u64>,
    base: ObjectState,
    kind: ObjectKind,
}

/// Suite-owned M3G object runtime.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Runtime {
    objects: Arena<Object>,
    guest_handles: BTreeMap<u64, Handle>,
    max_graph_depth: usize,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(ArenaLimits::default())
    }
}

fn estimated_bytes(kind: &ObjectKind) -> usize {
    let child_bytes = match kind {
        ObjectKind::Group { children, .. } | ObjectKind::World { children, .. } => children
            .capacity()
            .saturating_mul(std::mem::size_of::<Handle>()),
        ObjectKind::Mesh(mesh) => estimated_mesh_bytes(mesh),
        ObjectKind::MorphingMesh {
            mesh,
            targets,
            weights,
        } => estimated_mesh_bytes(mesh)
            .saturating_add(
                targets
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Handle>()),
            )
            .saturating_add(
                weights
                    .capacity()
                    .saturating_mul(std::mem::size_of::<f32>()),
            ),
        ObjectKind::SkinnedMesh {
            mesh,
            bones,
            bind_transforms,
            influences,
            ..
        } => estimated_mesh_bytes(mesh)
            .saturating_add(
                bones
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Handle>()),
            )
            .saturating_add(
                bind_transforms
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Mat4>()),
            )
            .saturating_add(
                influences
                    .capacity()
                    .saturating_mul(std::mem::size_of::<SkinInfluence>()),
            ),
        ObjectKind::VertexArray(state) => state.allocated_bytes(),
        ObjectKind::VertexBuffer { state, .. } => state.allocated_bytes(),
        ObjectKind::TriangleStripArray(state) => state.allocated_bytes(),
        ObjectKind::Image2D(state) => state.allocated_bytes(),
        ObjectKind::KeyframeSequence(state) => state.allocated_bytes(),
        _ => 0,
    };
    std::mem::size_of::<Object>().saturating_add(child_bytes)
}

fn estimated_object_bytes(object: &Object) -> usize {
    estimated_bytes(&object.kind).saturating_add(object.base.allocated_bytes())
}

fn estimated_mesh_bytes(mesh: &MeshState) -> usize {
    mesh.submeshes
        .capacity()
        .saturating_mul(std::mem::size_of::<Handle>())
        .saturating_add(
            mesh.appearances
                .capacity()
                .saturating_mul(std::mem::size_of::<Option<Handle>>()),
        )
}

fn clone_slice_preserving_capacity<T: Clone>(source: &[T], capacity: usize) -> Vec<T> {
    let mut clone = Vec::with_capacity(capacity);
    clone.extend_from_slice(source);
    clone
}

fn skin_vector_bytes(
    bones_capacity: usize,
    bind_transforms_capacity: usize,
    influences_capacity: usize,
) -> usize {
    bones_capacity
        .saturating_mul(std::mem::size_of::<Handle>())
        .saturating_add(bind_transforms_capacity.saturating_mul(std::mem::size_of::<Mat4>()))
        .saturating_add(influences_capacity.saturating_mul(std::mem::size_of::<SkinInfluence>()))
}

fn mesh_references(mesh: &MeshState, references: &mut Vec<Handle>) {
    references.push(mesh.vertices);
    references.extend(&mesh.submeshes);
    references.extend(mesh.appearances.iter().flatten());
}

fn graph_error(code: &'static str, message: &'static str) -> EmuError {
    EmuError::new(Category::M3g, code, message)
}

fn shortest_alignment_rotation(from: Vec3, to: Vec3) -> Result<Quaternion, EmuError> {
    let cosine = from.dot(to).clamp(-1.0, 1.0);
    if cosine >= 1.0 - 1.0e-6 {
        return Ok(Quaternion::IDENTITY);
    }
    let axis = from.cross(to);
    if cosine <= -1.0 + 1.0e-6 {
        // The specification permits either path for opposite axes, provided
        // the choice is deterministic.  Pick a stable perpendicular axis.
        let fallback = if from.x.abs() < 0.5 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        return Quaternion::from_axis_angle(180.0, from.cross(fallback));
    }
    Quaternion::from_axis_angle(cosine.acos().to_degrees(), axis)
}

#[cfg(test)]
#[path = "../../../tests/unit/m3g/scene/mod.rs"]
mod tests;
