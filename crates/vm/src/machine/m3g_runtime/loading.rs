use super::{
    ArrayKind, EmuError, Handle, HashMap, HeapValue, Machine, Value, heap_error, m3g_bounded_image,
    m3g_object_class, m3g_png_format, normalize_m3g_resource, vm_error,
};

enum LoadDocument {
    File(m3g::M3gFile),
    Image(m3g::Handle),
}

struct PendingFile {
    file: m3g::M3gFile,
    resource: Option<String>,
    guest_references: Vec<Option<u64>>,
    external_handles: Vec<Option<m3g::Handle>>,
    next_object: usize,
}

impl PendingFile {
    fn new(file: m3g::M3gFile, resource: Option<String>) -> Self {
        let count = file.objects.len();
        Self {
            file,
            resource,
            guest_references: vec![None; count],
            external_handles: vec![None; count],
            next_object: 1,
        }
    }

    fn resolve_external(&mut self, handle: m3g::Handle) {
        self.external_handles[self.next_object] = Some(handle);
        self.next_object += 1;
    }
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_load_bytes(
        &mut self,
        bytes: &[u8],
        base_resource: Option<&str>,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let temporary_roots = self.heap.temporary_roots.len();
        let mut created_native = Vec::new();
        let result = (|| {
            let native_roots =
                self.m3g_load_document(bytes, base_resource, roots, &mut created_native)?;
            let result_handles = native_roots
                .into_iter()
                .map(|handle| {
                    self.m3g_guest_handle(Some(handle)).ok_or_else(|| {
                        vm_error(
                            "unbound-guest-object",
                            "loaded M3G root has no guest object",
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let length = i32::try_from(result_handles.len())
                .map_err(|_| vm_error("memory-limit", "too many Loader root objects"))?;
            let array = self.allocate_array(
                ArrayKind::Reference("javax/microedition/m3g/Object3D".into()),
                length,
                &[],
                roots,
            )?;
            for (index, handle) in result_handles.into_iter().enumerate() {
                self.heap
                    .managed
                    .array_set(array, index as i32, HeapValue::Reference(Some(handle)))
                    .map_err(heap_error)?;
            }
            Ok(array)
        })();
        if result.is_err() {
            self.m3g.runtime.rollback_created(&created_native);
        }
        self.heap.temporary_roots.truncate(temporary_roots);
        result
    }

    fn m3g_load_document(
        &mut self,
        bytes: &[u8],
        base_resource: Option<&str>,
        roots: &[Value],
        created_native: &mut Vec<m3g::Handle>,
    ) -> Result<Vec<m3g::Handle>, EmuError> {
        // External files share one operation's parser budgets. Each pending
        // document owns only its parsed data; compressed input is released
        // before descending, and dependency depth never consumes host stack.
        let mut remaining = self.limits.m3g_loader;
        let document = self.m3g_decode_document(bytes, roots, created_native, &mut remaining)?;
        let file = match document {
            LoadDocument::Image(handle) => return Ok(vec![handle]),
            LoadDocument::File(file) => file,
        };
        let mut pending = vec![PendingFile::new(file, base_resource.map(str::to_owned))];
        let mut cache = HashMap::new();
        let mut loading = std::collections::BTreeSet::new();
        loading.extend(base_resource.map(str::to_owned));
        loop {
            self.m3g_check_load_cancellation()?;
            let current = pending.last_mut().expect("a root document remains pending");
            let Some(object) = current.file.objects.get(current.next_object) else {
                let completed = pending.pop().expect("the current document remains pending");
                let loaded = self.m3g_instantiate_document(&completed, roots, created_native)?;
                let Some(parent) = pending.last_mut() else {
                    return Ok(loaded);
                };
                if loaded.len() != 1 {
                    return Err(vm_error(
                        "invalid-external-reference",
                        "M3G external resource must resolve to exactly one root object",
                    ));
                }
                let resource = completed
                    .resource
                    .expect("external documents have a resource name");
                loading.remove(&resource);
                cache.insert(resource, loaded[0]);
                parent.resolve_external(loaded[0]);
                continue;
            };
            if let Some(uri) = object.external_uri()? {
                let resource = normalize_m3g_resource(current.resource.as_deref(), uri)?;
                if let Some(&handle) = cache.get(&resource) {
                    current.resolve_external(handle);
                    continue;
                }
                if !loading.insert(resource.clone()) {
                    return Err(vm_error(
                        "external-reference-cycle",
                        "recursive M3G external-reference cycle",
                    ));
                }
                if pending.len() >= self.limits.m3g_graph_depth {
                    return Err(vm_error(
                        "resource-limit",
                        "M3G external-reference depth exceeds the suite graph budget",
                    ));
                }
                let bytes = self
                    .native_context
                    .read_resource(&resource)?
                    .ok_or_else(|| {
                        vm_error(
                            "external-reference-missing",
                            "M3G external suite resource does not exist",
                        )
                    })?;
                match self.m3g_decode_document(&bytes, roots, created_native, &mut remaining)? {
                    LoadDocument::File(file) => {
                        pending.push(PendingFile::new(file, Some(resource)))
                    }
                    LoadDocument::Image(handle) => {
                        loading.remove(&resource);
                        cache.insert(resource, handle);
                        pending
                            .last_mut()
                            .expect("the parent document remains pending")
                            .resolve_external(handle);
                    }
                }
            } else {
                let class = m3g_object_class(object.object_type).ok_or_else(|| {
                    vm_error(
                        "unsupported-object-type",
                        "M3G object has no guest class mapping",
                    )
                })?;
                let guest = self.m3g_allocate_guest(class, roots)?;
                current.guest_references[current.next_object] = Some(guest.to_raw());
                current.next_object += 1;
            }
        }
    }

    pub(in crate::machine) fn m3g_check_load_cancellation(&self) -> Result<(), EmuError> {
        if self.native_context.execution_cancelled() {
            return Err(vm_error("execution-cancelled", "M3G loading cancelled"));
        }
        Ok(())
    }

    fn m3g_decode_document(
        &mut self,
        bytes: &[u8],
        roots: &[Value],
        created_native: &mut Vec<m3g::Handle>,
        remaining: &mut m3g::LoaderLimits,
    ) -> Result<LoadDocument, EmuError> {
        self.m3g_check_load_cancellation()?;
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            if bytes.len() > remaining.file_bytes {
                return Err(vm_error(
                    "resource-limit",
                    "M3G loader input exceeds the suite file budget",
                ));
            }
            let decoded = graphics::Image::from_png(bytes)?;
            m3g_bounded_image(
                decoded.width(),
                decoded.height(),
                self.limits.m3g_texture_pixels,
            )?;
            consume_loader_budget(&mut remaining.objects, 1)?;
            consume_loader_budget(
                &mut remaining.decompressed_bytes,
                decoded.pixels().len().saturating_mul(4),
            )?;
            let format = m3g_png_format(bytes)?;
            let state = m3g::Image2DState::from_argb(
                format,
                decoded.width(),
                decoded.height(),
                decoded.pixels(),
            )?;
            let guest = self.m3g_allocate_guest("javax/microedition/m3g/Image2D", roots)?;
            let native = self.m3g_allocate_native(roots, |runtime| {
                runtime.create(
                    Some(guest.to_raw()),
                    m3g::ObjectKind::Image2D(state.clone()),
                )
            })?;
            created_native.push(native);
            self.m3g.metrics.loaded_files = self.m3g.metrics.loaded_files.saturating_add(1);
            self.m3g.metrics.loaded_objects = self.m3g.metrics.loaded_objects.saturating_add(1);
            self.m3g.metrics.decompressed_bytes =
                self.m3g.metrics.decompressed_bytes.saturating_add(
                    u64::try_from(decoded.pixels().len().saturating_mul(4)).unwrap_or(u64::MAX),
                );
            return Ok(LoadDocument::Image(native));
        }

        let (file, _) = m3g::M3gFile::parse_prefix(bytes, *remaining)?;
        consume_loader_budget(&mut remaining.objects, file.objects.len())?;
        consume_loader_budget(&mut remaining.sections, file.sections.len())?;
        for section in &file.sections {
            consume_loader_budget(
                &mut remaining.decompressed_bytes,
                section.uncompressed_length as usize,
            )?;
        }
        Ok(LoadDocument::File(file))
    }

    fn m3g_instantiate_document(
        &mut self,
        pending: &PendingFile,
        roots: &[Value],
        created_native: &mut Vec<m3g::Handle>,
    ) -> Result<Vec<m3g::Handle>, EmuError> {
        let file = &pending.file;
        let instantiated = self.m3g_allocate_native(roots, |runtime| {
            m3g::instantiate_file(
                file,
                runtime,
                &pending.guest_references,
                &pending.external_handles,
            )
        })?;
        let local = file
            .objects
            .iter()
            .skip(1)
            .filter(|object| object.object_type != m3g::ObjectType::ExternalReference)
            .filter_map(|object| instantiated.handles[object.index as usize - 1])
            .collect::<Vec<_>>();
        created_native.extend(local.iter().copied());
        for handle in &local {
            if let m3g::ObjectKind::Sprite3D(state) = self.m3g.runtime.kind(*handle)? {
                m3g::SpriteState::validate_crop(
                    state.crop,
                    self.limits.m3g_max_sprite_crop_dimension,
                )?;
            }
        }
        for (slot, parameters) in instantiated.user_parameters.iter().enumerate() {
            self.m3g_check_load_cancellation()?;
            if parameters.is_empty() {
                continue;
            }
            let object = instantiated.handles[slot].ok_or_else(|| {
                vm_error(
                    "unbound-user-parameters",
                    "serialized M3G user parameters have no object",
                )
            })?;
            self.m3g_install_user_parameters(object, parameters, roots)?;
        }
        let mut referenced = std::collections::BTreeSet::new();
        for handle in &local {
            referenced.extend(self.m3g.runtime.references(*handle)?);
        }
        let roots = local
            .into_iter()
            .filter(|handle| !referenced.contains(handle))
            .collect::<Vec<_>>();
        if roots.is_empty() {
            return Err(vm_error(
                "invalid-object-graph",
                "M3G file has no root objects",
            ));
        }
        self.m3g.metrics.loaded_files = self.m3g.metrics.loaded_files.saturating_add(1);
        self.m3g.metrics.loaded_sections = self
            .m3g
            .metrics
            .loaded_sections
            .saturating_add(u64::try_from(file.sections.len()).unwrap_or(u64::MAX));
        self.m3g.metrics.loaded_objects = self
            .m3g
            .metrics
            .loaded_objects
            .saturating_add(u64::try_from(file.objects.len()).unwrap_or(u64::MAX));
        let decompressed = file.sections.iter().fold(0_u64, |total, section| {
            total.saturating_add(u64::from(section.uncompressed_length))
        });
        self.m3g.metrics.decompressed_bytes = self
            .m3g
            .metrics
            .decompressed_bytes
            .saturating_add(decompressed);
        Ok(roots)
    }

    pub(in crate::machine) fn m3g_install_user_parameters(
        &mut self,
        object: m3g::Handle,
        parameters: &[(u32, Vec<u8>)],
        roots: &[Value],
    ) -> Result<(), EmuError> {
        if parameters.is_empty() {
            return Ok(());
        }
        let length = i32::try_from(parameters.len())
            .map_err(|_| vm_error("memory-limit", "too many M3G user parameters"))?;
        let keys = self.allocate_array(
            ArrayKind::Reference("java/lang/Object".into()),
            length,
            &[],
            roots,
        )?;
        self.heap.temporary_roots.push(keys);
        let values = match self.allocate_array(
            ArrayKind::Reference("java/lang/Object".into()),
            length,
            &[],
            roots,
        ) {
            Ok(values) => values,
            Err(error) => {
                self.heap.temporary_roots.pop();
                return Err(error);
            }
        };
        self.heap.temporary_roots.push(values);
        let result = (|| {
            for (index, (id, bytes)) in parameters.iter().enumerate() {
                let key = self.allocate_object(
                    "java/lang/Integer",
                    HashMap::from([(
                        "java/lang/Integer.value:I".to_owned(),
                        HeapValue::Int(id.cast_signed()),
                    )]),
                    &[],
                    roots,
                )?;
                self.heap
                    .managed
                    .array_set(keys, index as i32, HeapValue::Reference(Some(key)))
                    .map_err(heap_error)?;

                let byte_count = i32::try_from(bytes.len())
                    .map_err(|_| vm_error("memory-limit", "M3G user parameter is too large"))?;
                let value = self.allocate_array(ArrayKind::Byte, byte_count, &[], roots)?;
                for (byte_index, byte) in bytes.iter().copied().enumerate() {
                    self.heap
                        .managed
                        .array_set(
                            value,
                            byte_index as i32,
                            HeapValue::Int(i32::from(byte as i8)),
                        )
                        .map_err(heap_error)?;
                }
                self.heap
                    .managed
                    .array_set(values, index as i32, HeapValue::Reference(Some(value)))
                    .map_err(heap_error)?;
            }
            let table = self.allocate_object(
                "java/util/Hashtable",
                HashMap::from([
                    (
                        "java/util/Hashtable.keys:[Ljava/lang/Object;".to_owned(),
                        HeapValue::Reference(Some(keys)),
                    ),
                    (
                        "java/util/Hashtable.values:[Ljava/lang/Object;".to_owned(),
                        HeapValue::Reference(Some(values)),
                    ),
                    (
                        "java/util/Hashtable.count:I".to_owned(),
                        HeapValue::Int(length),
                    ),
                ]),
                &[],
                roots,
            )?;
            self.m3g
                .runtime
                .set_user_object(object, Some(table.to_raw()))
        })();
        self.heap.temporary_roots.pop();
        self.heap.temporary_roots.pop();
        result
    }

    fn m3g_allocate_guest(&mut self, class: &str, roots: &[Value]) -> Result<Handle, EmuError> {
        let guest = self.allocate_native_instance(class, roots)?;
        self.heap.temporary_roots.push(guest);
        Ok(guest)
    }
}

fn consume_loader_budget(remaining: &mut usize, count: usize) -> Result<(), EmuError> {
    *remaining = remaining.checked_sub(count).ok_or_else(|| {
        vm_error(
            "resource-limit",
            "M3G external resources exceed the suite loader budget",
        )
    })?;
    Ok(())
}
