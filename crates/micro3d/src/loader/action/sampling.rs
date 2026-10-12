//! Fixed-point pose interpolation and appearance-mask sampling.

use super::{ActionData, ActionSegmentData, AffineTrans, ScalarKeyframe, Vector3D, VectorKeyframe};

impl ActionSegmentData {
    /// Samples one per-bone pose from the API's signed 16.16 frame value.
    #[must_use]
    pub fn sample_transform(&self, frame: i32) -> AffineTrans {
        match self {
            Self::Affine(value) => *value,
            Self::Components {
                translation,
                scale,
                rotation,
                roll,
            } => {
                let translation = sample_vector(translation, frame, Vector3D::default());
                let scale = sample_vector(scale, frame, Vector3D::new(4096, 4096, 4096));
                let rotation = sample_vector(rotation, frame, Vector3D::new(0, 0, 4096));
                let roll = sample_scalar(roll, frame, 0);
                let mut transform = direction_matrix(rotation);
                let sine = i64::from(crate::math::sin(roll));
                let cosine = i64::from(crate::math::cos(roll));
                // A Z rotation changes only the first two columns. Retain
                // the intermediate 4.12 rounding of T * direction * roll * S.
                for (row, translation) in transform.values.chunks_exact_mut(4).zip([
                    translation.x,
                    translation.y,
                    translation.z,
                ]) {
                    let x = i64::from(row[0]);
                    let y = i64::from(row[1]);
                    row[0] = crate::math::div_round(x * cosine + y * sine, 4096)
                        .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                        as i32;
                    row[1] = crate::math::div_round(y * cosine - x * sine, 4096)
                        .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                        as i32;
                    for (component, scale) in row[..3].iter_mut().zip([scale.x, scale.y, scale.z]) {
                        *component =
                            crate::math::div_round(i64::from(*component) * i64::from(scale), 4096)
                                .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                                as i32;
                    }
                    row[3] = translation;
                }
                transform
            }
        }
    }
}

impl ActionData {
    /// Samples the Figure pattern bit mask at the API's signed 16.16 frame value.
    #[must_use]
    pub fn sample_pattern(&self, frame: i32) -> i32 {
        self.pattern_keys
            .iter()
            .rev()
            .find(|key| fixed_frame(key[0]) <= i64::from(frame))
            .map_or(0, |&[_, low, high]| {
                // Each key replaces the complete file appearance mask. Its bit
                // zero addresses the always-visible common group. Figure's API
                // bit zero addresses group one, so discard the common bit here.
                ((u32::from(low) | (u32::from(high) << 16)) >> 1).cast_signed()
            })
    }
}

fn sample_vector(values: &[VectorKeyframe], frame: i32, fallback: Vector3D) -> Vector3D {
    let Some(first) = values.first() else {
        return fallback;
    };
    if i64::from(frame) <= fixed_frame(first.frame) {
        return first.value;
    }
    for pair in values.windows(2) {
        if i64::from(frame) <= fixed_frame(pair[1].frame) {
            return Vector3D::new(
                interpolate(
                    pair[0].value.x,
                    pair[1].value.x,
                    pair[0].frame,
                    pair[1].frame,
                    frame,
                ),
                interpolate(
                    pair[0].value.y,
                    pair[1].value.y,
                    pair[0].frame,
                    pair[1].frame,
                    frame,
                ),
                interpolate(
                    pair[0].value.z,
                    pair[1].value.z,
                    pair[0].frame,
                    pair[1].frame,
                    frame,
                ),
            );
        }
    }
    values.last().map_or(fallback, |value| value.value)
}

fn sample_scalar(values: &[ScalarKeyframe], frame: i32, fallback: i32) -> i32 {
    let Some(first) = values.first() else {
        return fallback;
    };
    if i64::from(frame) <= fixed_frame(first.frame) {
        return first.value;
    }
    for pair in values.windows(2) {
        if i64::from(frame) <= fixed_frame(pair[1].frame) {
            return interpolate(
                pair[0].value,
                pair[1].value,
                pair[0].frame,
                pair[1].frame,
                frame,
            );
        }
    }
    values.last().map_or(fallback, |value| value.value)
}

fn interpolate(start: i32, end: i32, start_frame: u16, end_frame: u16, frame: i32) -> i32 {
    let duration = i64::from(end_frame.saturating_sub(start_frame)) << 16;
    if duration == 0 {
        return end;
    }
    let elapsed = (i64::from(frame) - fixed_frame(start_frame)).clamp(0, duration);
    let delta = i64::from(end) - i64::from(start);
    let value = i64::from(start) + crate::math::div_round(delta * elapsed, duration);
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn fixed_frame(frame: u16) -> i64 {
    i64::from(frame) << 16
}

fn direction_matrix(mut direction: Vector3D) -> AffineTrans {
    if direction == Vector3D::default() {
        direction.z = 4096;
    }
    direction.unit();
    let x = i64::from(direction.x);
    let y = i64::from(direction.y);
    let z = i64::from(direction.z);
    let denominator = 4096_i64 + z;
    if denominator <= 0 {
        return AffineTrans::rotation_x(2048);
    }
    let quotient = |numerator: i64| {
        let rounded = if numerator >= 0 {
            numerator.saturating_add(denominator / 2) / denominator
        } else {
            numerator.saturating_sub(denominator / 2) / denominator
        };
        rounded.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    };
    AffineTrans::new([
        4096_i32.saturating_sub(quotient(x * x)),
        -quotient(x * y),
        direction.x,
        0,
        -quotient(x * y),
        4096_i32.saturating_sub(quotient(y * y)),
        direction.y,
        0,
        -direction.x,
        -direction.y,
        direction.z,
        0,
    ])
}

#[cfg(test)]
#[path = "../../../../../tests/unit/micro3d/loader/action/sampling.rs"]
mod tests;
