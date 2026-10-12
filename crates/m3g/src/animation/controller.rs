//! World-to-sequence time mapping and animation controller state.

use super::animation_error;
use diagnostics::EmuError;

/// Time mapping and weight state of an animation controller.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationControllerState {
    active_interval: (i32, i32),
    speed: f32,
    weight: f32,
    reference_sequence_time: f32,
    reference_world_time: i32,
}

impl Default for AnimationControllerState {
    fn default() -> Self {
        Self {
            // JSR-184 defines equal endpoints as the always-active sentinel.
            active_interval: (0, 0),
            speed: 1.0,
            weight: 1.0,
            reference_sequence_time: 0.0,
            reference_world_time: 0,
        }
    }
}

impl AnimationControllerState {
    /// Sets the half-open active world-time interval.
    pub fn set_active_interval(&mut self, start: i32, end: i32) -> Result<(), EmuError> {
        if start > end {
            return Err(animation_error(
                "invalid-active-interval",
                "animation active interval start exceeds end",
            ));
        }
        self.active_interval = (start, end);
        Ok(())
    }

    /// Changes speed while preserving sequence position at `world_time`.
    pub fn set_speed(&mut self, speed: f32, world_time: i32) -> Result<(), EmuError> {
        if !speed.is_finite() {
            return Err(animation_error(
                "non-finite-speed",
                "animation speed is NaN or infinity",
            ));
        }
        self.reference_sequence_time = self.position(world_time);
        self.reference_world_time = world_time;
        self.speed = speed;
        Ok(())
    }

    /// Sets sequence position at a given reference world time.
    pub fn set_position(&mut self, position: f32, world_time: i32) -> Result<(), EmuError> {
        if !position.is_finite() {
            return Err(animation_error(
                "non-finite-time",
                "animation position is NaN or infinity",
            ));
        }
        self.reference_sequence_time = position;
        self.reference_world_time = world_time;
        Ok(())
    }

    /// Returns mapped sequence position at world time.
    #[must_use]
    pub fn position(&self, world_time: i32) -> f32 {
        f64::from(self.speed).mul_add(
            f64::from(world_time) - f64::from(self.reference_world_time),
            f64::from(self.reference_sequence_time),
        ) as f32
    }

    /// Sets a finite non-negative blend weight.
    pub fn set_weight(&mut self, weight: f32) -> Result<(), EmuError> {
        if !weight.is_finite() || weight < 0.0 {
            return Err(animation_error(
                "invalid-weight",
                "animation weight must be finite and non-negative",
            ));
        }
        self.weight = weight;
        Ok(())
    }

    /// Returns effective weight, or zero outside the active interval.
    #[must_use]
    pub fn effective_weight(&self, world_time: i32) -> f32 {
        let (start, end) = self.active_interval;
        if start == end || world_time >= start && world_time < end {
            self.weight
        } else {
            0.0
        }
    }

    /// Active world-time interval.
    #[must_use]
    pub const fn active_interval(&self) -> (i32, i32) {
        self.active_interval
    }

    /// Playback speed.
    #[must_use]
    pub const fn speed(&self) -> f32 {
        self.speed
    }

    /// Unmasked controller weight.
    #[must_use]
    pub const fn weight(&self) -> f32 {
        self.weight
    }

    /// Reference world time used by position mapping.
    #[must_use]
    pub const fn reference_world_time(&self) -> i32 {
        self.reference_world_time
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/animation/controller.rs"]
mod tests;
