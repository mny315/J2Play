//! Homogeneous clipping and attribute interpolation for lines and triangles.

use crate::Vec4;
use diagnostics::EmuError;

use super::{Vertex, unpack_color};

pub(super) const MAX_CLIPPED_VERTICES: usize = 12;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ClipVertex {
    pub(super) position: Vec4,
    pub(super) color: [f64; 4],
    pub(super) texture: [[f64; 3]; 2],
    pub(super) eye_z: f64,
}

impl ClipVertex {
    pub(super) fn from_vertex(source: Vertex) -> Result<Self, EmuError> {
        source.validate_position()?;
        Ok(Self {
            position: source.position,
            color: unpack_color(source.color),
            texture: source.texture.map(|coordinates| coordinates.map(f64::from)),
            eye_z: f64::from(source.eye_z),
        })
    }
}

pub(super) fn clip_triangle(
    first: &mut [ClipVertex; MAX_CLIPPED_VERTICES],
    second: &mut [ClipVertex; MAX_CLIPPED_VERTICES],
) -> (usize, bool) {
    // A fully visible triangle needs neither intersections nor six rounds of
    // copying its interpolated attributes between the clipping buffers.
    let outside = [0, 1, 2].map(|index| clip_outcode(first[index].position));
    if outside[0] | outside[1] | outside[2] == 0 {
        return (3, false);
    }
    // A triangle wholly outside any one plane cannot enter the clip volume.
    if outside[0] & outside[1] & outside[2] != 0 {
        return (0, true);
    }
    let mut count = 3_usize;
    let mut clipped = false;
    let (mut input, mut output) = (first, second);
    let mut result_in_first = true;
    let mut distances = [0.0; MAX_CLIPPED_VERTICES];
    for plane in 0..6 {
        let mut outside_count = 0;
        for (index, vertex) in input[..count].iter().enumerate() {
            let distance = plane_distance(vertex.position, plane);
            distances[index] = distance;
            outside_count += usize::from(distance < 0.0);
        }
        // An inactive plane leaves every attribute unchanged. Keep the current
        // buffer instead of copying all vertices just to alternate buffers.
        if outside_count == 0 {
            continue;
        }
        clipped = true;
        if outside_count == count {
            return (0, true);
        }
        let mut output_count = 0_usize;
        let mut previous = input[count - 1];
        let mut previous_distance = distances[count - 1];
        for (&current, &current_distance) in input[..count].iter().zip(&distances) {
            let previous_inside = previous_distance >= 0.0;
            let current_inside = current_distance >= 0.0;
            if previous_inside != current_inside {
                let amount = previous_distance / (previous_distance - current_distance);
                output[output_count] = interpolate(previous, current, amount);
                output_count += 1;
            }
            if current_inside {
                output[output_count] = current;
                output_count += 1;
            }
            previous = current;
            previous_distance = current_distance;
        }
        count = output_count;
        std::mem::swap(&mut input, &mut output);
        result_in_first = !result_in_first;
    }
    if !result_in_first {
        output[..count].copy_from_slice(&input[..count]);
    }
    (count, clipped)
}

fn clip_outcode(p: Vec4) -> u8 {
    u8::from(p.x < -p.w)
        | u8::from(p.x > p.w) << 1
        | u8::from(p.y < -p.w) << 2
        | u8::from(p.y > p.w) << 3
        | u8::from(p.z < -p.w) << 4
        | u8::from(p.z > p.w) << 5
}

pub(super) fn clip_line(line: &mut [ClipVertex; 2]) -> (bool, bool) {
    let mut clipped = false;
    for plane in 0..6 {
        let first_distance = plane_distance(line[0].position, plane);
        let second_distance = plane_distance(line[1].position, plane);
        let first_inside = first_distance >= 0.0;
        let second_inside = second_distance >= 0.0;
        if !first_inside && !second_inside {
            return (false, true);
        }
        if first_inside != second_inside {
            let amount = first_distance / (first_distance - second_distance);
            let intersection = interpolate(line[0], line[1], amount);
            if first_inside {
                line[1] = intersection;
            } else {
                line[0] = intersection;
            }
            clipped = true;
        }
    }
    (true, clipped)
}

fn plane_distance(position: Vec4, plane: usize) -> f64 {
    let w = f64::from(position.w);
    match plane {
        0 => f64::from(position.x) + w,
        1 => w - f64::from(position.x),
        2 => f64::from(position.y) + w,
        3 => w - f64::from(position.y),
        4 => f64::from(position.z) + w,
        _ => w - f64::from(position.z),
    }
}

fn interpolate(first: ClipVertex, second: ClipVertex, amount: f64) -> ClipVertex {
    // Finite endpoints can have a difference larger than f32::MAX. Keep the
    // clipping ratio and interpolation wide until the bounded result is stored.
    let coordinate = |first, second| {
        let first = f64::from(first);
        amount.mul_add(f64::from(second) - first, first) as f32
    };
    ClipVertex {
        position: Vec4::new(
            coordinate(first.position.x, second.position.x),
            coordinate(first.position.y, second.position.y),
            coordinate(first.position.z, second.position.z),
            coordinate(first.position.w, second.position.w),
        ),
        color: std::array::from_fn(|index| {
            amount.mul_add(second.color[index] - first.color[index], first.color[index])
        }),
        texture: std::array::from_fn(|unit| {
            std::array::from_fn(|index| {
                amount.mul_add(
                    second.texture[unit][index] - first.texture[unit][index],
                    first.texture[unit][index],
                )
            })
        }),
        eye_z: amount.mul_add(second.eye_z - first.eye_z, first.eye_z),
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/renderer/clipping.rs"]
mod tests;
