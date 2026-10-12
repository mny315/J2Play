//! Material state and fixed-function vertex lighting.

use super::appearance_error;
use crate::Vec3;
use diagnostics::EmuError;

/// Fixed-function material colors and shininess.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaterialState {
    /// Ambient RGB.
    pub ambient: u32,
    /// Diffuse ARGB.
    pub diffuse: u32,
    /// Emissive RGB.
    pub emissive: u32,
    /// Specular RGB.
    pub specular: u32,
    /// Specular exponent in `[0, 128]`.
    pub shininess: f32,
}

impl Default for MaterialState {
    fn default() -> Self {
        Self {
            ambient: 0x0033_3333,
            diffuse: 0xffcc_cccc,
            emissive: 0,
            specular: 0,
            shininess: 0.0,
        }
    }
}

/// One bounded fixed-function light in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightSource {
    /// Scene-wide ambient contribution.
    Ambient { color: u32, intensity: f32 },
    /// Direction from the shaded point toward an infinitely distant light.
    Directional {
        direction: Vec3,
        color: u32,
        intensity: f32,
    },
    /// Positional light with constant/linear/quadratic attenuation.
    Omni {
        position: Vec3,
        color: u32,
        intensity: f32,
        attenuation: [f32; 3],
    },
    /// Positional cone light; `direction` points away from the light.
    Spot {
        position: Vec3,
        direction: Vec3,
        color: u32,
        intensity: f32,
        attenuation: [f32; 3],
        angle_degrees: f32,
        exponent: f32,
    },
}

/// Shades one world-space vertex using the complete JSR-184 light model.
/// Mesh rendering prepares the same constants once for the whole vertex batch.
pub fn shade_lit_vertex(
    material: MaterialState,
    vertex_color: u32,
    vertex_color_tracking: bool,
    normal: Vec3,
    position: Vec3,
    view: Vec3,
    lights: &[LightSource],
) -> Result<u32, EmuError> {
    PreparedLighting::new(material, lights)?.shade(
        vertex_color,
        vertex_color_tracking,
        normal,
        position,
        view,
    )
}

/// Validated constants for one mesh's material and bounded light set.
pub(crate) struct PreparedLighting {
    material: MaterialState,
    ambient: [f64; 3],
    diffuse: [f64; 3],
    emissive: [f64; 3],
    specular: [f64; 3],
    lights: [PreparedLight; crate::MAX_LIGHTS],
    light_count: usize,
    uses_normals: bool,
}

#[derive(Clone, Copy)]
struct PreparedLight {
    color: [f64; 3],
    intensity: f64,
    geometry: LightGeometry,
}

#[derive(Clone, Copy)]
enum LightGeometry {
    Ambient,
    Directional(Vec3),
    Positional {
        position: Vec3,
        attenuation: [f64; 3],
        cone: Option<SpotCone>,
    },
}

#[derive(Clone, Copy)]
struct SpotCone {
    direction: Vec3,
    cutoff: f32,
    exponent: f32,
}

impl PreparedLight {
    const EMPTY: Self = Self {
        color: [0.0; 3],
        intensity: 0.0,
        geometry: LightGeometry::Ambient,
    };

    fn new(source: LightSource) -> Result<Self, EmuError> {
        let (color, intensity, geometry) = match source {
            LightSource::Ambient { color, intensity } => (color, intensity, LightGeometry::Ambient),
            LightSource::Directional {
                direction,
                color,
                intensity,
            } => (
                color,
                intensity,
                LightGeometry::Directional(direction.normalized()?),
            ),
            LightSource::Omni {
                position,
                color,
                intensity,
                attenuation,
            } => (
                color,
                intensity,
                LightGeometry::Positional {
                    position,
                    attenuation: prepare_attenuation(attenuation)?,
                    cone: None,
                },
            ),
            LightSource::Spot {
                position,
                direction,
                color,
                intensity,
                attenuation,
                angle_degrees,
                exponent,
            } => {
                if !angle_degrees.is_finite()
                    || !(0.0..=90.0).contains(&angle_degrees)
                    || !exponent.is_finite()
                    || !(0.0..=128.0).contains(&exponent)
                {
                    return Err(appearance_error(
                        "invalid-lighting",
                        "spot angle or exponent is outside the JSR range",
                    ));
                }
                (
                    color,
                    intensity,
                    LightGeometry::Positional {
                        position,
                        attenuation: prepare_attenuation(attenuation)?,
                        cone: Some(SpotCone {
                            direction: normalized_or_zero(direction)?,
                            cutoff: angle_degrees.to_radians().cos(),
                            exponent,
                        }),
                    },
                )
            }
        };
        if !intensity.is_finite() {
            return Err(appearance_error(
                "invalid-lighting",
                "light intensity must be finite",
            ));
        }
        Ok(Self {
            color: color_rgb(color),
            intensity: f64::from(intensity),
            geometry,
        })
    }
}

impl PreparedLighting {
    pub(crate) fn new(material: MaterialState, lights: &[LightSource]) -> Result<Self, EmuError> {
        if lights.len() > crate::MAX_LIGHTS
            || !material.shininess.is_finite()
            || !(0.0..=128.0).contains(&material.shininess)
        {
            return Err(appearance_error(
                "invalid-lighting",
                "lighting exceeds the backend capacity or material range",
            ));
        }
        let mut prepared = [PreparedLight::EMPTY; crate::MAX_LIGHTS];
        for (destination, source) in prepared.iter_mut().zip(lights) {
            *destination = PreparedLight::new(*source)?;
        }
        Ok(Self {
            material,
            ambient: color_rgb(material.ambient),
            diffuse: color_rgb(material.diffuse),
            emissive: color_rgb(material.emissive),
            specular: color_rgb(material.specular),
            lights: prepared,
            light_count: lights.len(),
            uses_normals: lights
                .iter()
                .any(|light| !matches!(light, LightSource::Ambient { .. })),
        })
    }

    pub(crate) fn shade(
        &self,
        vertex_color: u32,
        vertex_color_tracking: bool,
        normal: Vec3,
        position: Vec3,
        view: Vec3,
    ) -> Result<u32, EmuError> {
        let (ambient, diffuse, alpha) = if vertex_color_tracking {
            let color = color_rgb(vertex_color);
            (color, color, vertex_color & 0xff00_0000)
        } else {
            (
                self.ambient,
                self.diffuse,
                self.material.diffuse & 0xff00_0000,
            )
        };
        // Zero normals and coincident light/view positions contribute no
        // directional lighting; nonzero vectors retain their direction at any scale.
        let normal = if self.uses_normals {
            normalized_or_zero(normal)?
        } else {
            // Ambient and emissive colors do not depend on either direction.
            crate::math::ensure_finite(&[normal.x, normal.y, normal.z])?;
            normal
        };
        let has_specular = self.uses_normals && self.material.specular & 0x00ff_ffff != 0;
        let view = if has_specular {
            normalized_or_zero(view)?
        } else {
            // Diffuse and ambient light do not use the view direction. Keep
            // input validation without a square root and three divisions per vertex.
            crate::math::ensure_finite(&[view.x, view.y, view.z])?;
            view
        };
        let mut rgb = self.emissive;
        for light in &self.lights[..self.light_count] {
            let (direction, amount) = match light.geometry {
                LightGeometry::Ambient => {
                    for component in 0..3 {
                        rgb[component] +=
                            light.color[component] * light.intensity * ambient[component];
                    }
                    continue;
                }
                LightGeometry::Directional(direction) => (direction, light.intensity),
                LightGeometry::Positional {
                    position: light_position,
                    attenuation,
                    cone,
                } => {
                    let (direction, distance) = positional_light_vector(light_position, position)?;
                    let amount = attenuated_intensity(light.intensity, attenuation, distance)?;
                    let amount = if let Some(cone) = cone {
                        let from_light = Vec3::new(-direction.x, -direction.y, -direction.z);
                        let cosine = cone.direction.dot(from_light);
                        let spot = if cosine >= cone.cutoff {
                            cosine.max(0.0).powf(cone.exponent)
                        } else {
                            0.0
                        };
                        amount * f64::from(spot)
                    } else {
                        amount
                    };
                    (direction, amount)
                }
            };
            let lambert = normal.dot(direction).max(0.0);
            let highlight = if !has_specular || lambert == 0.0 {
                0.0
            } else {
                let half_sum = Vec3::new(
                    direction.x + view.x,
                    direction.y + view.y,
                    direction.z + view.z,
                );
                if half_sum == Vec3::default() {
                    0.0
                } else {
                    normal
                        .dot(half_sum.normalized()?)
                        .max(0.0)
                        .powf(self.material.shininess)
                }
            };
            for component in 0..3 {
                rgb[component] += light.color[component]
                    * amount
                    * (diffuse[component] * f64::from(lambert)
                        + self.specular[component] * f64::from(highlight));
            }
        }
        Ok(
            alpha
                | quantize_unit(rgb[0]) << 16
                | quantize_unit(rgb[1]) << 8
                | quantize_unit(rgb[2]),
        )
    }
}

fn prepare_attenuation(coefficients: [f32; 3]) -> Result<[f64; 3], EmuError> {
    if !coefficients
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0)
    {
        return Err(appearance_error(
            "invalid-lighting",
            "light attenuation is invalid",
        ));
    }
    Ok(coefficients.map(f64::from))
}

fn normalized_or_zero(value: Vec3) -> Result<Vec3, EmuError> {
    if value == Vec3::default() {
        Ok(Vec3::default())
    } else {
        value.normalized()
    }
}

fn positional_light_vector(light: Vec3, vertex: Vec3) -> Result<(Vec3, f64), EmuError> {
    // Subtraction and squaring must not overflow or underflow for finite f32
    // coordinates. The same distance also normalizes the lighting direction.
    let x = f64::from(light.x) - f64::from(vertex.x);
    let y = f64::from(light.y) - f64::from(vertex.y);
    let z = f64::from(light.z) - f64::from(vertex.z);
    let distance = x.mul_add(x, y.mul_add(y, z * z)).sqrt();
    if !distance.is_finite() {
        return Err(appearance_error(
            "invalid-lighting",
            "light position is invalid",
        ));
    }
    let direction = if distance == 0.0 {
        Vec3::default()
    } else {
        Vec3::new(
            (x / distance) as f32,
            (y / distance) as f32,
            (z / distance) as f32,
        )
    };
    Ok((direction, distance))
}

fn attenuated_intensity(
    intensity: f64,
    coefficients: [f64; 3],
    distance: f64,
) -> Result<f64, EmuError> {
    let [constant, linear, quadratic] = coefficients;
    let denominator = quadratic.mul_add(distance * distance, linear.mul_add(distance, constant));
    if denominator == 0.0 {
        return Err(appearance_error(
            "invalid-lighting",
            "light attenuation denominator is zero",
        ));
    }
    Ok(intensity / denominator)
}

fn color_rgb(color: u32) -> [f64; 3] {
    [
        f64::from((color >> 16) & 0xff) / 255.0,
        f64::from((color >> 8) & 0xff) / 255.0,
        f64::from(color & 0xff) / 255.0,
    ]
}

fn quantize_unit(value: f64) -> u32 {
    crate::math::rounded_u32(value.clamp(0.0, 1.0) * 255.0, 255)
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/appearance/lighting.rs"]
mod tests;
