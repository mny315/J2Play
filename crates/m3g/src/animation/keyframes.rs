//! Bounded keyframe storage, circular valid ranges, and sequence sampling.

use super::animation_error;
use super::interpolation::{
    catmull_rom, interpolate_components, quaternion, quaternion_slerp, quaternion_squad,
    quaternion_squad_controls,
};
use crate::Quaternion;
use diagnostics::EmuError;
use std::cell::Cell;

/// JSR-184 keyframe interpolation mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Interpolation {
    /// Hold the preceding keyframe value.
    Step,
    /// Component-wise linear interpolation.
    Linear,
    /// Component-wise cubic spline interpolation.
    Spline,
    /// Quaternion spherical interpolation.
    Slerp,
    /// Spherical cubic quaternion interpolation.
    Squad,
}

/// Behavior outside the valid keyframe range.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RepeatMode {
    /// Clamp to the first or last valid keyframe.
    Constant,
    /// Wrap sequence time by duration.
    Loop,
}

/// Validated keyframe sequence state.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct KeyframeSequenceState {
    components: usize,
    interpolation: Interpolation,
    repeat_mode: RepeatMode,
    duration: i32,
    valid_range: (usize, usize),
    times: Vec<i32>,
    values: Vec<f32>,
    unordered_pairs: usize,
    // One segment of derived data; no per-key cache allocation or guest-state change.
    squad_controls: Cell<Option<(usize, usize, [Quaternion; 2])>>,
}

impl PartialEq for KeyframeSequenceState {
    fn eq(&self, other: &Self) -> bool {
        self.components == other.components
            && self.interpolation == other.interpolation
            && self.repeat_mode == other.repeat_mode
            && self.duration == other.duration
            && self.valid_range == other.valid_range
            && self.times == other.times
            && self.values == other.values
    }
}

impl KeyframeSequenceState {
    pub(crate) fn compact_checkpoint_buffers(&mut self) {
        self.times.shrink_to_fit();
        self.values.shrink_to_fit();
    }

    pub(crate) fn validate_checkpoint(&self) -> Result<(), EmuError> {
        validate_shape(self.times.len(), self.components, self.interpolation)?;
        if self.values.len() != self.times.len() * self.components
            || self.duration < 0
            || self.valid_range.0 >= self.times.len()
            || self.valid_range.1 >= self.times.len()
            || self.times.iter().any(|time| *time < 0)
            || self.values.iter().any(|value| !value.is_finite())
        {
            return Err(animation_error(
                "checkpoint-keyframes",
                "Checkpoint keyframes do not match their declared shape",
            ));
        }
        if self.unordered_pairs != self.count_unordered_pairs() {
            return Err(animation_error(
                "checkpoint-keyframes",
                "Checkpoint keyframe ordering is inconsistent",
            ));
        }
        self.squad_controls.set(None);
        Ok(())
    }

    /// Creates an empty sequence whose keyframes must be populated before sampling.
    pub fn new(
        keyframe_count: usize,
        components: usize,
        interpolation: Interpolation,
    ) -> Result<Self, EmuError> {
        validate_shape(keyframe_count, components, interpolation)?;
        Ok(Self {
            components,
            interpolation,
            repeat_mode: RepeatMode::Constant,
            duration: 0,
            valid_range: (0, keyframe_count - 1),
            times: vec![0; keyframe_count],
            values: vec![0.0; keyframe_count * components],
            unordered_pairs: 0,
            squad_controls: Cell::new(None),
        })
    }

    /// Updates one keyframe atomically after complete validation.
    pub fn set_keyframe(&mut self, index: usize, time: i32, value: &[f32]) -> Result<(), EmuError> {
        if index >= self.times.len() || value.len() < self.components || time < 0 {
            return Err(animation_error(
                "invalid-keyframe",
                "keyframe index, time or value length is invalid",
            ));
        }
        if !value[..self.components]
            .iter()
            .all(|component| component.is_finite())
        {
            return Err(animation_error(
                "non-finite-keyframe",
                "keyframe contains NaN or infinity",
            ));
        }
        let normalized;
        let value = if matches!(
            self.interpolation,
            Interpolation::Slerp | Interpolation::Squad
        ) && value[..self.components]
            .iter()
            .any(|component| component.abs() > 0.0)
        {
            let rotation = quaternion(value).normalized()?;
            normalized = [rotation.x, rotation.y, rotation.z, rotation.w];
            &normalized[..]
        } else {
            // Zero quaternions, including newly constructed keyframes, have
            // undefined interpolation; retain them without inventing a setter error.
            &value[..self.components]
        };
        let start = index * self.components;
        if self.times[index] != time {
            let previous = self.unordered_pairs_touching(index);
            self.times[index] = time;
            self.unordered_pairs =
                self.unordered_pairs - previous + self.unordered_pairs_touching(index);
        }
        self.values[start..start + self.components].copy_from_slice(&value[..self.components]);
        self.squad_controls.set(None);
        Ok(())
    }

    /// Sets the positive sequence duration.
    pub fn set_duration(&mut self, duration: i32) -> Result<(), EmuError> {
        if duration <= 0 {
            return Err(animation_error(
                "invalid-duration",
                "animation duration must be positive",
            ));
        }
        self.duration = duration;
        self.squad_controls.set(None);
        Ok(())
    }

    /// Sets the inclusive valid keyframe range.
    pub fn set_valid_range(&mut self, first: usize, last: usize) -> Result<(), EmuError> {
        if first >= self.times.len() || last >= self.times.len() {
            return Err(animation_error(
                "invalid-valid-range",
                "keyframe valid range is out of bounds",
            ));
        }
        self.valid_range = (first, last);
        self.squad_controls.set(None);
        self.unordered_pairs = self.count_unordered_pairs();
        Ok(())
    }

    /// Sets constant or looping repeat behavior.
    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat_mode = mode;
        self.squad_controls.set(None);
    }

    /// Samples the sequence at sequence time into a newly owned value.
    pub fn sample(&self, sequence_time: f32) -> Result<Vec<f32>, EmuError> {
        if !sequence_time.is_finite() {
            return Err(animation_error(
                "non-finite-time",
                "animation time is NaN or infinity",
            ));
        }
        if self.duration <= 0 {
            return Err(animation_error(
                "invalid-duration",
                "animation duration has not been set",
            ));
        }
        let (first_index, last_index) = self.valid_range;
        if self.times[last_index] > self.duration {
            return Err(animation_error(
                "invalid-duration",
                "animation duration is shorter than the last valid keyframe time",
            ));
        }
        // Setters may temporarily leave the sequence unordered. Maintain the
        // selected range's decreasing-edge count there, not on every sample.
        if self.unordered_pairs != 0 {
            return Err(animation_error(
                "unordered-keyframe",
                "valid keyframe range has decreasing times",
            ));
        }
        let first_time = f64::from(self.times[first_index]);
        let last_time = f64::from(self.times[last_index]);
        let time = match self.repeat_mode {
            RepeatMode::Constant => f64::from(sequence_time),
            RepeatMode::Loop => f64::from(sequence_time).rem_euclid(f64::from(self.duration)),
        };
        if first_index == last_index {
            return Ok(self.components_at(first_index).to_vec());
        }
        if self.repeat_mode == RepeatMode::Constant {
            if time < first_time {
                return Ok(self.components_at(first_index).to_vec());
            }
            if time >= last_time {
                return Ok(self.components_at(last_index).to_vec());
            }
        } else if time < first_time || time >= last_time {
            let duration = f64::from(self.duration);
            let elapsed = if time < first_time {
                time + duration
            } else {
                time
            } - last_time;
            let span = duration - last_time + first_time;
            let amount = if span > 0.0 {
                (elapsed / span) as f32
            } else {
                0.0
            };
            return self.sample_segment(last_index, first_index, amount);
        }
        // Search each sorted slice of a possibly wrapped range. Segments
        // include their start: skip all keys coinciding with the sampled time.
        let (tail, head) = self.valid_times();
        let offset = 1 + first_key_after(&tail[1..], time);
        let upper = if offset < tail.len() {
            first_index + offset
        } else {
            first_key_after(head, time)
        };
        let lower = self.previous_index(upper);
        let start = f64::from(self.times[lower]);
        let end = f64::from(self.times[upper]);
        self.sample_segment(lower, upper, ((time - start) / (end - start)) as f32)
    }

    fn sample_segment(
        &self,
        lower: usize,
        upper: usize,
        amount: f32,
    ) -> Result<Vec<f32>, EmuError> {
        match self.interpolation {
            Interpolation::Step => Ok(self.components_at(lower).to_vec()),
            Interpolation::Linear => Ok(interpolate_components(
                self.components_at(lower),
                self.components_at(upper),
                amount,
            )),
            Interpolation::Spline => {
                let before = self.previous_valid(lower);
                let after = self.next_valid(upper);
                Ok(catmull_rom(
                    self.components_at(before),
                    self.components_at(lower),
                    self.components_at(upper),
                    self.components_at(after),
                    self.tangent_scales(before, lower, upper, after),
                    amount,
                ))
            }
            Interpolation::Slerp => {
                quaternion_slerp(self.components_at(lower), self.components_at(upper), amount)
            }
            Interpolation::Squad => {
                let controls = match self.squad_controls.get() {
                    Some((cached_lower, cached_upper, controls))
                        if cached_lower == lower && cached_upper == upper =>
                    {
                        controls
                    }
                    _ => {
                        let before = self.previous_valid(lower);
                        let after = self.next_valid(upper);
                        let controls = quaternion_squad_controls(
                            self.components_at(before),
                            self.components_at(lower),
                            self.components_at(upper),
                            self.components_at(after),
                            self.tangent_scales(before, lower, upper, after),
                        )?;
                        self.squad_controls.set(Some((lower, upper, controls)));
                        controls
                    }
                };
                quaternion_squad(
                    self.components_at(lower),
                    self.components_at(upper),
                    controls,
                    amount,
                )
            }
        }
    }

    /// Number of components per keyframe.
    #[must_use]
    pub const fn component_count(&self) -> usize {
        self.components
    }

    /// Number of keyframes.
    #[must_use]
    pub fn keyframe_count(&self) -> usize {
        self.times.len()
    }

    /// Interpolation mode.
    #[must_use]
    pub const fn interpolation(&self) -> Interpolation {
        self.interpolation
    }

    /// Copies a keyframe value and returns its time.
    pub fn keyframe(&self, index: usize) -> Result<(i32, &[f32]), EmuError> {
        let time = self.times.get(index).copied().ok_or_else(|| {
            animation_error("invalid-keyframe", "keyframe index is out of bounds")
        })?;
        Ok((time, self.components_at(index)))
    }

    /// Inclusive valid range.
    #[must_use]
    pub const fn valid_range(&self) -> (usize, usize) {
        self.valid_range
    }

    /// Sequence duration.
    #[must_use]
    pub const fn duration(&self) -> i32 {
        self.duration
    }

    /// Repeat behavior.
    #[must_use]
    pub const fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    /// Logical bytes retained by keyframe times and their packed component values.
    #[must_use]
    pub(crate) fn allocated_bytes(&self) -> usize {
        self.times
            .capacity()
            .saturating_mul(size_of::<i32>())
            .saturating_add(self.values.capacity().saturating_mul(size_of::<f32>()))
    }

    fn components_at(&self, index: usize) -> &[f32] {
        let start = index * self.components;
        &self.values[start..start + self.components]
    }

    fn count_unordered_pairs(&self) -> usize {
        let (tail, head) = self.valid_times();
        tail.windows(2).filter(|pair| pair[0] > pair[1]).count()
            + head.windows(2).filter(|pair| pair[0] > pair[1]).count()
            + usize::from(
                self.valid_range.0 > self.valid_range.1
                    && self.times[self.times.len() - 1] > self.times[0],
            )
    }

    fn unordered_pairs_touching(&self, index: usize) -> usize {
        let previous = self.previous_index(index);
        let next = self.next_index(index);
        usize::from(self.contains_edge(previous) && self.times[previous] > self.times[index])
            + usize::from(self.contains_edge(index) && self.times[index] > self.times[next])
    }

    fn contains_edge(&self, index: usize) -> bool {
        let (first, last) = self.valid_range;
        if first <= last {
            index >= first && index < last
        } else {
            index >= first || index < last
        }
    }

    fn valid_times(&self) -> (&[i32], &[i32]) {
        let (first, last) = self.valid_range;
        if first <= last {
            (&self.times[first..=last], &[])
        } else {
            (&self.times[first..], &self.times[..=last])
        }
    }

    fn previous_index(&self, index: usize) -> usize {
        if index == 0 {
            self.times.len() - 1
        } else {
            index - 1
        }
    }

    fn next_index(&self, index: usize) -> usize {
        if index + 1 == self.times.len() {
            0
        } else {
            index + 1
        }
    }

    fn previous_valid(&self, index: usize) -> usize {
        if index == self.valid_range.0 {
            if self.repeat_mode == RepeatMode::Loop {
                self.valid_range.1
            } else {
                index
            }
        } else {
            self.previous_index(index)
        }
    }

    fn next_valid(&self, index: usize) -> usize {
        if index == self.valid_range.1 {
            if self.repeat_mode == RepeatMode::Loop {
                self.valid_range.0
            } else {
                index
            }
        } else {
            self.next_index(index)
        }
    }

    fn interval(&self, first: usize, last: usize) -> f64 {
        let interval = f64::from(self.times[last]) - f64::from(self.times[first]);
        if first == self.valid_range.1 && last == self.valid_range.0 {
            interval + f64::from(self.duration)
        } else {
            interval
        }
    }

    fn tangent_scales(&self, before: usize, first: usize, second: usize, after: usize) -> [f32; 2] {
        let span = self.interval(first, second);
        let start = if before == first || span <= 0.0 {
            0.0
        } else {
            (span / (self.interval(before, first) + span)) as f32
        };
        let end = if second == after || span <= 0.0 {
            0.0
        } else {
            (span / (span + self.interval(second, after))) as f32
        };
        // These include the 1/2 factor in the centered finite difference.
        // Duplicated CONSTANT endpoints therefore have zero tangents.
        [start, end]
    }
}

fn validate_shape(
    keyframe_count: usize,
    components: usize,
    interpolation: Interpolation,
) -> Result<(), EmuError> {
    if keyframe_count == 0 || components == 0 || keyframe_count > 262_144 || components > 16 {
        return Err(animation_error(
            "invalid-keyframe-shape",
            "keyframe or component count is outside configured limits",
        ));
    }
    if matches!(interpolation, Interpolation::Slerp | Interpolation::Squad) && components != 4 {
        return Err(animation_error(
            "invalid-keyframe-shape",
            "SLERP and SQUAD require four quaternion components",
        ));
    }
    Ok(())
}

fn first_key_after(times: &[i32], time: f64) -> usize {
    // Tiny sequences are cheaper to scan; large ones need logarithmic lookup.
    if times.len() <= 32 {
        times
            .iter()
            .position(|key_time| f64::from(*key_time) > time)
            .unwrap_or(times.len())
    } else {
        times.partition_point(|key_time| f64::from(*key_time) <= time)
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/animation/keyframes.rs"]
mod tests;
