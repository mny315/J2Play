//! Reusable rays and scale-independent triangle intersections.

use super::{EmuError, Vec3, geometry_error};

/// A validated ray, retaining the application's original direction and units.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    origin: [f64; 3],
    direction: [f64; 3],
}

/// Ray/triangle intersection result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// Non-negative parameter in `origin + distance * direction`.
    pub distance: f32,
    /// Barycentric U coordinate.
    pub u: f32,
    /// Barycentric V coordinate.
    pub v: f32,
    /// Whether a counterclockwise triangle faces the incoming ray.
    pub front_facing: bool,
}

impl Ray {
    /// Validates finite coordinates and a nonzero direction once for a scene query.
    pub fn new(origin: Vec3, direction: Vec3) -> Result<Self, EmuError> {
        let origin = coordinates(origin)?;
        let direction = coordinates(direction)?;
        if direction.iter().all(|component| *component == 0.0) {
            return Err(geometry_error("zero-length", "pick ray direction is zero"));
        }
        Ok(Self { origin, direction })
    }

    /// Intersects a triangle without renormalizing the ray or its face normal.
    pub fn intersect_triangle(&self, triangle: [Vec3; 3]) -> Result<Option<RayHit>, EmuError> {
        let [first, second, third] = triangle;
        let first = coordinates(first)?;
        let edge1 = subtract(coordinates(second)?, first);
        let edge2 = subtract(coordinates(third)?, first);
        let perpendicular = cross(self.direction, edge2);
        let determinant = dot(edge1, perpendicular);
        // A fixed epsilon rejects small valid triangles. Wide intermediates
        // preserve products across the full finite f32 input range, so only
        // a zero determinant identifies parallel or degenerate geometry.
        if determinant == 0.0 {
            return Ok(None);
        }
        let inverse = determinant.recip();
        let offset = subtract(self.origin, first);
        let u = dot(offset, perpendicular) * inverse;
        if !(0.0..=1.0).contains(&u) {
            return Ok(None);
        }
        let cross = cross(offset, edge1);
        let v = dot(self.direction, cross) * inverse;
        if v < 0.0 || u + v > 1.0 {
            return Ok(None);
        }
        let distance = dot(edge2, cross) * inverse;
        Ok((distance >= 0.0).then_some(RayHit {
            distance: distance as f32,
            u: u as f32,
            v: v as f32,
            front_facing: determinant > 0.0,
        }))
    }
}

fn coordinates(value: Vec3) -> Result<[f64; 3], EmuError> {
    let values = [value.x, value.y, value.z];
    if !values.iter().all(|value| value.is_finite()) {
        return Err(geometry_error(
            "non-finite",
            "pick coordinates must be finite",
        ));
    }
    Ok(values.map(f64::from))
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|index| left[index] - right[index])
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}
