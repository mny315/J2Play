//! Deterministic keyframe sampling and weighted animation control.

use diagnostics::{Category, EmuError};

mod controller;
mod interpolation;
mod keyframes;

pub use controller::AnimationControllerState;
pub use keyframes::{Interpolation, KeyframeSequenceState, RepeatMode};

/// Validates and blends compatible sampled values without partial output mutation.
pub fn blend_samples<T: AsRef<[f32]>>(samples: &[(T, f32)]) -> Result<Vec<f32>, EmuError> {
    let Some((first, _)) = samples.first() else {
        return Ok(Vec::new());
    };
    let components = first.as_ref().len();
    for (values, weight) in samples {
        let values = values.as_ref();
        if values.len() != components
            || !values.iter().all(|value| value.is_finite())
            || !weight.is_finite()
            || *weight < 0.0
        {
            return Err(animation_error(
                "invalid-blend",
                "animation samples have incompatible or non-finite values",
            ));
        }
    }
    // Controller weights are absolute contributions, not a weighted average.
    // Only an orientation target normalizes its final blended quaternion.
    // Opposite contributions may cancel after exceeding the f32 range.
    Ok((0..components)
        .map(|component| {
            samples.iter().fold(0.0, |sum, (values, weight)| {
                f64::from(*weight).mul_add(f64::from(values.as_ref()[component]), sum)
            }) as f32
        })
        .collect())
}

fn animation_error(code: &'static str, message: &'static str) -> EmuError {
    EmuError::new(Category::M3g, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/m3g/animation/mod.rs"]
mod tests;
