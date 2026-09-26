//! Host-independent JSR-184 1.1 object model, loader and software renderer.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::similar_names,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
#![cfg_attr(test, allow(clippy::float_cmp))]

mod animation;
mod api;
mod appearance;
mod arena;
mod geometry;
mod instantiate;
mod loader;
mod math;
mod renderer;
mod scene;

/// Maximum texture-unit capacity of the current software backend.
pub const MAX_TEXTURE_UNITS: usize = 2;
/// Maximum simultaneous-light capacity of the current software backend.
pub const MAX_LIGHTS: usize = 8;

pub use animation::{
    AnimationControllerState, Interpolation, KeyframeSequenceState, RepeatMode, blend_samples,
};
pub use api::{api_counts, bootstrap_classes, register_natives};
pub use appearance::{
    BlendFunction, FogMode, FogState, Image2DState, ImageFormat, LightSource, MaterialState,
    Texture2DState, WrapMode, shade_lit_vertex,
};
pub use arena::{Arena, ArenaLimits, Handle};
pub use geometry::{
    MorphedVertices, Ray, RayHit, SkinInfluence, TriangleStripArrayState, VertexArrayState,
    VertexBufferState, VertexComponent,
};
pub use instantiate::{InstantiatedFile, instantiate_file};
pub use loader::{
    FileHeader, LoaderLimits, M3gFile, ObjectType, ParsedObject, Section,
    parse_section_object_stream,
};
pub use math::{Mat4, Quaternion, Vec3, Vec4};
pub use renderer::{CullMode, FrameBlend, RenderLimits, RenderStats, SoftwareRenderer, Vertex};
pub use scene::{
    AppearanceState, BackgroundState, CameraProjection, CompositingModeState, LightState,
    MaterialObjectState, MeshState, NodeState, ObjectKind, ObjectState, PolygonModeState,
    RayIntersectionState, Runtime, SpriteState, TextureObjectState, TransformableState,
};
