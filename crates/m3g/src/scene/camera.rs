//! Camera parameter contracts and projection math shared by loading and execution.

use super::{EmuError, Mat4, graph_error};

/// Camera projection mode and parameters.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CameraProjection {
    /// Generic projection transform.
    Generic(Mat4),
    /// Parallel projection `(height, aspect, near, far)`.
    Parallel {
        /// Vertical extent.
        height: f32,
        /// Width divided by height.
        aspect_ratio: f32,
        /// Near plane.
        near: f32,
        /// Far plane.
        far: f32,
    },
    /// Perspective projection `(field of view, aspect, near, far)`.
    Perspective {
        /// Vertical field of view in degrees.
        field_of_view: f32,
        /// Width divided by height.
        aspect_ratio: f32,
        /// Near plane.
        near: f32,
        /// Far plane.
        far: f32,
    },
}

impl Default for CameraProjection {
    fn default() -> Self {
        Self::Generic(Mat4::IDENTITY)
    }
}

impl CameraProjection {
    /// JSR-184 GENERIC, PARALLEL or PERSPECTIVE constant.
    #[must_use]
    pub const fn projection_type(self) -> i32 {
        match self {
            Self::Generic(_) => 48,
            Self::Parallel { .. } => 49,
            Self::Perspective { .. } => 50,
        }
    }

    /// Parameters in setter order; a generic matrix has no parameter array.
    #[must_use]
    pub const fn parameters(self) -> Option<[f32; 4]> {
        match self {
            Self::Generic(_) => None,
            Self::Parallel {
                height,
                aspect_ratio,
                near,
                far,
            } => Some([height, aspect_ratio, near, far]),
            Self::Perspective {
                field_of_view,
                aspect_ratio,
                near,
                far,
            } => Some([field_of_view, aspect_ratio, near, far]),
        }
    }

    /// Checks the common API/file contract; clip planes may be reversed or equal.
    pub fn validate(self) -> Result<(), EmuError> {
        let Some(values) = self.parameters() else {
            return Ok(());
        };
        if !values.iter().all(|value| value.is_finite())
            || values[0] <= 0.0
            || values[1] <= 0.0
            || matches!(self, Self::Perspective { .. })
                && (values[0] >= 180.0 || values[2] <= 0.0 || values[3] <= 0.0)
        {
            Err(graph_error(
                "invalid-projection",
                "invalid Camera projection parameters",
            ))
        } else {
            Ok(())
        }
    }

    /// Equal near/far distances define an empty view volume, not an invalid setter.
    #[must_use]
    #[allow(clippy::float_cmp)]
    pub const fn has_empty_volume(self) -> bool {
        match self {
            Self::Generic(_) => false,
            Self::Parallel { near, far, .. } | Self::Perspective { near, far, .. } => near == far,
        }
    }

    /// Builds the rendering projection, mapping an empty volume to zero clip W.
    /// Camera.getProjection(Transform) must report an arithmetic error for that
    /// special case instead of exposing this rendering-only zero matrix.
    pub fn render_matrix(self) -> Result<Mat4, EmuError> {
        self.validate()?;
        if self.has_empty_volume() {
            return Mat4::from_array([0.0; 16]);
        }
        let matrix = match self {
            Self::Generic(matrix) => return Ok(matrix),
            Self::Parallel {
                height,
                aspect_ratio,
                near,
                far,
            } => {
                let height = f64::from(height);
                let width = height * f64::from(aspect_ratio);
                let near = f64::from(near);
                let far = f64::from(far);
                let depth = far - near;
                [
                    2.0 / width,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    2.0 / height,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    -2.0 / depth,
                    0.0,
                    0.0,
                    0.0,
                    -(far + near) / depth,
                    1.0,
                ]
            }
            Self::Perspective {
                field_of_view,
                aspect_ratio,
                near,
                far,
            } => {
                let scale = (f64::from(field_of_view).to_radians() * 0.5).tan().recip();
                let near = f64::from(near);
                let far = f64::from(far);
                let depth = far - near;
                [
                    scale / f64::from(aspect_ratio),
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    scale,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    -(far + near) / depth,
                    -1.0,
                    0.0,
                    0.0,
                    -2.0 * far * near / depth,
                    0.0,
                ]
            }
        };
        // Float inputs may overflow intermediate width/depth sums or products
        // even when every final matrix element is representable as a float.
        Mat4::from_array(matrix.map(|value| value as f32))
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/scene/camera.rs"]
mod tests;
