//! Deterministic fixed-point math used by the `Micro3D` public API.

/// `Micro3D` fixed-point unit used by matrices, vectors and trigonometry.
pub const ONE: i32 = 4096;
/// Version 3 trigonometric scale, compatible with 4.12 vector and matrix arithmetic.
pub const TRIG_ONE: i32 = ONE;
/// One full clockwise turn in the public angle representation.
pub const FULL_TURN: i32 = 4096;

/// Public three-component integer vector state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vector3D {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Vector3D {
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub fn inner(self, rhs: Self) -> i32 {
        clamp_i128(self.inner_wide(rhs))
    }

    fn inner_wide(self, rhs: Self) -> i128 {
        i128::from(self.x) * i128::from(rhs.x)
            + i128::from(self.y) * i128::from(rhs.y)
            + i128::from(self.z) * i128::from(rhs.z)
    }

    #[must_use]
    pub fn outer(self, rhs: Self) -> Self {
        let [x, y, z] = self.outer_wide(rhs).map(clamp_i128);
        Self::new(x, y, z)
    }

    fn outer_wide(self, rhs: Self) -> [i128; 3] {
        [
            i128::from(self.y) * i128::from(rhs.z) - i128::from(self.z) * i128::from(rhs.y),
            i128::from(self.z) * i128::from(rhs.x) - i128::from(self.x) * i128::from(rhs.z),
            i128::from(self.x) * i128::from(rhs.y) - i128::from(self.y) * i128::from(rhs.x),
        ]
    }

    /// Normalizes to a length of 4096, leaving the zero vector unchanged.
    pub fn unit(&mut self) {
        *self = normalize_i128([i128::from(self.x), i128::from(self.y), i128::from(self.z)]);
    }
}

/// Public 3x4 affine matrix. Rotational/scaling elements use 4.12 fixed point;
/// translation elements are integer model coordinates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AffineTrans {
    pub values: [i32; 12],
}

impl AffineTrans {
    pub const ZERO: Self = Self { values: [0; 12] };
    pub const IDENTITY: Self = Self {
        values: [ONE, 0, 0, 0, 0, ONE, 0, 0, 0, 0, ONE, 0],
    };

    #[must_use]
    pub const fn new(values: [i32; 12]) -> Self {
        Self { values }
    }

    #[must_use]
    pub fn transform(self, vector: Vector3D) -> Vector3D {
        let coordinate = |row: usize| {
            let base = row * 4;
            let scaled = i128::from(self.values[base]) * i128::from(vector.x)
                + i128::from(self.values[base + 1]) * i128::from(vector.y)
                + i128::from(self.values[base + 2]) * i128::from(vector.z);
            clamp_i128(div_round_i128(scaled, i128::from(ONE)) + i128::from(self.values[base + 3]))
        };
        Vector3D::new(coordinate(0), coordinate(1), coordinate(2))
    }

    /// Transforms a surface normal by the inverse transpose of the linear
    /// matrix, ignoring translation, and normalizes it to the `Micro3D` unit.
    #[must_use]
    pub fn transform_normal(self, normal: Vector3D) -> Vector3D {
        self.normal_transform()(normal)
    }

    /// Prepares the inverse transpose once for a batch of surface normals.
    pub(crate) fn normal_transform(self) -> impl Fn(Vector3D) -> Vector3D {
        let [a, b, c, _, d, e, f, _, g, h, i, _] = self.values.map(i128::from);
        let cofactors = [
            e * i - f * h,
            f * g - d * i,
            d * h - e * g,
            c * h - b * i,
            a * i - c * g,
            b * g - a * h,
            b * f - c * e,
            c * d - a * f,
            a * e - b * d,
        ];
        let determinant = a * cofactors[0] + b * cofactors[1] + c * cofactors[2];
        let sign = determinant.signum();
        move |normal| {
            if sign == 0 {
                return Vector3D::default();
            }
            let components = [
                i128::from(normal.x),
                i128::from(normal.y),
                i128::from(normal.z),
            ];
            // i32 matrix/normal components keep these cofactor products and
            // their sums below 2^96, comfortably inside i128.
            let transformed = [0, 1, 2].map(|row| {
                sign * (0..3)
                    .map(|column| cofactors[row * 3 + column] * components[column])
                    .sum::<i128>()
            });
            normalize_i128(transformed)
        }
    }

    #[must_use]
    pub fn multiplied(self, rhs: Self) -> Self {
        let mut output = [0_i32; 12];
        for row in 0..3 {
            for column in 0..3 {
                let value = (0..3).fold(0_i64, |sum, inner| {
                    sum.saturating_add(
                        i64::from(self.values[row * 4 + inner])
                            * i64::from(rhs.values[inner * 4 + column]),
                    )
                });
                output[row * 4 + column] = clamp_i64(div_round(value, i64::from(ONE)));
            }
            let translation = (0..3).fold(0_i64, |sum, inner| {
                sum.saturating_add(
                    i64::from(self.values[row * 4 + inner]) * i64::from(rhs.values[inner * 4 + 3]),
                )
            });
            output[row * 4 + 3] = clamp_i64(
                div_round(translation, i64::from(ONE)) + i64::from(self.values[row * 4 + 3]),
            );
        }
        Self::new(output)
    }

    #[must_use]
    pub fn rotation_x(angle: i32) -> Self {
        let sine = sin(angle);
        let cosine = cos(angle);
        Self::new([ONE, 0, 0, 0, 0, cosine, -sine, 0, 0, sine, cosine, 0])
    }

    #[must_use]
    pub fn rotation_y(angle: i32) -> Self {
        let sine = sin(angle);
        let cosine = cos(angle);
        Self::new([cosine, 0, sine, 0, 0, ONE, 0, 0, -sine, 0, cosine, 0])
    }

    #[must_use]
    pub fn rotation_z(angle: i32) -> Self {
        let sine = sin(angle);
        let cosine = cos(angle);
        Self::new([cosine, -sine, 0, 0, sine, cosine, 0, 0, 0, 0, ONE, 0])
    }

    #[must_use]
    pub fn rotation(axis: Vector3D, angle: i32) -> Self {
        let mut axis = axis;
        axis.unit();
        if axis == Vector3D::default() {
            return Self::IDENTITY;
        }
        let x = i64::from(axis.x);
        let y = i64::from(axis.y);
        let z = i64::from(axis.z);
        let cosine = i64::from(cos(angle));
        let sine = i64::from(sin(angle));
        let inverse = i64::from(ONE) - cosine;
        let component = |a: i64, b: i64| div_round(a * b, i64::from(ONE));
        let diagonal = |a: i64| cosine + component(component(a, a), inverse);
        let mixed = |a: i64, b: i64, cross: i64| {
            component(component(a, b), inverse) + component(cross, sine)
        };
        Self::new([
            clamp_i64(diagonal(x)),
            clamp_i64(mixed(x, y, -z)),
            clamp_i64(mixed(x, z, y)),
            0,
            clamp_i64(mixed(y, x, z)),
            clamp_i64(diagonal(y)),
            clamp_i64(mixed(y, z, -x)),
            0,
            clamp_i64(mixed(z, x, -y)),
            clamp_i64(mixed(z, y, x)),
            clamp_i64(diagonal(z)),
            0,
        ])
    }

    #[must_use]
    pub fn look_at(position: Vector3D, look: Vector3D, up: Vector3D) -> Self {
        // `look` is a viewing direction independent of `position`.
        let mut forward = look;
        forward.unit();
        // Cross the original directions before rounding either one. Clamping
        // the product or normalizing its inputs first can change the camera's
        // orientation or collapse nearly parallel directions onto each other.
        let side = normalize_i128(look.outer_wide(up));
        let actual_up = normalize_i128(forward.outer_wide(side));
        // The public vector dot product clamps to an integer, but this value
        // still carries the fixed-point scale. Clamp only after dividing it out.
        let translation = |axis: Vector3D| {
            clamp_i128(-div_round_i128(axis.inner_wide(position), i128::from(ONE)))
        };
        Self::new([
            side.x,
            side.y,
            side.z,
            translation(side),
            actual_up.x,
            actual_up.y,
            actual_up.z,
            translation(actual_up),
            forward.x,
            forward.y,
            forward.z,
            translation(forward),
        ])
    }
}

/// Integer square root of the absolute value, matching the documented sign-insensitive input.
#[must_use]
pub fn sqrt(value: i32) -> i32 {
    value.unsigned_abs().isqrt().cast_signed()
}

/// Sine for 4096 units per turn, scaled by 4096.
#[must_use]
pub fn sin(angle: i32) -> i32 {
    trig(angle)
}

/// Cosine for 4096 units per turn, scaled by 4096.
#[must_use]
pub fn cos(angle: i32) -> i32 {
    trig(angle.wrapping_add(FULL_TURN / 4))
}

fn trig(angle: i32) -> i32 {
    let normalized = angle.rem_euclid(FULL_TURN);
    let quadrant = normalized / 1024;
    let offset = normalized % 1024;
    let x = if quadrant & 1 == 0 {
        offset
    } else {
        1024 - offset
    };
    let x = i64::from(x);
    // Bhaskara I evaluated in integer arithmetic. The endpoints and quadrant
    // boundaries are exact, and every host architecture follows the same path.
    let numerator = 16_i64 * x * (2048 - x) * i64::from(TRIG_ONE);
    let denominator = 5_i64 * 2048 * 2048 - 4_i64 * x * (2048 - x);
    let magnitude = div_round(numerator, denominator);
    let signed = if quadrant >= 2 { -magnitude } else { magnitude };
    clamp_i64(signed)
}

fn normalize_i128(values: [i128; 3]) -> Vector3D {
    let magnitudes = values.map(i128::unsigned_abs);
    let maximum = magnitudes[0].max(magnitudes[1]).max(magnitudes[2]);
    let Some(exponent) = maximum.checked_ilog2() else {
        return Vector3D::default();
    };
    // Keep the largest magnitude in [2^30, 2^31). This preserves fractional
    // precision before the square root, even for (1, 1, 0), while all three
    // squares and the final fixed-point numerators fit in u64.
    let scaled = magnitudes.map(|value| {
        (if exponent > 30 {
            value >> (exponent - 30)
        } else {
            value << (30 - exponent)
        }) as u64
    });
    let length = scaled
        .iter()
        .map(|value| value * value)
        .sum::<u64>()
        .isqrt();
    let component = |index: usize| {
        // Each magnitude is at most the length, so the result is at most ONE.
        let rounded =
            ((scaled[index] * u64::from(ONE.unsigned_abs()) + length / 2) / length) as i32;
        if values[index] < 0 { -rounded } else { rounded }
    };
    Vector3D::new(component(0), component(1), component(2))
}

pub(crate) fn div_round(value: i64, divisor: i64) -> i64 {
    if value >= 0 {
        value.saturating_add(divisor / 2) / divisor
    } else {
        value.saturating_sub(divisor / 2) / divisor
    }
}

fn div_round_i128(value: i128, divisor: i128) -> i128 {
    if value >= 0 {
        (value + divisor / 2) / divisor
    } else {
        (value - divisor / 2) / divisor
    }
}

fn clamp_i64(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn clamp_i128(value: i128) -> i32 {
    value.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

#[cfg(test)]
#[path = "../../../tests/unit/micro3d/math/mod.rs"]
mod tests;
