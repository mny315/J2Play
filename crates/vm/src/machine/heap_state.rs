//! Managed heap payloads, GC roots and host-side object backing.

use super::{
    Allocation, Arc, EmuError, Handle, HashMap, HashSet, Heap, ImmutableImagePixels,
    JavaStackFrame, Program, StringValues, heap_error, type_error,
};

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct HeapState {
    pub(super) managed: Heap,
    pub(super) string_values: StringValues,
    pub(super) interned_strings: HashMap<Vec<u16>, Handle>,
    pub(super) weak_references: HashMap<Handle, Option<Handle>>,
    pub(super) throwable_traces: HashMap<Handle, Vec<JavaStackFrame>>,
    // Identify VM-created OOMs even when the Java class omits its detail message.
    pub(super) managed_heap_limit_throwables: HashSet<Handle>,
    pub(super) frame_roots: FrameRoots,
    pub(super) temporary_roots: Vec<Handle>,
    // Shared lossless backing avoids copying immutable images on each draw.
    pub(super) immutable_image_pixels: HashMap<Handle, Arc<ImmutableImagePixels>>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct FrameRoots {
    slots: Vec<Vec<Handle>>,
}

impl FrameRoots {
    pub(super) fn at_depth(&mut self, depth: usize) -> &mut Vec<Handle> {
        if self.slots.len() <= depth {
            self.slots.resize_with(depth.saturating_add(1), Vec::new);
        }
        &mut self.slots[depth]
    }

    pub(super) fn remove(&mut self, depth: usize) {
        if let Some(roots) = self.slots.get_mut(depth) {
            roots.clear();
        }
    }

    pub(super) fn values(&self) -> impl Iterator<Item = &Vec<Handle>> {
        self.slots.iter()
    }
}

impl HeapState {
    pub(super) fn validate_throwable(
        &self,
        program: &Program,
        handle: Handle,
    ) -> Result<(), EmuError> {
        match self.managed.get(handle).map_err(heap_error)? {
            Allocation::Object { class, .. }
                if program.is_assignable_to(class, "java/lang/Throwable") =>
            {
                Ok(())
            }
            _ => Err(type_error()),
        }
    }

    pub(super) fn new(limit: usize) -> Self {
        Self {
            managed: Heap::new(limit),
            string_values: StringValues::new(),
            interned_strings: HashMap::new(),
            weak_references: HashMap::new(),
            throwable_traces: HashMap::new(),
            managed_heap_limit_throwables: HashSet::new(),
            frame_roots: FrameRoots::default(),
            temporary_roots: Vec::new(),
            immutable_image_pixels: HashMap::new(),
        }
    }

    pub(super) fn append_roots(&self, roots: &mut Vec<Handle>) {
        roots.extend(self.frame_roots.values().flatten().copied());
        roots.extend(self.temporary_roots.iter().copied());
        roots.extend(self.interned_strings.values().copied());
    }

    pub(super) fn sweep(&mut self) {
        let managed = &self.managed;
        self.immutable_image_pixels
            .retain(|handle, _| managed.get(*handle).is_ok());
        self.string_values
            .retain(|handle, _| managed.get(*handle).is_ok());
        self.throwable_traces
            .retain(|handle, _| managed.get(*handle).is_ok());
        self.managed_heap_limit_throwables
            .retain(|handle| managed.get(*handle).is_ok());
        self.weak_references.retain(|reference, referent| {
            if managed.get(*reference).is_err() {
                return false;
            }
            if referent.is_some_and(|handle| managed.get(handle).is_err()) {
                *referent = None;
            }
            true
        });
    }
}
