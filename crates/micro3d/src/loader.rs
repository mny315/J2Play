//! Bounded parsers for `Micro3D` Figure, `ActionTable` and Texture resources.

use crate::math::{AffineTrans, Vector3D};
use diagnostics::{Category, EmuError};
use std::sync::Arc;

mod action;
mod figure;
mod reader;
mod texture;

pub use action::*;
pub use figure::{Bone, Face, FigureData};
use reader::{BitReader, Cursor};
pub use texture::*;

const TRAILER_BYTES: usize = 20;

/// Suite-scoped parser and native-allocation limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoaderLimits {
    pub file_bytes: usize,
    pub vertices: usize,
    pub faces: usize,
    pub bones: usize,
    pub actions: usize,
    pub keyframes: usize,
    pub texture_pixels: usize,
    pub decoded_bytes: usize,
}

impl Default for LoaderLimits {
    fn default() -> Self {
        Self {
            file_bytes: 8 * 1024 * 1024,
            vertices: 262_144,
            faces: 262_144,
            bones: 16_384,
            actions: 4_096,
            keyframes: 262_144,
            texture_pixels: 1_048_576,
            decoded_bytes: 8 * 1024 * 1024,
        }
    }
}

fn validate_trailer(bytes: &[u8]) -> Result<(), EmuError> {
    if bytes.len() != TRAILER_BYTES {
        return Err(loader_error(
            "resource-trailer",
            "Micro3D trailer is truncated",
        ));
    }
    // The trailer is an implementation-license marker encrypted with two
    // independent keys. Structural validation rejects all-zero/truncated
    // trailers without logging the untrusted marker payload.
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(loader_error(
            "resource-trailer",
            "Micro3D trailer is invalid",
        ));
    }
    Ok(())
}

fn bounded_file(bytes: &[u8], limits: LoaderLimits) -> Result<(), EmuError> {
    if bytes.is_empty() || bytes.len() > limits.file_bytes {
        return Err(loader_error(
            "resource-budget",
            "Micro3D resource exceeds the file budget",
        ));
    }
    Ok(())
}

fn validate_retained_bytes(
    retained_bytes: usize,
    limits: LoaderLimits,
    resource: &str,
) -> Result<(), EmuError> {
    if retained_bytes > limits.decoded_bytes {
        return Err(loader_error(
            "resource-budget",
            format!("{resource} retained allocation exceeds the decoded byte budget"),
        ));
    }
    Ok(())
}

fn checked_mul(left: usize, right: usize, label: &str) -> Result<usize, EmuError> {
    left.checked_mul(right)
        .ok_or_else(|| loader_error("resource-overflow", format!("{label} size overflow")))
}

fn loader_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Micro3d, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/micro3d/loader/mod.rs"]
mod tests;
