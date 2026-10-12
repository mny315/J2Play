//! Column-major M3G transform math.

use diagnostics::{Category, EmuError};

/// Three-component vector.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
}

impl Vec3 {
    /// Constructs a vector.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Scalar product.
    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x
            .mul_add(other.x, self.y.mul_add(other.y, self.z * other.z))
    }

    /// Vector product.
    #[must_use]
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y.mul_add(other.z, -self.z * other.y),
            self.z.mul_add(other.x, -self.x * other.z),
            self.x.mul_add(other.y, -self.y * other.x),
        )
    }

    /// Returns a unit vector or a categorized error for zero/non-finite input.
    pub fn normalized(self) -> Result<Self, EmuError> {
        let [x, y, z] = normalized_components([self.x, self.y, self.z])?;
        Ok(Self::new(x, y, z))
    }
}

/// Four-component homogeneous vector.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vec4 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
    /// W component.
    pub w: f32,
}

impl Vec4 {
    /// Constructs a vector.
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

/// Rotation quaternion stored as `(x, y, z, w)`.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Quaternion {
    /// X imaginary component.
    pub x: f32,
    /// Y imaginary component.
    pub y: f32,
    /// Z imaginary component.
    pub z: f32,
    /// Real component.
    pub w: f32,
}

impl Quaternion {
    /// Identity rotation.
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Creates a normalized quaternion.
    pub fn normalized(self) -> Result<Self, EmuError> {
        let [x, y, z, w] = normalized_components([self.x, self.y, self.z, self.w])?;
        Ok(Self { x, y, z, w })
    }

    /// Creates a quaternion from an angle in degrees and a rotation axis.
    pub fn from_axis_angle(angle_degrees: f32, axis: Vec3) -> Result<Self, EmuError> {
        ensure_finite(&[angle_degrees, axis.x, axis.y, axis.z])?;
        // JSR-184 leaves the axis undefined for an identity rotation and only
        // rejects a zero axis when the angle is nonzero.
        if angle_degrees == 0.0 {
            return Ok(Self::IDENTITY);
        }
        let axis = axis.normalized()?;
        let half = angle_degrees.to_radians() * 0.5;
        let (sin, cos) = half.sin_cos();
        Ok(Self {
            x: axis.x * sin,
            y: axis.y * sin,
            z: axis.z * sin,
            w: cos,
        })
    }

    /// Converts this rotation to an equivalent canonical axis-angle pair.
    ///
    /// The angle is in `[0, 180]` degrees.  For the identity rotation the
    /// axis is returned as zero, which the JSR-184 contract explicitly allows.
    pub fn to_axis_angle(self) -> Result<(f32, Vec3), EmuError> {
        ensure_finite(&[self.x, self.y, self.z, self.w])?;
        // atan2 retains small rotations even when their cosine rounds to one.
        // The common quaternion length cancels, so normalize only the axis.
        // Wide components also preserve very small or large finite inputs.
        let [x, y, z] = [self.x, self.y, self.z].map(f64::from);
        let length = (x * x + y * y + z * z).sqrt();
        if length == 0.0 {
            self.normalized()?; // Reject the all-zero quaternion.
            return Ok((0.0, Vec3::default()));
        }
        let signed_length = if self.w < 0.0 { -length } else { length };
        Ok((
            (2.0 * length.atan2(f64::from(self.w).abs())).to_degrees() as f32,
            Vec3::new(
                (x / signed_length) as f32,
                (y / signed_length) as f32,
                (z / signed_length) as f32,
            ),
        ))
    }

    /// Spherical interpolation preserving quaternion signs as required by JSR-184.
    pub fn slerp(self, other: Self, amount: f32) -> Result<Self, EmuError> {
        ensure_finite(&[amount])?;
        let this = self.normalized()?;
        let other = other.normalized()?;
        let cosine = this.x.mul_add(
            other.x,
            this.y
                .mul_add(other.y, this.z.mul_add(other.z, this.w * other.w)),
        );
        if cosine < -0.9995 {
            return this.slerp_near_opposite(other, amount);
        }
        if cosine > 0.9995 {
            return Self {
                x: this.x + amount * (other.x - this.x),
                y: this.y + amount * (other.y - this.y),
                z: this.z + amount * (other.z - this.z),
                w: this.w + amount * (other.w - this.w),
            }
            .normalized();
        }
        let angle = cosine.clamp(-1.0, 1.0).acos();
        let denominator = angle.sin();
        let left = ((1.0 - amount) * angle).sin() / denominator;
        let right = (amount * angle).sin() / denominator;
        Self {
            x: left.mul_add(this.x, right * other.x),
            y: left.mul_add(this.y, right * other.y),
            z: left.mul_add(this.z, right * other.z),
            w: left.mul_add(this.w, right * other.w),
        }
        .normalized()
    }

    // Near the opposite quaternion, acos(dot) can round to pi even though
    // the endpoints still define a valid arc. Build its plane in f64 instead
    // of dividing by the nearly zero sin(angle).
    #[allow(clippy::float_cmp)] // Exact endpoints remain defined even for an ambiguous arc.
    fn slerp_near_opposite(self, other: Self, amount: f32) -> Result<Self, EmuError> {
        if amount == 0.0 {
            return Ok(self);
        }
        if amount == 1.0 {
            return Ok(other);
        }
        let left = [self.x, self.y, self.z, self.w].map(f64::from);
        let right = [other.x, other.y, other.z, other.w].map(f64::from);
        let left_squared = left.iter().map(|value| value * value).sum::<f64>();
        let dot = left
            .iter()
            .zip(right)
            .map(|(left, right)| left * right)
            .sum::<f64>();
        let projection = dot / left_squared;
        let mut perpendicular: [f64; 4] =
            std::array::from_fn(|index| (-projection).mul_add(left[index], right[index]));
        let mut perpendicular_length = perpendicular
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        let left_length = left_squared.sqrt();
        let angle = if perpendicular_length == 0.0 {
            // JSR-184 leaves the plane undefined for opposite quaternions.
            // Choose a stable orthogonal axis so intermediate SQUAD arcs also
            // remain finite, without changing the defined endpoint values.
            let axis = (1..4).fold(0, |axis, index| {
                if left[index].abs() < left[axis].abs() {
                    index
                } else {
                    axis
                }
            });
            let projection = left[axis] / left_squared;
            perpendicular =
                std::array::from_fn(|index| f64::from(index == axis) - projection * left[index]);
            perpendicular_length = perpendicular
                .iter()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt();
            std::f64::consts::PI
        } else {
            (perpendicular_length * left_length).atan2(dot)
        };
        let (sine, cosine) = (f64::from(amount) * angle).sin_cos();
        let [x, y, z, w] = std::array::from_fn(|index| {
            (cosine * left[index] / left_length
                + sine * perpendicular[index] / perpendicular_length) as f32
        });
        Self { x, y, z, w }.normalized()
    }

    /// Hamilton product followed by normalization.
    pub fn multiplied(self, right: Self) -> Result<Self, EmuError> {
        let left = self.normalized()?;
        let right = right.normalized()?;
        Self {
            x: left.w * right.x + left.x * right.w + left.y * right.z - left.z * right.y,
            y: left.w * right.y - left.x * right.z + left.y * right.w + left.z * right.x,
            z: left.w * right.z + left.x * right.y - left.y * right.x + left.z * right.w,
            w: left.w * right.w - left.x * right.x - left.y * right.y - left.z * right.z,
        }
        .normalized()
    }

    /// Converts an orthonormal, right-handed rotation basis to a quaternion.
    pub fn from_basis(x: Vec3, y: Vec3, z: Vec3) -> Result<Self, EmuError> {
        ensure_finite(&[x.x, x.y, x.z, y.x, y.y, y.z, z.x, z.y, z.z])?;
        let m00 = x.x;
        let m01 = y.x;
        let m02 = z.x;
        let m10 = x.y;
        let m11 = y.y;
        let m12 = z.y;
        let m20 = x.z;
        let m21 = y.z;
        let m22 = z.z;
        let trace = m00 + m11 + m22;
        let result = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            Self {
                w: 0.25 * s,
                x: (m21 - m12) / s,
                y: (m02 - m20) / s,
                z: (m10 - m01) / s,
            }
        } else if m00 > m11 && m00 > m22 {
            let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
            Self {
                w: (m21 - m12) / s,
                x: 0.25 * s,
                y: (m01 + m10) / s,
                z: (m02 + m20) / s,
            }
        } else if m11 > m22 {
            let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
            Self {
                w: (m02 - m20) / s,
                x: (m01 + m10) / s,
                y: 0.25 * s,
                z: (m12 + m21) / s,
            }
        } else {
            let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
            Self {
                w: (m10 - m01) / s,
                x: (m02 + m20) / s,
                y: (m12 + m21) / s,
                z: 0.25 * s,
            }
        };
        result.normalized()
    }
}

/// A 4×4 matrix stored column-major internally.
///
/// The public JSR-184 `Transform` array contract is row-major; callers at
/// that boundary must use [`Mat4::from_row_major`] and
/// [`Mat4::to_row_major`].
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Mat4 {
    values: [f32; 16],
}

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat4 {
    /// Identity matrix.
    pub const IDENTITY: Self = Self {
        values: [
            1.0, 0.0, 0.0, 0.0, // column 0
            0.0, 1.0, 0.0, 0.0, // column 1
            0.0, 0.0, 1.0, 0.0, // column 2
            0.0, 0.0, 0.0, 1.0, // column 3
        ],
    };

    /// Validates and copies a column-major matrix.
    pub fn from_array(values: [f32; 16]) -> Result<Self, EmuError> {
        ensure_finite(&values)?;
        Ok(Self { values })
    }

    /// Converts the row-major array layout mandated by JSR-184 to the
    /// renderer's column-major internal representation.
    pub fn from_row_major(values: [f32; 16]) -> Result<Self, EmuError> {
        ensure_finite(&values)?;
        Ok(Self {
            values: std::array::from_fn(|index| {
                let column = index / 4;
                let row = index % 4;
                values[row * 4 + column]
            }),
        })
    }

    /// Exposes the column-major values.
    #[must_use]
    pub const fn as_array(&self) -> &[f32; 16] {
        &self.values
    }

    /// Returns the row-major array layout exposed by JSR-184 `Transform`.
    #[must_use]
    pub fn to_row_major(self) -> [f32; 16] {
        std::array::from_fn(|index| {
            let row = index / 4;
            let column = index % 4;
            self.values[column * 4 + row]
        })
    }

    /// Translation matrix.
    pub fn translation(x: f32, y: f32, z: f32) -> Result<Self, EmuError> {
        ensure_finite(&[x, y, z])?;
        let mut result = Self::IDENTITY;
        result.values[12] = x;
        result.values[13] = y;
        result.values[14] = z;
        Ok(result)
    }

    /// Scale matrix.
    pub fn scale(x: f32, y: f32, z: f32) -> Result<Self, EmuError> {
        ensure_finite(&[x, y, z])?;
        let mut result = Self::IDENTITY;
        result.values[0] = x;
        result.values[5] = y;
        result.values[10] = z;
        Ok(result)
    }

    /// Rotation matrix from an angle in degrees and an axis.
    pub fn rotation(angle_degrees: f32, axis: Vec3) -> Result<Self, EmuError> {
        Self::from_quaternion(Quaternion::from_axis_angle(angle_degrees, axis)?)
    }

    /// Rotation matrix from a normalized quaternion.
    pub fn from_quaternion(quaternion: Quaternion) -> Result<Self, EmuError> {
        let q = quaternion.normalized()?;
        let xx = q.x * q.x;
        let yy = q.y * q.y;
        let zz = q.z * q.z;
        let xy = q.x * q.y;
        let xz = q.x * q.z;
        let yz = q.y * q.z;
        let wx = q.w * q.x;
        let wy = q.w * q.y;
        let wz = q.w * q.z;
        Self::from_array([
            1.0 - 2.0 * (yy + zz),
            2.0 * (xy + wz),
            2.0 * (xz - wy),
            0.0,
            2.0 * (xy - wz),
            1.0 - 2.0 * (xx + zz),
            2.0 * (yz + wx),
            0.0,
            2.0 * (xz + wy),
            2.0 * (yz - wx),
            1.0 - 2.0 * (xx + yy),
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ])
    }

    /// Builds translation * rotation * scale without dense matrix products.
    pub(crate) fn from_components(
        translation: Vec3,
        orientation: Quaternion,
        scale: Vec3,
    ) -> Result<Self, EmuError> {
        ensure_finite(&[translation.x, translation.y, translation.z])?;
        let mut result = Self::from_quaternion(orientation)?;
        ensure_finite(&[scale.x, scale.y, scale.z])?;
        for (column, scale) in [scale.x, scale.y, scale.z].into_iter().enumerate() {
            for row in 0..3 {
                result.values[column * 4 + row] *= scale;
            }
        }
        result.values[12] = translation.x;
        result.values[13] = translation.y;
        result.values[14] = translation.z;
        Ok(result)
    }

    /// Matrix product `self * right`.
    #[must_use]
    pub fn multiplied(self, right: Self) -> Self {
        let mut values = [0.0; 16];
        for column in 0..4 {
            for row in 0..4 {
                let mut value = 0.0;
                for inner in 0..4 {
                    value = self.values[inner * 4 + row]
                        .mul_add(right.values[column * 4 + inner], value);
                }
                values[column * 4 + row] = value;
            }
        }
        Self { values }
    }

    /// Transposes this matrix.
    #[must_use]
    pub fn transposed(self) -> Self {
        let mut values = [0.0; 16];
        for column in 0..4 {
            for row in 0..4 {
                values[column * 4 + row] = self.values[row * 4 + column];
            }
        }
        Self { values }
    }

    /// Computes the inverse with bounded pivoted Gauss-Jordan elimination.
    pub fn inverted(self) -> Result<Self, EmuError> {
        let mut augmented = [[0.0_f64; 8]; 4];
        for (row, row_values) in augmented.iter_mut().enumerate() {
            for (column, value) in row_values.iter_mut().take(4).enumerate() {
                *value = f64::from(self.values[column * 4 + row]);
            }
            row_values[row + 4] = 1.0;
        }
        for pivot_column in 0..4 {
            let mut pivot_row = pivot_column;
            for row in (pivot_column + 1)..4 {
                if augmented[row][pivot_column].abs() > augmented[pivot_row][pivot_column].abs() {
                    pivot_row = row;
                }
            }
            // A small pivot can represent a valid change of units. Keep the
            // elimination wide and reject zero pivots instead of an absolute
            // threshold that incorrectly makes small scales singular.
            if augmented[pivot_row][pivot_column] == 0.0 {
                return Err(math_error(
                    "singular-transform",
                    "transform is not invertible",
                ));
            }
            augmented.swap(pivot_column, pivot_row);
            let pivot = augmented[pivot_column][pivot_column];
            for value in &mut augmented[pivot_column] {
                *value /= pivot;
            }
            let pivot_values = augmented[pivot_column];
            for (row, row_values) in augmented.iter_mut().enumerate() {
                if row == pivot_column {
                    continue;
                }
                let factor = row_values[pivot_column];
                for (value, pivot_value) in row_values.iter_mut().zip(pivot_values) {
                    *value -= factor * pivot_value;
                }
            }
        }
        let values = std::array::from_fn(|index| {
            let column = index / 4;
            let row = index % 4;
            augmented[row][column + 4] as f32
        });
        ensure_finite(&values)?;
        Ok(Self { values })
    }

    /// Transforms one homogeneous vector.
    #[must_use]
    pub fn transform(self, vector: Vec4) -> Vec4 {
        Vec4::new(
            self.values[0].mul_add(
                vector.x,
                self.values[4].mul_add(
                    vector.y,
                    self.values[8].mul_add(vector.z, self.values[12] * vector.w),
                ),
            ),
            self.values[1].mul_add(
                vector.x,
                self.values[5].mul_add(
                    vector.y,
                    self.values[9].mul_add(vector.z, self.values[13] * vector.w),
                ),
            ),
            self.values[2].mul_add(
                vector.x,
                self.values[6].mul_add(
                    vector.y,
                    self.values[10].mul_add(vector.z, self.values[14] * vector.w),
                ),
            ),
            self.values[3].mul_add(
                vector.x,
                self.values[7].mul_add(
                    vector.y,
                    self.values[11].mul_add(vector.z, self.values[15] * vector.w),
                ),
            ),
        )
    }
}

fn normalized_components<const N: usize>(values: [f32; N]) -> Result<[f32; N], EmuError> {
    ensure_finite(&values)?;
    // Squaring finite f32 components can overflow to infinity. Keep the norm
    // and division wide so a large valid axis cannot normalize to zero.
    let length_squared = values
        .iter()
        .map(|&value| f64::from(value).powi(2))
        .sum::<f64>();
    if length_squared == 0.0 {
        return Err(math_error(
            "zero-length",
            "axis, vector or quaternion has zero length",
        ));
    }
    let length = length_squared.sqrt();
    Ok(values.map(|value| (f64::from(value) / length) as f32))
}

/// Rounds a bounded nonnegative channel or depth value.
/// Matches `value.round().clamp(0.0, maximum as f64) as u32`, including NaN.
#[inline]
pub(crate) fn rounded_u32(value: f64, maximum: u32) -> u32 {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // Baseline x86 targets otherwise call libm for every shaded pixel.
        let value = value.clamp(0.0, f64::from(maximum));
        // Adding 0.5 immediately below 0.5 can round the sum to 1.0. Above
        // this boundary the u32 ceiling keeps integer rounding exact in f64.
        if value < 0.5 { 0 } else { (value + 0.5) as u32 }
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        // AArch64 has native ties-away rounding. Preserve that shorter path;
        // the arithmetic x86 replacement is slower on the Android target.
        value.round().clamp(0.0, f64::from(maximum)) as u32
    }
}

pub(crate) fn ensure_finite(values: &[f32]) -> Result<(), EmuError> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(math_error(
            "non-finite",
            "M3G transform contains NaN or infinity",
        ))
    }
}

fn math_error(code: &'static str, message: &'static str) -> EmuError {
    EmuError::new(Category::M3g, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/m3g/math/mod.rs"]
mod tests;
