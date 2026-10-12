//! Native boundary allocation, ownership and garbage collection.

use super::{
    Allocation, ArrayKind, DefaultNativeContext, EmuError, Handle, HashMap, Heap, HeapError,
    HeapValue, HostServices, MMAPI_HANDLE_FIELDS, MachineNativeContext, collect_native_heap,
    heap_error, reject_canonical_string_mutation, type_error, value_reference, vm_error,
};

mod services;
mod vm_access;

impl HostServices for DefaultNativeContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

pub(super) fn live_mmapi_handles(heap: &Heap) -> Vec<u64> {
    let mut handles = heap
        .live_allocations()
        .filter_map(|allocation| {
            let Allocation::Object { class, fields } = allocation else {
                return None;
            };
            let (_, handle_field) = MMAPI_HANDLE_FIELDS
                .iter()
                .find(|(owner_class, _)| *owner_class == class.as_ref())?;
            let HeapValue::Long(handle) = fields.get(handle_field)? else {
                return None;
            };
            u64::try_from(*handle).ok().filter(|handle| *handle != 0)
        })
        .collect::<Vec<_>>();
    handles.sort_unstable();
    handles.dedup();
    handles
}

pub(super) fn is_mmapi_resource_pressure(error: &EmuError) -> bool {
    // Per-clip synthesis work cannot be reduced by collecting other players.
    // Keep media-work-limit out of this allocation-pressure retry path.
    matches!(
        error.code(),
        "player-limit" | "media-limit" | "audio-resource-limit"
    )
}

impl MachineNativeContext<'_> {
    fn string_units(&self, reference: u64) -> Result<&[u16], EmuError> {
        self.heap
            .string_values
            .get(&Handle::from_raw(reference))
            .map(Vec::as_slice)
            .ok_or_else(|| {
                vm_error(
                    "type-mismatch",
                    "native reference is not a java/lang/String",
                )
            })
    }

    fn collect_heap(&mut self, extra_root: Option<Handle>) {
        // Rebuild the snapshot for each collection: native callbacks may add
        // or remove ownership edges, and extra roots live only for this GC.
        let mut roots = self
            .arguments
            .iter()
            .filter_map(value_reference)
            .collect::<Vec<_>>();
        roots.extend(extra_root);
        self.heap.append_roots(&mut roots);
        self.classes.append_roots(&mut roots);
        self.scheduler.append_roots(&mut roots);
        self.m3g.append_roots(&mut roots);
        self.micro3d.append_roots(&mut roots);
        self.jsr239.append_roots(&mut roots);
        collect_native_heap(
            &mut self.heap.managed,
            self.m3g,
            self.micro3d,
            &self.scheduler.timer_threads,
            roots,
        );
        let live_mmapi_handles = live_mmapi_handles(&self.heap.managed);
        self.heap.sweep();
        self.scheduler.sweep(&self.heap.managed);
        let heap = &self.heap.managed;
        self.m3g.sweep(heap);
        self.micro3d.sweep(heap);
        self.host.mmapi_retain_handles(&live_mmapi_handles);
    }

    fn retry_mmapi_resource<T>(
        &mut self,
        mut operation: impl FnMut(&mut dyn HostServices) -> Result<T, EmuError>,
    ) -> Result<T, EmuError> {
        let result = operation(self.host);
        if !result.as_ref().is_err_and(is_mmapi_resource_pressure) {
            return result;
        }
        self.collect_heap(None);
        operation(self.host)
    }

    fn allocate_native_array(
        &mut self,
        kind: &ArrayKind,
        length: i32,
    ) -> Result<(Handle, &mut [HeapValue]), EmuError> {
        let handle = match self.heap.managed.allocate_array(kind.clone(), length) {
            Ok(handle) => handle,
            Err(HeapError::LimitExceeded | HeapError::MetadataLimitExceeded) => {
                self.collect_heap(None);
                self.heap
                    .managed
                    .allocate_array(kind.clone(), length)
                    .map_err(|error| {
                        if *kind == ArrayKind::Int && std::env::var_os("J2PLAY_TRACE_HEAP").is_some() {
                            eprintln!(
                                "j2play: vm[heap]: native int[{length}] allocation failed after GC; live-bytes={} live-objects={}",
                                self.heap.managed.bytes(),
                                self.heap.managed.len()
                            );
                            for (label, count, bytes) in
                                self.heap.managed.allocation_summary().into_iter().take(24)
                            {
                                eprintln!(
                                    "j2play: vm[heap]: live {label}: count={count} bytes={bytes}"
                                );
                            }
                        }
                        heap_error(error)
                    })?
            }
            Err(error) => return Err(heap_error(error)),
        };
        let Allocation::Array { elements, .. } =
            self.heap.managed.get_mut(handle).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        Ok((handle, elements))
    }

    fn allocate_native_object(&mut self, class: &str) -> Result<Handle, EmuError> {
        match self.heap.managed.allocate_object(class, HashMap::new()) {
            Ok(handle) => Ok(handle),
            Err(HeapError::LimitExceeded | HeapError::MetadataLimitExceeded) => {
                self.collect_heap(None);
                self.heap
                    .managed
                    .allocate_object(class, HashMap::new())
                    .map_err(heap_error)
            }
            Err(error) => Err(heap_error(error)),
        }
    }

    fn allocate_native_string(&mut self, units: Vec<u16>) -> Result<u64, EmuError> {
        let handle = self.allocate_native_object("java/lang/String")?;
        self.store_string_units(handle, units)?;
        Ok(handle.to_raw())
    }

    fn store_string_units(&mut self, handle: Handle, units: Vec<u16>) -> Result<(), EmuError> {
        reject_canonical_string_mutation(
            handle,
            &self.heap.string_values,
            &self.heap.interned_strings,
        )?;
        let bytes = units
            .len()
            .checked_mul(std::mem::size_of::<u16>())
            .ok_or_else(|| vm_error("out-of-memory-error", "String payload size overflow"))?;
        self.account_external_bytes(handle, bytes)?;
        self.heap.string_values.insert(handle, units);
        Ok(())
    }

    fn account_external_bytes(&mut self, handle: Handle, bytes: usize) -> Result<(), EmuError> {
        match self.heap.managed.set_external_bytes(handle, bytes) {
            Ok(()) => {}
            Err(HeapError::LimitExceeded) => {
                self.collect_heap(Some(handle));
                self.heap
                    .managed
                    .set_external_bytes(handle, bytes)
                    .map_err(heap_error)?;
            }
            Err(error) => return Err(heap_error(error)),
        }
        Ok(())
    }
}
