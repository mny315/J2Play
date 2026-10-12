use super::super::constant_string_units;
use super::{
    Arc, ArrayKind, EmuError, FieldToken, Handle, HashMap, HeapError, HeapValue,
    ImmutableImagePixels, Machine, Method, Value, array_kind_from_descriptor, collect_native_heap,
    heap_error, live_mmapi_handles, reject_canonical_string_mutation, type_error, value_reference,
    vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn publish_frame_roots(
        &mut self,
        depth: usize,
        locals: &[Option<Value>],
        stack: &[Value],
        extra: &[Value],
    ) {
        let roots = self.heap.frame_roots.at_depth(depth);
        roots.clear();
        roots.extend(
            locals
                .iter()
                .flatten()
                .chain(stack)
                .chain(extra)
                .filter_map(value_reference),
        );
    }

    pub(in crate::machine) fn roots(
        &self,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Vec<Handle> {
        let mut roots = locals
            .iter()
            .flatten()
            .chain(stack)
            .filter_map(value_reference)
            .collect::<Vec<_>>();
        self.heap.append_roots(&mut roots);
        self.classes.append_roots(&mut roots);
        self.scheduler.append_roots(&mut roots);
        self.m3g.append_roots(&mut roots);
        self.micro3d.append_roots(&mut roots);
        self.jsr239.append_roots(&mut roots);
        roots
    }

    pub(in crate::machine) fn collect_heap(&mut self, roots: Vec<Handle>) {
        collect_native_heap(
            &mut self.heap.managed,
            &self.m3g,
            &self.micro3d,
            &self.scheduler.timer_threads,
            roots,
        );
        let live_mmapi_handles = live_mmapi_handles(&self.heap.managed);
        self.heap.sweep();
        self.scheduler.sweep(&self.heap.managed);
        let heap = &self.heap.managed;
        self.m3g.sweep(heap);
        self.micro3d.sweep(heap);
        self.native_context
            .mmapi_retain_handles(&live_mmapi_handles);
    }

    pub(in crate::machine) fn trace_heap_limit_failure(&self, allocation: std::fmt::Arguments<'_>) {
        if std::env::var_os("J2PLAY_TRACE_HEAP").is_none() {
            return;
        }
        eprintln!(
            "j2play: vm[heap]: {allocation} failed after GC; live-bytes={} limit={} live-objects={}",
            self.heap.managed.bytes(),
            self.limits.max_heap_bytes,
            self.heap.managed.len()
        );
        for (label, count, bytes) in self.heap.managed.allocation_summary().into_iter().take(24) {
            eprintln!("j2play: vm[heap]: live {label}: count={count} bytes={bytes}");
        }
    }

    pub(in crate::machine) fn initial_instance_fields(
        &self,
        class: &str,
    ) -> Result<Vec<(Arc<str>, FieldToken, HeapValue)>, EmuError> {
        Ok(self
            .instance_fields(class)?
            .into_iter()
            .map(|field| (field.key.clone(), field.field_token.clone(), field.initial))
            .collect())
    }

    pub(in crate::machine) fn allocate_native_instance(
        &mut self,
        class: &str,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let fields = self.initial_instance_fields(class)?;
        self.allocate_linked_object(class, fields, &[], roots)
    }

    pub(in crate::machine) fn allocate_object(
        &mut self,
        class: &str,
        mut fields: HashMap<String, HeapValue>,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        // Every linked class uses one superclass-first layout. Numeric field
        // slots are therefore stable for a declaring class and all of its
        // subclasses. Synthetic test/native objects retain an exact-name
        // fallback by sorting any entries not described by the Program.
        let mut ordered = Vec::with_capacity(fields.len());
        if !fields.is_empty()
            && let Ok(layout) = self.instance_fields(class)
        {
            for field in layout {
                if let Some(value) = fields.remove(field.key.as_ref()) {
                    ordered.push((field.key.clone(), field.field_token.clone(), value));
                }
            }
        }
        let mut remaining = fields.into_iter().collect::<Vec<_>>();
        remaining.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        ordered.extend(
            remaining
                .into_iter()
                .map(|(key, value)| (key.into(), FieldToken::new(), value)),
        );

        self.allocate_linked_object(class, ordered, locals, stack)
    }

    pub(in crate::machine) fn allocate_linked_object(
        &mut self,
        class: &str,
        mut ordered: Vec<(Arc<str>, FieldToken, HeapValue)>,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        match self
            .heap
            .managed
            .allocate_object_linked(class, &mut ordered)
        {
            Ok(handle) => Ok(handle),
            Err(HeapError::LimitExceeded | HeapError::MetadataLimitExceeded) => {
                let mut roots = self.roots(locals, stack);
                // The not-yet-allocated object can be the only owner of a
                // reference supplied by a native adapter. Keep its prospective
                // fields alive while collection makes room for the object.
                roots.extend(
                    ordered
                        .iter()
                        .filter_map(|(_, _, value)| value_reference(value)),
                );
                self.collect_heap(roots);
                self.heap
                    .managed
                    .allocate_object_linked(class, &mut ordered)
                    .map_err(|error| {
                        self.trace_heap_limit_failure(format_args!("object {class}"));
                        heap_error(error)
                    })
            }
            Err(error) => Err(heap_error(error)),
        }
    }

    pub(in crate::machine) fn allocate_array(
        &mut self,
        kind: ArrayKind,
        length: i32,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        match self.heap.managed.allocate_array(kind.clone(), length) {
            Ok(handle) => Ok(handle),
            Err(HeapError::LimitExceeded | HeapError::MetadataLimitExceeded) => {
                let roots = self.roots(locals, stack);
                self.collect_heap(roots);
                self.heap
                    .managed
                    .allocate_array(kind, length)
                    .map_err(|error| {
                        self.trace_heap_limit_failure(format_args!("array[{length}]"));
                        heap_error(error)
                    })
            }
            Err(error) => Err(heap_error(error)),
        }
    }

    pub(in crate::machine) fn allocate_multi_array(
        &mut self,
        descriptor: &str,
        lengths: &[i32],
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        if self.native_context.execution_cancelled() {
            return Err(vm_error(
                "execution-cancelled",
                "multidimensional array allocation cancelled",
            ));
        }
        let (&length, rest) = lengths
            .split_first()
            .ok_or_else(|| vm_error("invalid-array", "zero dimensions"))?;
        let component = descriptor
            .strip_prefix('[')
            .ok_or_else(|| vm_error("invalid-array", descriptor))?;
        let kind = if rest.is_empty() {
            array_kind_from_descriptor(component)?
        } else {
            ArrayKind::Reference(component.into())
        };
        let handle = self.allocate_array(kind, length, locals, stack)?;
        if !rest.is_empty() {
            self.heap.temporary_roots.push(handle);
            let result = (|| {
                for index in 0..length {
                    let child = self.allocate_multi_array(component, rest, locals, stack)?;
                    self.heap
                        .managed
                        .array_set(handle, index, HeapValue::Reference(Some(child)))
                        .map_err(heap_error)?;
                }
                Ok(())
            })();
            self.heap.temporary_roots.pop();
            result?;
        }
        Ok(handle)
    }

    pub(in crate::machine) fn account_external_bytes(
        &mut self,
        handle: Handle,
        bytes: usize,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<(), EmuError> {
        match self.heap.managed.set_external_bytes(handle, bytes) {
            Ok(()) => Ok(()),
            Err(HeapError::LimitExceeded) => {
                let mut roots = self.roots(locals, stack);
                roots.push(handle);
                self.collect_heap(roots);
                self.heap
                    .managed
                    .set_external_bytes(handle, bytes)
                    .map_err(heap_error)
            }
            Err(error) => Err(heap_error(error)),
        }
    }

    pub(in crate::machine) fn store_string_units(
        &mut self,
        handle: Handle,
        units: Vec<u16>,
        retained_copies: usize,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<(), EmuError> {
        reject_canonical_string_mutation(
            handle,
            &self.heap.string_values,
            &self.heap.interned_strings,
        )?;
        let bytes = units
            .len()
            .checked_mul(std::mem::size_of::<u16>())
            .and_then(|bytes| bytes.checked_mul(retained_copies))
            .ok_or_else(|| vm_error("out-of-memory-error", "String payload size overflow"))?;
        self.account_external_bytes(handle, bytes, locals, stack)?;
        self.heap.string_values.insert(handle, units);
        Ok(())
    }

    pub(in crate::machine) fn store_immutable_image_pixels(
        &mut self,
        handle: Handle,
        pixels: Arc<ImmutableImagePixels>,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<(), EmuError> {
        let bytes = pixels
            .storage_bytes()
            .ok_or_else(|| vm_error("out-of-memory-error", "image payload size overflow"))?;
        if let Err(error) = self.account_external_bytes(handle, bytes, locals, stack) {
            self.trace_heap_limit_failure(format_args!("immutable image payload ({bytes} bytes)"));
            return Err(error);
        }
        self.heap.immutable_image_pixels.insert(handle, pixels);
        Ok(())
    }

    pub(in crate::machine) fn intern_string_constant(
        &mut self,
        method: &Method,
        string_index: u16,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        if let Some(handle) = self
            .classes
            .constant_pool_cache(method)
            .and_then(|cache| cache.string_literals.get(string_index))
        {
            return Ok(*handle);
        }
        let units = constant_string_units(&method.constants, string_index)?;
        let handle = self.intern_string_units(units, locals, stack)?;
        if let Some(cache) = self.classes.constant_pool_cache_mut(method) {
            cache.string_literals.insert(string_index, handle);
        }
        Ok(handle)
    }

    pub(in crate::machine) fn intern_string(
        &mut self,
        text: &str,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        self.intern_string_units(text.encode_utf16().collect(), locals, stack)
    }

    pub(in crate::machine) fn intern_string_units(
        &mut self,
        units: Vec<u16>,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        if let Some(handle) = self.heap.interned_strings.get(&units) {
            return Ok(*handle);
        }
        let handle = self.allocate_object("java/lang/String", HashMap::new(), locals, stack)?;
        self.store_string_units(handle, units.clone(), 2, locals, stack)?;
        self.heap.interned_strings.insert(units, handle);
        Ok(handle)
    }

    pub(in crate::machine) fn allocate_dynamic_string(
        &mut self,
        text: &str,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        let handle = self.allocate_object("java/lang/String", HashMap::new(), locals, stack)?;
        self.store_string_units(handle, text.encode_utf16().collect(), 1, locals, stack)?;
        Ok(handle)
    }

    pub(in crate::machine) fn intern_existing_string(
        &mut self,
        string: Handle,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let units = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        if let Some(canonical) = self.heap.interned_strings.get(units) {
            return Ok(*canonical);
        }
        let units = units.clone();
        let bytes = units
            .len()
            .checked_mul(std::mem::size_of::<u16>() * 2)
            .ok_or_else(|| vm_error("out-of-memory-error", "String payload size overflow"))?;
        self.account_external_bytes(string, bytes, &[], roots)?;
        self.heap.interned_strings.insert(units, string);
        Ok(string)
    }
}
