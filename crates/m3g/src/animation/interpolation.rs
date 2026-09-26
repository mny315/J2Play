//! Component and quaternion interpolation for a selected keyframe segment.

use crate::Quaternion;
use diagnostics::EmuError;

pub(super) fn interpolate_components(first: &[f32], second: &[f32], amount: f32) -> Vec<f32> {
    let amount = f64::from(amount);
    first
        .iter()
        .zip(second)
        .map(|(left, right)| {
            let left = f64::from(*left);
            amount.mul_add(f64::from(*right) - left, left) as f32
        })
        .collect()
}

pub(super) fn catmull_rom(
    before: &[f32],
    first: &[f32],
    second: &[f32],
    after: &[f32],
    tangent_scales: [f32; 2],
    amount: f32,
) -> Vec<f32> {
    // Finite keys can have differences outside f32 even when the sample fits.
    let amount = f64::from(amount);
    let squared = amount * amount;
    let cubed = squared * amount;
    let first_weight = 2.0 * cubed - 3.0 * squared + 1.0;
    let second_weight = -2.0 * cubed + 3.0 * squared;
    let start_weight = (cubed - 2.0 * squared + amount) * f64::from(tangent_scales[0]);
    let end_weight = (cubed - squared) * f64::from(tangent_scales[1]);
    (0..first.len())
        .map(|index| {
            let first = f64::from(first[index]);
            let second = f64::from(second[index]);
            (first_weight * first
                + second_weight * second
                + start_weight * (second - f64::from(before[index]))
                + end_weight * (f64::from(after[index]) - first)) as f32
        })
        .collect()
}

pub(super) fn quaternion(value: &[f32]) -> Quaternion {
    Quaternion {
        x: value[0],
        y: value[1],
        z: value[2],
        w: value[3],
    }
}

pub(super) fn quaternion_slerp(
    first: &[f32],
    second: &[f32],
    amount: f32,
) -> Result<Vec<f32>, EmuError> {
    let result = quaternion(first).slerp(quaternion(second), amount)?;
    Ok(vec![result.x, result.y, result.z, result.w])
}

pub(super) fn quaternion_squad_controls(
    before: &[f32],
    first: &[f32],
    second: &[f32],
    after: &[f32],
    tangent_scales: [f32; 2],
) -> Result<[Quaternion; 2], EmuError> {
    let before = quaternion(before).normalized()?;
    let first = quaternion(first).normalized()?;
    let second = quaternion(second).normalized()?;
    let after = quaternion(after).normalized()?;
    let previous_log = quaternion_log_delta(before, first);
    let segment_log = quaternion_log_delta(first, second);
    let next_log = quaternion_log_delta(second, after);
    let start_exponent = std::array::from_fn(|index| {
        ((previous_log[index] + segment_log[index]) * f64::from(tangent_scales[0])
            - segment_log[index])
            * 0.5
    });
    let end_exponent = std::array::from_fn(|index| {
        (segment_log[index] - (segment_log[index] + next_log[index]) * f64::from(tangent_scales[1]))
            * 0.5
    });
    let start_control = first.multiplied(quaternion_exp(start_exponent))?;
    let end_control = second.multiplied(quaternion_exp(end_exponent))?;
    Ok([start_control, end_control])
}

pub(super) fn quaternion_squad(
    first: &[f32],
    second: &[f32],
    [start_control, end_control]: [Quaternion; 2],
    amount: f32,
) -> Result<Vec<f32>, EmuError> {
    let arc = quaternion(first).slerp(quaternion(second), amount)?;
    let controls = start_control.slerp(end_control, amount)?;
    let result = arc.slerp(controls, 2.0 * amount * (1.0 - amount))?;
    Ok(vec![result.x, result.y, result.z, result.w])
}

fn quaternion_log_delta(first: Quaternion, second: Quaternion) -> [f64; 3] {
    let [ax, ay, az, aw] = [first.x, first.y, first.z, first.w].map(f64::from);
    let [bx, by, bz, bw] = [second.x, second.y, second.z, second.w].map(f64::from);
    // log(inverse(first) * second); atan2 preserves long arcs and tiny angles.
    let vector = [
        aw * bx - ax * bw - ay * bz + az * by,
        aw * by + ax * bz - ay * bw - az * bx,
        aw * bz - ax * by + ay * bx - az * bw,
    ];
    let scalar = aw * bw + ax * bx + ay * by + az * bz;
    let length = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
    if length > 0.0 {
        let scale = length.atan2(scalar) / length;
        vector.map(|value| value * scale)
    } else if scalar < 0.0 {
        // The axis is unspecified for diametrically opposed quaternions.
        [std::f64::consts::PI, 0.0, 0.0]
    } else {
        [0.0; 3]
    }
}

fn quaternion_exp(value: [f64; 3]) -> Quaternion {
    let angle = value.iter().map(|value| value * value).sum::<f64>().sqrt();
    if angle <= 0.0 {
        return Quaternion::IDENTITY;
    }
    let (sine, cosine) = angle.sin_cos();
    let [x, y, z] = value.map(|component| (component * (sine / angle)) as f32);
    Quaternion {
        x,
        y,
        z,
        w: cosine as f32,
    }
}
