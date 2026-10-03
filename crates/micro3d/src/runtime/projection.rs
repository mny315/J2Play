//! Projection constants shared by all vertices in one render call.

use super::{EmuError, FigureLayoutState, Projection, Vec4, Vector3D, cos, runtime_error, sin};

pub(super) struct PreparedProjection {
    center: [f64; 2],
    mode: ProjectionMode,
}

enum ProjectionMode {
    ParallelScale { scale: [i32; 2], target: [f64; 2] },
    Parallel { size: [f64; 2] },
    Perspective { scale: [f64; 2], depth: [f64; 2] },
}

impl PreparedProjection {
    pub(super) fn new(
        layout: &FigureLayoutState,
        center_x: i32,
        center_y: i32,
        target_width: u32,
        target_height: u32,
    ) -> Result<Self, EmuError> {
        let target_width = f64::from(target_width.max(1));
        let target_height = f64::from(target_height.max(1));
        let center = [
            f64::from(center_x) / target_width * 2.0 - 1.0,
            1.0 - f64::from(center_y) / target_height * 2.0,
        ];
        let mode = match layout.projection {
            Projection::ParallelScale => ProjectionMode::ParallelScale {
                scale: layout.scale,
                target: [target_width, target_height],
            },
            Projection::Parallel { width, height } => {
                if width <= 0 || height <= 0 {
                    return Err(runtime_error(
                        "projection",
                        "parallel projection dimensions must be positive",
                    ));
                }
                ProjectionMode::Parallel {
                    size: [f64::from(width), f64::from(height)],
                }
            }
            Projection::PerspectiveFov { near, far, angle } => {
                validate_perspective(near, far)?;
                let half_angle = (angle.clamp(1, 2047) + 1) / 2;
                let sine = f64::from(sin(half_angle).max(1));
                let cosine = f64::from(cos(half_angle).max(1));
                let focal = target_width * cosine / (2.0 * sine);
                ProjectionMode::Perspective {
                    scale: [2.0 * focal / target_width, 2.0 * focal / target_height],
                    depth: perspective_depth(near, far),
                }
            }
            Projection::PerspectiveSize {
                near,
                far,
                width,
                height,
            } => {
                validate_perspective(near, far)?;
                if width <= 0 || height <= 0 {
                    return Err(runtime_error(
                        "projection",
                        "perspective dimensions must be positive",
                    ));
                }
                ProjectionMode::Perspective {
                    scale: [
                        8192.0 * f64::from(near) / f64::from(width),
                        8192.0 * f64::from(near) / f64::from(height),
                    ],
                    depth: perspective_depth(near, far),
                }
            }
        };
        Ok(Self { center, mode })
    }

    pub(super) fn project(&self, vector: Vector3D) -> Vec4 {
        let [center_x, center_y] = self.center;
        match self.mode {
            ProjectionMode::ParallelScale { scale, target } => {
                let screen_x = i64::from(vector.x) * i64::from(scale[0]) / 4096;
                let screen_y = i64::from(vector.y) * i64::from(scale[1]) / 4096;
                Vec4::new(
                    (center_x + 2.0 * screen_x as f64 / target[0]) as f32,
                    (center_y - 2.0 * screen_y as f64 / target[1]) as f32,
                    parallel_depth(vector.z),
                    1.0,
                )
            }
            ProjectionMode::Parallel { size } => {
                // setParallelSize uses integer coordinates; setPerspective uses 4.12.
                let ndc_x = f64::from(vector.x) * 2.0 / size[0];
                let ndc_y = f64::from(vector.y) * 2.0 / size[1];
                Vec4::new(
                    (center_x + ndc_x) as f32,
                    (center_y - ndc_y) as f32,
                    parallel_depth(vector.z),
                    1.0,
                )
            }
            ProjectionMode::Perspective { scale, depth } => {
                let z = f64::from(vector.z);
                Vec4::new(
                    (center_x * z + f64::from(vector.x) * scale[0]) as f32,
                    (center_y * z - f64::from(vector.y) * scale[1]) as f32,
                    (depth[0] * z + depth[1]) as f32,
                    z as f32,
                )
            }
        }
    }
}

pub(super) fn parallel_depth(z: i32) -> f32 {
    // Keep parallel layers ordered near the far plane so they do not occlude
    // perspective geometry in the same Graphics3D batch.
    let z = f64::from(z);
    let normalized = z / (z.abs() + 4096.0);
    (0.995 + normalized * 0.005) as f32
}

fn perspective_depth(near: i32, far: i32) -> [f64; 2] {
    let near = f64::from(near);
    let far = f64::from(far);
    [
        (far + near) / (far - near),
        -2.0 * far * near / (far - near),
    ]
}

fn validate_perspective(near: i32, far: i32) -> Result<(), EmuError> {
    if near <= 0 || far <= near {
        return Err(runtime_error(
            "projection",
            "perspective near/far planes are invalid",
        ));
    }
    Ok(())
}
