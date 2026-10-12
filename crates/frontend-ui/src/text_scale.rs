//! Host text sizes in logical points, independent of display density and UI zoom.

/// A bounded sample of the platform's text-size conversion, including nonlinear
/// accessibility scaling. Samples cover the UI's font sizes at one-point steps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlatformTextScale {
    points: [f32; Self::SAMPLE_COUNT],
}

impl PlatformTextScale {
    pub const SAMPLE_COUNT: usize = 64;

    /// Convert each nominal size (1 through 64) to density-independent points.
    /// Reject invalid host metrics before they can enter font layout or caches.
    #[must_use]
    pub fn from_points(points: [f32; Self::SAMPLE_COUNT]) -> Option<Self> {
        let mut previous = 0.0;
        for (size, point) in (1_u16..).zip(points) {
            let size = f32::from(size);
            if !point.is_finite()
                || !(size * 0.25..=size * 4.0).contains(&point)
                || point < previous
            {
                return None;
            }
            previous = point;
        }
        Some(Self { points })
    }

    pub(crate) fn font_size_map(self) -> Vec<(f32, f32)> {
        if self == Self::default() {
            return Vec::new();
        }
        std::iter::once((0.0, 0.0))
            .chain(
                (1_u16..)
                    .zip(self.points)
                    .map(|(size, point)| (f32::from(size), point)),
            )
            .collect()
    }
}

impl Default for PlatformTextScale {
    fn default() -> Self {
        let mut points = [0.0; Self::SAMPLE_COUNT];
        for (size, point) in (1_u16..).zip(&mut points) {
            *point = f32::from(size);
        }
        Self { points }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/text_scale.rs"]
mod tests;
