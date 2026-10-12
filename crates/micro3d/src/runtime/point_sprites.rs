//! Validated point-sprite batches and billboard projection.

use super::rendering::{normalized_uv, runtime_error};
use super::{AffineTrans, EmuError, PreparedProjection, TextureData, Vec4, Vector3D, Vertex};
use crate::{cos, math::TRIG_ONE, sin};

/// Validates the complete batch before rendering. A shared block is checked
/// once and repeated by the iterator; per-sprite blocks are consumed in order.
pub(super) fn point_sprite_parameters(
    command: i32,
    primitive_count: usize,
    parameters: &[i32],
) -> Result<impl Iterator<Item = [i32; 8]> + '_, EmuError> {
    let required = match command & 0x3000 {
        0x1000 => 8,
        0x2000 | 0x3000 => primitive_count.checked_mul(8).ok_or_else(|| {
            runtime_error(
                "point-sprite-overflow",
                "point sprite parameter count overflow",
            )
        })?,
        _ => {
            return Err(runtime_error(
                "point-sprite-parameters",
                "point sprites require a parameter layout",
            ));
        }
    };
    let parameters = parameters.get(..required).ok_or_else(|| {
        runtime_error(
            "point-sprite-parameters",
            "point sprite parameter array is too short",
        )
    })?;
    let (blocks, _) = parameters.as_chunks::<8>();
    for values in blocks {
        if values[0] < 0 || values[1] < 0 {
            return Err(runtime_error(
                "point-sprite-size",
                "point sprite dimensions must not be negative",
            ));
        }
        if values[7] & !0x3 != 0 {
            return Err(runtime_error(
                "point-sprite-flags",
                "point sprite flags contain unsupported bits",
            ));
        }
    }
    Ok(blocks.iter().copied().cycle().take(primitive_count))
}

pub(super) fn point_sprite_vertices(
    source: Vector3D,
    parameters: [i32; 8],
    projection: &PreparedProjection,
    affine: AffineTrans,
    texture: Option<&TextureData>,
    target_width: u32,
    target_height: u32,
) -> Result<[Vertex; 4], EmuError> {
    let transformed = affine.transform(source);
    let center = projection.project(transformed);
    let size = point_sprite_screen_size(
        [parameters[0], parameters[1]],
        parameters[7],
        transformed,
        center,
        projection,
        [target_width, target_height],
    )?;
    let cosine = f64::from(cos(parameters[2])) / f64::from(TRIG_ONE);
    let sine = f64::from(sin(parameters[2])) / f64::from(TRIG_ONE);
    let half_width = size[0] * 0.5;
    let half_height = size[1] * 0.5;
    let corners = [
        [-half_width, -half_height],
        [half_width, -half_height],
        [half_width, half_height],
        [-half_width, half_height],
    ];
    let uv_pixels = [
        [parameters[3], parameters[4]],
        [parameters[5], parameters[4]],
        [parameters[5], parameters[6]],
        [parameters[3], parameters[6]],
    ];
    let mut vertices = [Vertex::default(); 4];
    for (index, ([corner_x, corner_y], uv)) in corners.into_iter().zip(uv_pixels).enumerate() {
        let rotated_x = corner_x.mul_add(cosine, -(corner_y * sine));
        let rotated_y = corner_x.mul_add(sine, corner_y * cosine);
        let clip_x = 2.0 * rotated_x / f64::from(target_width.max(1)) * f64::from(center.w);
        let clip_y = -2.0 * rotated_y / f64::from(target_height.max(1)) * f64::from(center.w);
        let (s, t) = normalized_uv([uv[0].cast_unsigned(), uv[1].cast_unsigned()], texture);
        vertices[index] = Vertex::textured(
            Vec4::new(
                (f64::from(center.x) + clip_x) as f32,
                (f64::from(center.y) + clip_y) as f32,
                center.z,
                center.w,
            ),
            0xffff_ffff,
            s,
            t,
        );
    }
    Ok(vertices)
}

fn point_sprite_screen_size(
    [width, height]: [i32; 2],
    flags: i32,
    center: Vector3D,
    center_position: Vec4,
    projection: &PreparedProjection,
    [target_width, target_height]: [u32; 2],
) -> Result<[f64; 2], EmuError> {
    // Local sizes project at the center depth; pixel sizes bypass projection.
    if flags & 0x1 != 0 || flags & 0x2 != 0 {
        return Ok([f64::from(width), f64::from(height)]);
    }
    let horizontal = projection.project(Vector3D::new(
        center.x.saturating_add(width),
        center.y,
        center.z,
    ));
    let vertical = projection.project(Vector3D::new(
        center.x,
        center.y.saturating_add(height),
        center.z,
    ));
    let center_screen = projected_screen_position(center_position, target_width, target_height)?;
    let horizontal_screen = projected_screen_position(horizontal, target_width, target_height)?;
    let vertical_screen = projected_screen_position(vertical, target_width, target_height)?;
    Ok([
        (horizontal_screen[0] - center_screen[0]).abs(),
        (vertical_screen[1] - center_screen[1]).abs(),
    ])
}

pub(super) fn projected_screen_position(
    position: Vec4,
    target_width: u32,
    target_height: u32,
) -> Result<[f64; 2], EmuError> {
    if ![position.x, position.y, position.w]
        .into_iter()
        .all(f32::is_finite)
        || position.w.abs() <= f32::EPSILON
    {
        return Err(runtime_error(
            "point-sprite-projection",
            "point sprite has an invalid projected center",
        ));
    }
    let inverse_w = f64::from(position.w).recip();
    Ok([
        (f64::from(position.x) * inverse_w + 1.0) * 0.5 * f64::from(target_width.max(1)),
        (1.0 - f64::from(position.y) * inverse_w) * 0.5 * f64::from(target_height.max(1)),
    ])
}
