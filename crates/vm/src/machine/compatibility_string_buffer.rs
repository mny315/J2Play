use super::{
    Allocation, ArrayKind, EmuError, Handle, HashMap, HeapValue, Machine, Value, heap_error,
    int_argument, reference_argument, type_error, vm_error,
};

pub(super) struct StringBufferState {
    pub(super) count: usize,
    pub(super) value: Handle,
    capacity: usize,
}

impl Machine<'_, '_> {
    pub(super) fn string_buffer_state(
        &self,
        buffer: Handle,
    ) -> Result<StringBufferState, EmuError> {
        let count =
            usize::try_from(self.graphics_int_field(buffer, "java/lang/StringBuffer.count:I")?)
                .map_err(|_| vm_error("invalid-string-buffer", "negative StringBuffer count"))?;
        let value = self.graphics_reference_field(buffer, "java/lang/StringBuffer.value:[C")?;
        let capacity = self.heap.managed.array_length(value).map_err(heap_error)?;
        if count > capacity {
            return Err(vm_error(
                "invalid-string-buffer",
                "StringBuffer count exceeds backing array",
            ));
        }
        Ok(StringBufferState {
            count,
            value,
            capacity,
        })
    }
    pub(super) fn string_buffer_ensure_capacity(
        &mut self,
        buffer: Handle,
        state: &StringBufferState,
        minimum: usize,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let StringBufferState {
            count,
            value,
            capacity,
        } = *state;
        if minimum <= capacity {
            return Ok(value);
        }
        let new_capacity = minimum.max(capacity.saturating_mul(2).saturating_add(2));
        let new_capacity = i32::try_from(new_capacity)
            .map_err(|_| vm_error("out-of-memory-error", "StringBuffer is too large"))?;
        let new_value = self.allocate_array(ArrayKind::Char, new_capacity, &[], roots)?;
        let (
            Allocation::Array {
                kind: ArrayKind::Char,
                elements: source,
            },
            Allocation::Array {
                kind: ArrayKind::Char,
                elements: destination,
            },
        ) = self
            .heap
            .managed
            .get_pair_mut(value, new_value)
            .map_err(heap_error)?
        else {
            return Err(type_error());
        };
        destination[..count].copy_from_slice(&source[..count]);
        self.heap
            .managed
            .set_field(
                buffer,
                "java/lang/StringBuffer.value:[C",
                HeapValue::Reference(Some(new_value)),
            )
            .map_err(heap_error)?;
        Ok(new_value)
    }

    pub(super) fn string_buffer_append_char(&mut self, args: &[Value]) -> Result<Handle, EmuError> {
        let buffer = reference_argument(args, 0)?;
        let state = self.string_buffer_state(buffer)?;
        let count = state.count;
        let required = count
            .checked_add(1)
            .ok_or_else(|| vm_error("out-of-memory-error", "StringBuffer size overflow"))?;
        let value = self.string_buffer_ensure_capacity(buffer, &state, required, args)?;
        self.heap
            .managed
            .array_set(
                value,
                i32::try_from(count).map_err(|_| type_error())?,
                HeapValue::Int(int_argument(args, 1)? & 0xffff),
            )
            .map_err(heap_error)?;
        self.heap
            .managed
            .set_field(
                buffer,
                "java/lang/StringBuffer.count:I",
                HeapValue::Int(i32::try_from(required).map_err(|_| type_error())?),
            )
            .map_err(heap_error)?;
        Ok(buffer)
    }

    pub(super) fn string_buffer_append_string(
        &mut self,
        args: &[Value],
    ) -> Result<Handle, EmuError> {
        let buffer = reference_argument(args, 0)?;
        let Some(Value::Reference(string)) = args.get(1) else {
            return Err(type_error());
        };
        let length = match string {
            Some(string) => self
                .heap
                .string_values
                .get(string)
                .ok_or_else(type_error)?
                .len(),
            None => 4,
        };
        let state = self.string_buffer_state(buffer)?;
        let count = state.count;
        let required = count
            .checked_add(length)
            .ok_or_else(|| vm_error("out-of-memory-error", "StringBuffer size overflow"))?;
        let value = self.string_buffer_ensure_capacity(buffer, &state, required, args)?;
        // Growth may collect the heap. The arguments keep the source alive;
        // borrow its payload only after that allocation, without a host copy.
        let units = match string {
            Some(string) => self
                .heap
                .string_values
                .get(string)
                .ok_or_else(type_error)?
                .as_slice(),
            None => &[110, 117, 108, 108], // "null" in UTF-16.
        };
        let Allocation::Array {
            kind: ArrayKind::Char,
            elements,
        } = self.heap.managed.get_mut(value).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        for (slot, unit) in elements[count..required].iter_mut().zip(units) {
            *slot = HeapValue::Int(i32::from(*unit));
        }
        self.heap
            .managed
            .set_field(
                buffer,
                "java/lang/StringBuffer.count:I",
                HeapValue::Int(i32::try_from(required).map_err(|_| type_error())?),
            )
            .map_err(heap_error)?;
        Ok(buffer)
    }

    pub(super) fn string_buffer_to_string(
        &mut self,
        buffer: Handle,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let units = self.string_buffer_units(buffer)?;
        let string = self.allocate_object("java/lang/String", HashMap::new(), &[], roots)?;
        self.store_string_units(string, units, 1, &[], roots)?;
        Ok(string)
    }

    pub(super) fn string_from_buffer(&mut self, args: &[Value]) -> Result<(), EmuError> {
        let string = reference_argument(args, 0)?;
        let buffer = reference_argument(args, 1)?;
        let units = self.string_buffer_units(buffer)?;
        self.store_string_units(string, units, 1, &[], args)
    }

    fn string_buffer_units(&self, buffer: Handle) -> Result<Vec<u16>, EmuError> {
        let StringBufferState { count, value, .. } = self.string_buffer_state(buffer)?;
        let Allocation::Array {
            kind: ArrayKind::Char,
            elements,
        } = self.heap.managed.get(value).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        elements[..count]
            .iter()
            .map(|value| match value {
                HeapValue::Int(value) => Ok(*value as u16),
                _ => Err(type_error()),
            })
            .collect()
    }

    pub(super) fn string_buffer_delete(
        &mut self,
        receiver: Handle,
        start: i32,
        end: i32,
        single_character: bool,
    ) -> Result<(), EmuError> {
        let HeapValue::Int(count) = self
            .heap
            .managed
            .field(receiver, "java/lang/StringBuffer.count:I")
            .map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let invalid =
            start < 0 || start > count || start > end || (single_character && start == count);
        if invalid {
            return Err(vm_error(
                "string-index",
                format!("StringBuffer range {start}..{end} outside 0..{count}"),
            ));
        }
        let bounded_end = end.min(count);
        let HeapValue::Reference(Some(value)) = self
            .heap
            .managed
            .field(receiver, "java/lang/StringBuffer.value:[C")
            .map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let start = usize::try_from(start).map_err(|_| type_error())?;
        let end = usize::try_from(bounded_end).map_err(|_| type_error())?;
        let count = usize::try_from(count).map_err(|_| type_error())?;
        let Allocation::Array {
            kind: ArrayKind::Char,
            elements,
        } = self.heap.managed.get_mut(value).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if count > elements.len() {
            return Err(vm_error(
                "invalid-string-buffer",
                "StringBuffer count exceeds backing array",
            ));
        }
        elements.copy_within(end..count, start);
        let new_count = count - (end - start);
        elements[new_count..count].fill(HeapValue::Int(0));
        self.heap
            .managed
            .set_field(
                receiver,
                "java/lang/StringBuffer.count:I",
                HeapValue::Int(i32::try_from(new_count).map_err(|_| type_error())?),
            )
            .map_err(heap_error)
    }
}
