//! Exact, handle-based managed heap used by the Java VM.
#![allow(clippy::cast_possible_truncation, clippy::missing_errors_doc)]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod arrays;
mod checkpoint;
mod fields;
mod names;

use arrays::normalize;
pub use arrays::{ArrayAccessKind, ArrayKind};
use fields::object_bytes;
pub use fields::{FieldToken, ObjectFields};
use names::SharedNames;

/// Stable reference into the managed heap. The generation prevents stale
/// handles from becoming valid after a slot is reused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Handle {
    slot: u32,
    generation: u32,
}

// Hash the complete identity in one word. Two separate u32 writes make the
// VM's frequent payload lookups pay the streaming hasher overhead twice.
impl std::hash::Hash for Handle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.to_raw());
    }
}

impl Handle {
    #[must_use]
    pub const fn to_raw(self) -> u64 {
        (self.slot as u64) << 32 | self.generation as u64
    }

    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self {
            slot: (raw >> 32) as u32,
            generation: raw as u32,
        }
    }
}

/// Values stored in object fields and arrays.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HeapValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Reference(Option<Handle>),
}

/// The computational category of a Java value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ValueKind {
    Int,
    Long,
    Float,
    Double,
    Reference,
}

impl HeapValue {
    #[must_use]
    pub const fn slots(self) -> usize {
        if matches!(self, Self::Long(_) | Self::Double(_)) {
            2
        } else {
            1
        }
    }

    #[must_use]
    pub const fn kind(self) -> ValueKind {
        match self {
            Self::Int(_) => ValueKind::Int,
            Self::Long(_) => ValueKind::Long,
            Self::Float(_) => ValueKind::Float,
            Self::Double(_) => ValueKind::Double,
            Self::Reference(_) => ValueKind::Reference,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Allocation {
    Object {
        class: Arc<str>,
        fields: ObjectFields,
    },
    Array {
        kind: ArrayKind,
        elements: Vec<HeapValue>,
    },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Slot {
    generation: u32,
    allocation: Option<Allocation>,
    bytes: usize,
    external_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeapError {
    InvalidHandle,
    LimitExceeded,
    MetadataLimitExceeded,
    NegativeArraySize,
    TypeMismatch,
    Bounds,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Collection {
    pub objects: usize,
    pub bytes: usize,
}

/// Bounded mark-and-sweep heap. It never exposes host pointers to the VM.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Heap {
    slots: Vec<Slot>,
    free: Vec<u32>,
    bytes: usize,
    objects: usize,
    peak_bytes: usize,
    peak_objects: usize,
    limit: usize,
    #[serde(skip)]
    names: SharedNames,
}

impl Heap {
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            bytes: 0,
            objects: 0,
            peak_bytes: 0,
            peak_objects: 0,
            limit,
            names: SharedNames::default(),
        }
    }

    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    #[must_use]
    pub const fn peak_bytes(&self) -> usize {
        self.peak_bytes
    }

    #[must_use]
    pub const fn peak_objects(&self) -> usize {
        self.peak_objects
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.objects
    }

    /// Summarizes live allocations by class or exact array shape for opt-in
    /// diagnostics. Entries are ordered from largest to smallest byte total.
    #[must_use]
    pub fn allocation_summary(&self) -> Vec<(String, usize, usize)> {
        let mut summary = HashMap::<String, (usize, usize)>::new();
        for slot in &self.slots {
            let Some(allocation) = &slot.allocation else {
                continue;
            };
            let label = match allocation {
                Allocation::Object { class, .. } => class.to_string(),
                Allocation::Array { kind, elements } => {
                    format!("{kind:?}[{}]", elements.len())
                }
            };
            let entry = summary.entry(label).or_default();
            entry.0 = entry.0.saturating_add(1);
            entry.1 = entry.1.saturating_add(slot.bytes);
        }
        let mut summary = summary
            .into_iter()
            .map(|(label, (count, bytes))| (label, count, bytes))
            .collect::<Vec<_>>();
        summary.sort_unstable_by(|left, right| {
            right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0))
        });
        summary
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterates over allocations that remain live after the most recent GC.
    pub fn live_allocations(&self) -> impl Iterator<Item = &Allocation> {
        self.slots
            .iter()
            .filter_map(|slot| slot.allocation.as_ref())
    }

    fn allocate_with(
        &mut self,
        bytes: usize,
        build: impl FnOnce(&mut SharedNames) -> Result<Allocation, HeapError>,
    ) -> Result<Handle, HeapError> {
        if self
            .bytes
            .checked_add(bytes)
            .is_none_or(|total| total > self.limit)
        {
            return Err(HeapError::LimitExceeded);
        }
        let index = if let Some(index) = self.free.last() {
            *index
        } else {
            u32::try_from(self.slots.len()).map_err(|_| HeapError::LimitExceeded)?
        };
        // Every fallible limit check precedes taking fields or creating array
        // storage. A caller can therefore collect and retry the same input.
        let allocation = build(&mut self.names)?;
        self.bytes += bytes;
        let generation = if self.free.pop().is_some() {
            let slot = &mut self.slots[index as usize];
            slot.allocation = Some(allocation);
            slot.bytes = bytes;
            slot.external_bytes = 0;
            slot.generation
        } else {
            self.slots.push(Slot {
                generation: 0,
                allocation: Some(allocation),
                bytes,
                external_bytes: 0,
            });
            0
        };
        self.objects += 1;
        self.peak_bytes = self.peak_bytes.max(self.bytes);
        self.peak_objects = self.peak_objects.max(self.objects);
        Ok(Handle {
            slot: index,
            generation,
        })
    }

    pub fn get(&self, handle: Handle) -> Result<&Allocation, HeapError> {
        let slot = self
            .slots
            .get(handle.slot as usize)
            .ok_or(HeapError::InvalidHandle)?;
        if slot.generation != handle.generation {
            return Err(HeapError::InvalidHandle);
        }
        slot.allocation.as_ref().ok_or(HeapError::InvalidHandle)
    }

    pub fn get_mut(&mut self, handle: Handle) -> Result<&mut Allocation, HeapError> {
        let slot = self
            .slots
            .get_mut(handle.slot as usize)
            .ok_or(HeapError::InvalidHandle)?;
        if slot.generation != handle.generation {
            return Err(HeapError::InvalidHandle);
        }
        slot.allocation.as_mut().ok_or(HeapError::InvalidHandle)
    }

    /// Replaces storage owned by a live allocation but kept outside its Java
    /// field/array representation, such as a String payload or native image.
    pub fn set_external_bytes(
        &mut self,
        handle: Handle,
        external_bytes: usize,
    ) -> Result<(), HeapError> {
        let slot = self
            .slots
            .get(handle.slot as usize)
            .ok_or(HeapError::InvalidHandle)?;
        if slot.generation != handle.generation || slot.allocation.is_none() {
            return Err(HeapError::InvalidHandle);
        }
        let slot_bytes = slot
            .bytes
            .checked_sub(slot.external_bytes)
            .and_then(|bytes| bytes.checked_add(external_bytes))
            .ok_or(HeapError::LimitExceeded)?;
        let heap_bytes = self
            .bytes
            .checked_sub(slot.external_bytes)
            .and_then(|bytes| bytes.checked_add(external_bytes))
            .filter(|bytes| *bytes <= self.limit)
            .ok_or(HeapError::LimitExceeded)?;
        let slot = &mut self.slots[handle.slot as usize];
        slot.bytes = slot_bytes;
        slot.external_bytes = external_bytes;
        self.bytes = heap_bytes;
        self.peak_bytes = self.peak_bytes.max(heap_bytes);
        Ok(())
    }

    /// Borrows two distinct live allocations at once without exposing raw
    /// pointers. Graphics hot paths use this to stream pixels directly from a
    /// guest source array into the LCD framebuffer instead of first cloning
    /// the whole source region into a temporary Vec.
    pub fn get_pair_mut(
        &mut self,
        first: Handle,
        second: Handle,
    ) -> Result<(&mut Allocation, &mut Allocation), HeapError> {
        if first.slot == second.slot {
            return Err(HeapError::TypeMismatch);
        }
        let [first_slot, second_slot] = self
            .slots
            .get_disjoint_mut([first.slot as usize, second.slot as usize])
            .map_err(|_| HeapError::InvalidHandle)?;
        if first_slot.generation != first.generation || second_slot.generation != second.generation
        {
            return Err(HeapError::InvalidHandle);
        }
        let first = first_slot
            .allocation
            .as_mut()
            .ok_or(HeapError::InvalidHandle)?;
        let second = second_slot
            .allocation
            .as_mut()
            .ok_or(HeapError::InvalidHandle)?;
        Ok((first, second))
    }

    /// Collects everything not reachable from the exact root handle set.
    pub fn collect(&mut self, roots: impl IntoIterator<Item = Handle>) -> Collection {
        let marked = self.reachable_handles(roots);
        self.sweep_unreachable(&marked)
    }

    /// Collects a graph whose objects also have externally owned reference edges.
    ///
    /// The callback receives each reachable, live handle exactly once and returns
    /// any additional handles owned by that frontier. Java and external edges
    /// are followed to a fixed point before sweeping. The immutable frontier
    /// callback cannot invalidate heap handles during this collection.
    pub fn collect_with_external_edges<I: IntoIterator<Item = Handle>>(
        &mut self,
        roots: impl IntoIterator<Item = Handle>,
        mut external_edges: impl FnMut(&[Handle]) -> I,
    ) -> Collection {
        let mut marked = HashSet::new();
        let mut frontier = self.extend_reachable_handles(&mut marked, roots);
        while !frontier.is_empty() {
            frontier = self.extend_reachable_handles(&mut marked, external_edges(&frontier));
        }
        // Reuse the closure we just computed. Re-marking the initial roots
        // would traverse the entire Java graph a second time.
        self.sweep_unreachable(&marked)
    }

    fn sweep_unreachable(&mut self, marked: &HashSet<Handle>) -> Collection {
        let mut result = Collection::default();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let handle = Handle {
                slot: index as u32,
                generation: slot.generation,
            };
            if slot.allocation.is_some() && !marked.contains(&handle) {
                result.objects += 1;
                result.bytes += slot.bytes;
                self.bytes -= slot.bytes;
                slot.allocation = None;
                slot.bytes = 0;
                slot.external_bytes = 0;
                if let Some(generation) = slot.generation.checked_add(1) {
                    slot.generation = generation;
                    self.free.push(index as u32);
                }
            }
        }
        self.objects -= result.objects;
        self.names.sweep();
        result
    }

    /// Returns the exact transitive Java heap closure without mutating the heap.
    /// Native runtimes use this to expand ownership edges before collection.
    #[must_use]
    pub fn reachable_handles(&self, roots: impl IntoIterator<Item = Handle>) -> HashSet<Handle> {
        let mut marked = HashSet::new();
        self.extend_reachable_handles(&mut marked, roots);
        marked
    }

    /// Extends an existing reachability set and returns only newly discovered handles.
    ///
    /// This permits a collector that spans Java and native ownership graphs to
    /// process just the new frontier instead of rescanning the complete Java heap
    /// after every native edge expansion.
    pub fn extend_reachable_handles(
        &self,
        marked: &mut HashSet<Handle>,
        roots: impl IntoIterator<Item = Handle>,
    ) -> Vec<Handle> {
        let mut discovered = Vec::new();
        // Mark on enqueue so shared references occupy one pending slot. A
        // large array pointing to one object must not allocate another array
        // of duplicate handles just to discard them while popping the queue.
        let mut pending: Vec<_> = roots
            .into_iter()
            .filter(|handle| marked.insert(*handle))
            .collect();
        while let Some(handle) = pending.pop() {
            let Ok(allocation) = self.get(handle) else {
                continue;
            };
            discovered.push(handle);
            match allocation {
                Allocation::Object { fields, .. } => {
                    pending.extend(
                        fields
                            .values()
                            .filter_map(reference)
                            .filter(|handle| marked.insert(*handle)),
                    );
                }
                Allocation::Array {
                    kind: ArrayKind::Reference(_),
                    elements,
                } => {
                    pending.extend(
                        elements
                            .iter()
                            .filter_map(reference)
                            .filter(|handle| marked.insert(*handle)),
                    );
                }
                // Primitive storage cannot contain Java references. Large
                // resource/image arrays must not make marking proportional
                // to their byte count on every collection.
                Allocation::Array { .. } => {}
            }
        }
        discovered
    }
}

fn reference(value: &HeapValue) -> Option<Handle> {
    if let HeapValue::Reference(handle) = value {
        *handle
    } else {
        None
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/heap/mod.rs"]
mod tests;
