use super::{
    Allocation, ArrayKind, EmuError, Machine, Value, heap_error, int_argument, reference_argument,
    type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(super) fn system_arraycopy(&mut self, args: &[Value]) -> Result<(), EmuError> {
        let source = reference_argument(args, 0)?;
        let source_position = int_argument(args, 1)?;
        let destination = reference_argument(args, 2)?;
        let destination_position = int_argument(args, 3)?;
        let length = int_argument(args, 4)?;

        let source_kind = self.heap.managed.array_kind(source).map_err(heap_error)?;
        let destination_kind = self
            .heap
            .managed
            .array_kind(destination)
            .map_err(heap_error)?;
        let compatible = matches!(
            (source_kind, destination_kind),
            (ArrayKind::Reference(_), ArrayKind::Reference(_))
        ) || source_kind == destination_kind;
        if !compatible {
            return Err(vm_error(
                "array-store-exception",
                "source and destination array kinds are incompatible",
            ));
        }

        let source_position = usize::try_from(source_position).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative source position",
            )
        })?;
        let destination_position = usize::try_from(destination_position).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative destination position",
            )
        })?;
        let length = usize::try_from(length).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative copy length",
            )
        })?;
        let source_length = self.heap.managed.array_length(source).map_err(heap_error)?;
        let destination_length = self
            .heap
            .managed
            .array_length(destination)
            .map_err(heap_error)?;
        let source_end = source_position.checked_add(length).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "source arraycopy range overflow",
            )
        })?;
        let destination_end = destination_position.checked_add(length).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "destination arraycopy range overflow",
            )
        })?;
        if source_end > source_length || destination_end > destination_length {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "arraycopy range is outside array bounds",
            ));
        }

        self.execution.counters.arraycopy_calls =
            self.execution.counters.arraycopy_calls.saturating_add(1);
        self.execution.counters.arraycopy_elements = self
            .execution
            .counters
            .arraycopy_elements
            .saturating_add(u64::try_from(length).unwrap_or(u64::MAX));
        if length == 0 {
            return Ok(());
        }

        // A self-copy preserves component types and Java's overlap semantics.
        if source == destination {
            let Allocation::Array { elements, .. } =
                self.heap.managed.get_mut(source).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            elements.copy_within(source_position..source_end, destination_position);
            return Ok(());
        }

        let mut failure = None;
        // Assignable component types prove that every element can be copied,
        // including subclass/interface arrays and nested arrays. Otherwise find
        // the compatible prefix without executing guest code or collecting.
        if let (ArrayKind::Reference(source_type), ArrayKind::Reference(destination_type)) =
            (source_kind, destination_kind)
            && !self.is_instance(source_type, destination_type)
        {
            let Allocation::Array { elements, .. } =
                self.heap.managed.get(source).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            for (index, value) in elements[source_position..source_end].iter().enumerate() {
                if index.is_multiple_of(1_024) && self.native_context.execution_cancelled() {
                    return Err(vm_error(
                        "execution-cancelled",
                        "arraycopy reference validation was cancelled by the host",
                    ));
                }
                if let Err(error) = self.validate_reference_component(destination_type, *value) {
                    failure = Some((index, error));
                    break;
                }
            }
        }
        // Java exposes the copied prefix even when a later reference causes
        // ArrayStoreException. Both the full and partial copy share this path.
        let copied = failure.as_ref().map_or(length, |(index, _)| *index);
        if copied > 0 {
            let (
                Allocation::Array {
                    elements: source, ..
                },
                Allocation::Array {
                    elements: destination,
                    ..
                },
            ) = self
                .heap
                .managed
                .get_pair_mut(source, destination)
                .map_err(heap_error)?
            else {
                return Err(type_error());
            };
            destination[destination_position..destination_position + copied]
                .copy_from_slice(&source[source_position..source_position + copied]);
        }
        failure.map_or(Ok(()), |(_, error)| Err(error))
    }
}
