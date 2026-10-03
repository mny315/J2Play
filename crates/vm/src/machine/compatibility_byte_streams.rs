//! Platform byte-array stream buffers and cursor semantics.

use super::{
    Allocation, ArrayKind, EmuError, Handle, HeapValue, Machine, Value, heap_error, int_argument,
    reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(super) fn byte_array_output_ensure_capacity(
        &mut self,
        stream: Handle,
        required: usize,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let buffer =
            self.graphics_reference_field(stream, "java/io/ByteArrayOutputStream.buf:[B")?;
        let old_length = self.heap.managed.array_length(buffer).map_err(heap_error)?;
        if required <= old_length {
            return Ok(buffer);
        }
        let count = usize::try_from(
            self.graphics_int_field(stream, "java/io/ByteArrayOutputStream.count:I")?,
        )
        .map_err(|_| type_error())?;
        let new_length = required.max(old_length.saturating_mul(2).saturating_add(1));
        let new_length_i32 = i32::try_from(new_length)
            .map_err(|_| vm_error("out-of-memory-error", "byte buffer is too large"))?;
        let new_buffer = self.allocate_array(ArrayKind::Byte, new_length_i32, &[], roots)?;
        let (
            Allocation::Array {
                kind: ArrayKind::Byte,
                elements: source,
            },
            Allocation::Array {
                kind: ArrayKind::Byte,
                elements: destination,
            },
        ) = self
            .heap
            .managed
            .get_pair_mut(buffer, new_buffer)
            .map_err(heap_error)?
        else {
            return Err(type_error());
        };
        destination[..count].copy_from_slice(source.get(..count).ok_or_else(type_error)?);
        self.heap
            .managed
            .set_field(
                stream,
                "java/io/ByteArrayOutputStream.buf:[B",
                HeapValue::Reference(Some(new_buffer)),
            )
            .map_err(heap_error)?;
        Ok(new_buffer)
    }

    pub(super) fn byte_array_output_write_byte(&mut self, args: &[Value]) -> Result<(), EmuError> {
        let stream = reference_argument(args, 0)?;
        let count = usize::try_from(
            self.graphics_int_field(stream, "java/io/ByteArrayOutputStream.count:I")?,
        )
        .map_err(|_| type_error())?;
        let required = count
            .checked_add(1)
            .ok_or_else(|| vm_error("out-of-memory-error", "byte buffer size overflow"))?;
        let buffer = self.byte_array_output_ensure_capacity(stream, required, args)?;
        self.heap
            .managed
            .array_set(
                buffer,
                i32::try_from(count).map_err(|_| type_error())?,
                HeapValue::Int(int_argument(args, 1)?),
            )
            .map_err(heap_error)?;
        self.heap
            .managed
            .set_field(
                stream,
                "java/io/ByteArrayOutputStream.count:I",
                HeapValue::Int(i32::try_from(required).map_err(|_| type_error())?),
            )
            .map_err(heap_error)
    }

    pub(super) fn byte_array_output_write_slice(&mut self, args: &[Value]) -> Result<(), EmuError> {
        let stream = reference_argument(args, 0)?;
        let source = reference_argument(args, 1)?;
        let offset = usize::try_from(int_argument(args, 2)?).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative byte source offset",
            )
        })?;
        let length = usize::try_from(int_argument(args, 3)?).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative byte source length",
            )
        })?;
        let end = offset.checked_add(length).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "byte source range overflow",
            )
        })?;
        {
            let Allocation::Array {
                kind: ArrayKind::Byte,
                elements,
            } = self.heap.managed.get(source).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            if elements.get(offset..end).is_none() {
                return Err(vm_error(
                    "array-index-out-of-bounds-exception",
                    "byte source range is outside the array",
                ));
            }
        }
        let count = usize::try_from(
            self.graphics_int_field(stream, "java/io/ByteArrayOutputStream.count:I")?,
        )
        .map_err(|_| type_error())?;
        let required = count
            .checked_add(length)
            .ok_or_else(|| vm_error("out-of-memory-error", "byte buffer size overflow"))?;
        let buffer = self.byte_array_output_ensure_capacity(stream, required, args)?;
        if source == buffer {
            let Allocation::Array {
                kind: ArrayKind::Byte,
                elements,
            } = self.heap.managed.get_mut(buffer).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            elements.copy_within(offset..end, count);
        } else {
            let (
                Allocation::Array {
                    kind: ArrayKind::Byte,
                    elements: source,
                },
                Allocation::Array {
                    kind: ArrayKind::Byte,
                    elements: destination,
                },
            ) = self
                .heap
                .managed
                .get_pair_mut(source, buffer)
                .map_err(heap_error)?
            else {
                return Err(type_error());
            };
            destination[count..required].copy_from_slice(&source[offset..end]);
        }
        self.heap
            .managed
            .set_field(
                stream,
                "java/io/ByteArrayOutputStream.count:I",
                HeapValue::Int(i32::try_from(required).map_err(|_| type_error())?),
            )
            .map_err(heap_error)
    }

    pub(super) fn byte_array_input_read_byte(&mut self, args: &[Value]) -> Result<i32, EmuError> {
        let stream = reference_argument(args, 0)?;
        let position = self.graphics_int_field(stream, "java/io/ByteArrayInputStream.pos:I")?;
        let count = self.graphics_int_field(stream, "java/io/ByteArrayInputStream.count:I")?;
        if position >= count {
            return Ok(-1);
        }
        let buffer = self
            .heap
            .managed
            .field(stream, "java/io/ByteArrayInputStream.buf:[B")
            .map_err(heap_error)?;
        // The bootstrap evaluates buf[pos++] before baload checks null/bounds.
        // A subclass can mutate these protected fields; preserve that cursor
        // side effect even when the access throws.
        self.heap
            .managed
            .set_field(
                stream,
                "java/io/ByteArrayInputStream.pos:I",
                HeapValue::Int(position.wrapping_add(1)),
            )
            .map_err(heap_error)?;
        let buffer = reference_argument(&[buffer], 0)?;
        let value = self
            .heap
            .managed
            .array_get_typed(buffer, position, super::typed_array_access(0x33)?)
            .map_err(super::array_opcode_error)?;
        let HeapValue::Int(value) = value else {
            return Err(type_error());
        };
        Ok(value & 255)
    }

    pub(super) fn byte_array_input_read_slice(&mut self, args: &[Value]) -> Result<i32, EmuError> {
        let stream = reference_argument(args, 0)?;
        let destination = reference_argument(args, 1)?;
        let offset = usize::try_from(int_argument(args, 2)?).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative byte destination offset",
            )
        })?;
        let requested = usize::try_from(int_argument(args, 3)?).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative byte destination length",
            )
        })?;
        let destination_end = offset.checked_add(requested).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "byte destination range overflow",
            )
        })?;
        if destination_end
            > self
                .heap
                .managed
                .array_length(destination)
                .map_err(heap_error)?
        {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "byte destination range is outside the array",
            ));
        }
        let position = self.graphics_int_field(stream, "java/io/ByteArrayInputStream.pos:I")?;
        let count = self.graphics_int_field(stream, "java/io/ByteArrayInputStream.count:I")?;
        if position >= count {
            return Ok(-1);
        }
        let copied = i32::try_from(requested)
            .map_err(|_| type_error())?
            .min(count.wrapping_sub(position));
        let buffer = self
            .heap
            .managed
            .field(stream, "java/io/ByteArrayInputStream.buf:[B")
            .map_err(heap_error)?;
        let buffer = reference_argument(&[buffer], 0)?;
        let bounds_error = || {
            vm_error(
                "array-index-out-of-bounds-exception",
                "byte source range is outside the array",
            )
        };
        let start = usize::try_from(position).map_err(|_| bounds_error())?;
        let length = usize::try_from(copied).map_err(|_| bounds_error())?;
        let end = start.checked_add(length).ok_or_else(bounds_error)?;
        // Destination bounds were checked before EOF. Validate the protected
        // source state even for an empty read, then copy without a snapshot.
        if buffer == destination {
            let Allocation::Array {
                kind: ArrayKind::Byte,
                elements,
            } = self.heap.managed.get_mut(buffer).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            if elements.get(start..end).is_none() {
                return Err(bounds_error());
            }
            elements.copy_within(start..end, offset);
        } else {
            let (
                Allocation::Array {
                    kind: ArrayKind::Byte,
                    elements: source,
                },
                Allocation::Array {
                    kind: ArrayKind::Byte,
                    elements: destination,
                },
            ) = self
                .heap
                .managed
                .get_pair_mut(buffer, destination)
                .map_err(heap_error)?
            else {
                return Err(type_error());
            };
            destination[offset..offset + length]
                .copy_from_slice(source.get(start..end).ok_or_else(bounds_error)?);
        }
        self.heap
            .managed
            .set_field(
                stream,
                "java/io/ByteArrayInputStream.pos:I",
                HeapValue::Int(position.wrapping_add(copied)),
            )
            .map_err(heap_error)?;
        Ok(copied)
    }

    pub(super) fn byte_array_input_read_is_unmodified(
        &self,
        input: Handle,
        descriptor: &str,
    ) -> Result<bool, EmuError> {
        let class = self.object_class(input)?;
        if !self.is_instance(&class, "java/io/ByteArrayInputStream") {
            return Ok(false);
        }
        // A subclass may replace read() while retaining the inherited buf,
        // pos and count fields. Only the resolved built-in implementation is
        // equivalent to reading those fields directly.
        Ok(self.resolve_virtual(&class, "read", descriptor)?.class
            == "java/io/ByteArrayInputStream")
    }
}
