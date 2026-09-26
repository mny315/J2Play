//! Suite-scoped storage for native M3G objects.

use diagnostics::{Category, EmuError};

/// Opaque, generation-checked native object reference.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct Handle {
    index: u32,
    generation: u32,
}

impl Handle {
    /// Slot index, exposed for diagnostics and stable ordering only.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Slot generation used to reject stale references.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// Hard suite-local limits for the native object arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArenaLimits {
    /// Maximum simultaneously live objects.
    pub objects: usize,
    /// Maximum bytes accounted to live objects.
    pub bytes: usize,
}

impl Default for ArenaLimits {
    fn default() -> Self {
        Self {
            objects: 16_384,
            bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Slot<T> {
    generation: u32,
    bytes: usize,
    value: Option<T>,
}

/// Bounded arena which never accepts a stale or fabricated handle.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    limits: ArenaLimits,
    live_objects: usize,
    live_bytes: usize,
    peak_objects: usize,
    peak_bytes: usize,
}

impl<T> Arena<T> {
    pub(crate) fn checkpoint_values(&self) -> impl Iterator<Item = (Handle, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.value.as_ref().map(|value| {
                (
                    Handle {
                        index: index as u32,
                        generation: slot.generation,
                    },
                    value,
                )
            })
        })
    }

    pub(crate) fn validate_checkpoint(
        &mut self,
        limits: ArenaLimits,
        mut retained_bytes: impl FnMut(&mut T) -> usize,
    ) -> Result<(), EmuError> {
        if self.limits != limits
            || self.slots.len() > limits.objects
            || self.free.len() > self.slots.len()
            || self.live_objects > self.peak_objects
            || self.peak_objects > limits.objects
            || self.live_bytes > self.peak_bytes
            || self.peak_bytes > limits.bytes
        {
            return Err(limit_error());
        }

        let mut free = vec![false; self.slots.len()];
        for index in &self.free {
            let listed = free.get_mut(*index as usize).ok_or_else(limit_error)?;
            if std::mem::replace(listed, true) {
                return Err(limit_error());
            }
        }
        let mut objects = 0_usize;
        let mut saved_bytes = 0_usize;
        let mut live_bytes = 0_usize;
        for (slot, free) in self.slots.iter_mut().zip(free) {
            if let Some(value) = &mut slot.value {
                if free || slot.generation == 0 {
                    return Err(limit_error());
                }
                objects += 1;
                saved_bytes = saved_bytes
                    .checked_add(slot.bytes)
                    .ok_or_else(limit_error)?;
                slot.bytes = retained_bytes(value);
                live_bytes = live_bytes
                    .checked_add(slot.bytes)
                    .filter(|bytes| *bytes <= limits.bytes)
                    .ok_or_else(limit_error)?;
            } else if slot.bytes != 0 || free != (slot.generation != 0) {
                // Every reusable slot occurs exactly once; retired slots never do.
                return Err(limit_error());
            }
        }
        if objects != self.live_objects || saved_bytes != self.live_bytes {
            return Err(limit_error());
        }
        self.live_bytes = live_bytes;
        self.peak_bytes = self.peak_bytes.max(live_bytes);
        Ok(())
    }

    /// Creates an empty suite-local arena.
    #[must_use]
    pub const fn new(limits: ArenaLimits) -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            limits,
            live_objects: 0,
            live_bytes: 0,
            peak_objects: 0,
            peak_bytes: 0,
        }
    }

    /// Inserts an object after checking both count and accounted-byte budgets.
    pub fn insert(&mut self, value: T, bytes: usize) -> Result<Handle, EmuError> {
        let next_objects = self.live_objects.checked_add(1).ok_or_else(limit_error)?;
        let next_bytes = self.live_bytes.checked_add(bytes).ok_or_else(limit_error)?;
        if next_objects > self.limits.objects || next_bytes > self.limits.bytes {
            return Err(limit_error());
        }

        let handle = if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            debug_assert!(slot.value.is_none());
            slot.bytes = bytes;
            slot.value = Some(value);
            Handle {
                index,
                generation: slot.generation,
            }
        } else {
            if self.slots.len() >= self.limits.objects {
                return Err(limit_error());
            }
            let index = u32::try_from(self.slots.len()).map_err(|_| limit_error())?;
            self.slots.push(Slot {
                generation: 1,
                bytes,
                value: Some(value),
            });
            Handle {
                index,
                generation: 1,
            }
        };
        self.live_objects = next_objects;
        self.live_bytes = next_bytes;
        self.peak_objects = self.peak_objects.max(next_objects);
        self.peak_bytes = self.peak_bytes.max(next_bytes);
        Ok(handle)
    }

    /// Returns a shared object reference after validating index and generation.
    pub fn get(&self, handle: Handle) -> Result<&T, EmuError> {
        self.slot(handle)?.value.as_ref().ok_or_else(stale_error)
    }

    /// Returns an exclusive object reference after validating index and generation.
    pub fn get_mut(&mut self, handle: Handle) -> Result<&mut T, EmuError> {
        self.slot_mut(handle)?
            .value
            .as_mut()
            .ok_or_else(stale_error)
    }

    /// Atomically replaces a live value after validating its new byte charge.
    pub fn replace(&mut self, handle: Handle, value: T, bytes: usize) -> Result<(), EmuError> {
        self.reaccount(handle, bytes)?;
        self.slot_mut(handle)
            .expect("reaccounted arena slot remains live")
            .value = Some(value);
        Ok(())
    }

    /// Changes one live slot's byte charge without exposing a partial limit update.
    pub(crate) fn reaccount(&mut self, handle: Handle, bytes: usize) -> Result<(), EmuError> {
        let previous_bytes = self.slot(handle)?.bytes;
        let retained_bytes = self
            .live_bytes
            .checked_sub(previous_bytes)
            .ok_or_else(limit_error)?;
        let next_bytes = retained_bytes.checked_add(bytes).ok_or_else(limit_error)?;
        if next_bytes > self.limits.bytes {
            return Err(limit_error());
        }

        let slot = self.slot_mut(handle)?;
        if slot.value.is_none() {
            return Err(stale_error());
        }
        slot.bytes = bytes;
        self.live_bytes = next_bytes;
        self.peak_bytes = self.peak_bytes.max(next_bytes);
        Ok(())
    }

    /// Removes an object and invalidates every copy of its handle.
    pub fn remove(&mut self, handle: Handle) -> Result<T, EmuError> {
        let slot = self.slot_mut(handle)?;
        let value = slot.value.take().ok_or_else(stale_error)?;
        let bytes = slot.bytes;
        slot.bytes = 0;
        slot.generation = next_generation(slot.generation);
        if slot.generation != 0 {
            self.free.push(handle.index);
        }
        self.live_objects -= 1;
        self.live_bytes -= bytes;
        Ok(value)
    }

    /// Removes all objects for suite teardown and invalidates outstanding handles.
    pub fn clear(&mut self) {
        self.free.clear();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.value.take().is_some() {
                slot.generation = next_generation(slot.generation);
            }
            slot.bytes = 0;
            if slot.generation != 0 {
                self.free.push(index as u32);
            }
        }
        self.live_objects = 0;
        self.live_bytes = 0;
    }

    /// Current number of live objects.
    #[must_use]
    pub const fn live_objects(&self) -> usize {
        self.live_objects
    }

    /// Current accounted native bytes.
    #[must_use]
    pub const fn live_bytes(&self) -> usize {
        self.live_bytes
    }

    /// Maximum live object count observed in this runtime.
    #[must_use]
    pub const fn peak_objects(&self) -> usize {
        self.peak_objects
    }

    /// Maximum accounted native bytes observed in this runtime.
    #[must_use]
    pub const fn peak_bytes(&self) -> usize {
        self.peak_bytes
    }

    fn slot(&self, handle: Handle) -> Result<&Slot<T>, EmuError> {
        let slot = self
            .slots
            .get(handle.index as usize)
            .ok_or_else(stale_error)?;
        if slot.generation != handle.generation {
            return Err(stale_error());
        }
        Ok(slot)
    }

    fn slot_mut(&mut self, handle: Handle) -> Result<&mut Slot<T>, EmuError> {
        let slot = self
            .slots
            .get_mut(handle.index as usize)
            .ok_or_else(stale_error)?;
        if slot.generation != handle.generation {
            return Err(stale_error());
        }
        Ok(slot)
    }
}

fn next_generation(generation: u32) -> u32 {
    // Generation zero retires a slot permanently; wrapping to one would
    // resurrect a reference from the first allocation in that slot.
    generation.checked_add(1).unwrap_or(0)
}

fn stale_error() -> EmuError {
    EmuError::new(Category::M3g, "stale-handle", "invalid or stale M3G handle")
}

fn limit_error() -> EmuError {
    EmuError::new(
        Category::M3g,
        "resource-limit",
        "M3G native object arena budget exceeded",
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/m3g/arena/mod.rs"]
mod tests;
