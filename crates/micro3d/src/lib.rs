//! Host-independent `MascotCapsule Micro3D Version 3` frontend and loaders.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::missing_errors_doc,
    clippy::similar_names,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
#![cfg_attr(test, allow(clippy::float_cmp))]

mod api;
mod loader;
mod math;
mod runtime;

pub use api::{api_counts, bootstrap_classes, register_natives};
pub use loader::{
    ActionData, ActionSegmentData, ActionTableData, Bone, Face, FigureData, LoaderLimits,
    ScalarKeyframe, TextureData, VectorKeyframe,
};
pub use math::{AffineTrans, Vector3D, cos, sin, sqrt};
pub use runtime::{
    EffectState, FigureLayoutState, FigureState, LightState, Micro3dMetrics, ObjectKind,
    PrimitiveData, PrimitiveEnvironment, PrimitivePayloadCounts, Projection, Runtime,
    primitive_payload_counts,
};
