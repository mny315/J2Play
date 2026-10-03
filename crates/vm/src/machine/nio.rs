//! NIO buffer views and bounded transfers shared with JSR-239.

use super::{
    Allocation, ArrayKind, CallOutcome, EmuError, Handle, HeapValue, Machine, Method, Value,
    display_key, heap_error, int_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(super) fn invoke_nio_native(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let signature = (
            method.key.class.as_str(),
            method.key.name.as_str(),
            method.key.descriptor.as_str(),
        );
        match signature {
            ("java/nio/ByteBuffer", "allocateDirect", "(I)Ljava/nio/ByteBuffer;") => {
                let capacity = int_argument(args, 0)?;
                if capacity < 0 {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("negative direct buffer capacity"),
                    );
                }
                let bytes = self.allocate_array(ArrayKind::Byte, capacity, &[], args)?;
                let buffer = self.allocate_native_instance(
                    "java/nio/ByteBuffer",
                    &[Value::Reference(Some(bytes))],
                )?;
                self.initialize_nio_view(buffer, capacity, bytes, 0)?;
                Ok(CallOutcome::Return(Some(Value::Reference(Some(buffer)))))
            }
            ("java/nio/ByteBuffer", "asFloatBuffer", "()Ljava/nio/FloatBuffer;")
            | ("java/nio/ByteBuffer", "asIntBuffer", "()Ljava/nio/IntBuffer;") => {
                let byte_buffer = reference_argument(args, 0)?;
                let (bytes, range) = self.nio_buffer_range(byte_buffer, 1)?;
                let capacity = i32::try_from(range.len() / 4).map_err(|_| type_error())?;
                let offset = i32::try_from(range.start).map_err(|_| type_error())?;
                let class = if method.key.name == "asFloatBuffer" {
                    "java/nio/FloatBuffer"
                } else {
                    "java/nio/IntBuffer"
                };
                let buffer = self.allocate_native_instance(class, args)?;
                self.initialize_nio_view(buffer, capacity, bytes, offset)?;
                Ok(CallOutcome::Return(Some(Value::Reference(Some(buffer)))))
            }
            ("java/nio/FloatBuffer", "put", "([F)Ljava/nio/FloatBuffer;")
            | ("java/nio/IntBuffer", "put", "([I)Ljava/nio/IntBuffer;") => {
                let buffer = reference_argument(args, 0)?;
                let source = reference_argument(args, 1)?;
                let kind = if signature.0 == "java/nio/FloatBuffer" {
                    ArrayKind::Float
                } else {
                    ArrayKind::Int
                };
                self.nio_put_values(buffer, source, &kind)?;
                Ok(CallOutcome::Return(Some(Value::Reference(Some(buffer)))))
            }
            ("java/nio/Buffer", "remaining", "()I") => {
                let buffer = reference_argument(args, 0)?;
                let remaining = self
                    .graphics_int_field(buffer, "java/nio/Buffer.capacity:I")?
                    .saturating_sub(self.graphics_int_field(buffer, "java/nio/Buffer.position:I")?);
                Ok(CallOutcome::Return(Some(Value::Int(remaining))))
            }
            ("java/nio/Buffer", "rewind", "()Ljava/nio/Buffer;") => {
                let buffer = reference_argument(args, 0)?;
                self.heap
                    .managed
                    .set_field(buffer, "java/nio/Buffer.position:I", HeapValue::Int(0))
                    .map_err(heap_error)?;
                Ok(CallOutcome::Return(Some(Value::Reference(Some(buffer)))))
            }
            _ => Err(vm_error("method-not-found", display_key(&method.key))),
        }
    }

    fn initialize_nio_view(
        &mut self,
        buffer: Handle,
        capacity: i32,
        bytes: Handle,
        byte_offset: i32,
    ) -> Result<(), EmuError> {
        self.heap
            .managed
            .set_field(
                buffer,
                "java/nio/Buffer.capacity:I",
                HeapValue::Int(capacity),
            )
            .map_err(heap_error)?;
        self.heap
            .managed
            .set_field(buffer, "java/nio/Buffer.position:I", HeapValue::Int(0))
            .map_err(heap_error)?;
        self.heap
            .managed
            .set_field(
                buffer,
                "java/nio/Buffer.byteOffset:I",
                HeapValue::Int(byte_offset),
            )
            .map_err(heap_error)?;
        self.heap
            .managed
            .set_field(
                buffer,
                "java/nio/Buffer.bytes:[B",
                HeapValue::Reference(Some(bytes)),
            )
            .map_err(heap_error)
    }

    pub(super) fn nio_buffer_range(
        &self,
        buffer: Handle,
        element_size: usize,
    ) -> Result<(Handle, std::ops::Range<usize>), EmuError> {
        let capacity =
            usize::try_from(self.graphics_int_field(buffer, "java/nio/Buffer.capacity:I")?)
                .map_err(|_| type_error())?;
        let position =
            usize::try_from(self.graphics_int_field(buffer, "java/nio/Buffer.position:I")?)
                .map_err(|_| type_error())?;
        let offset =
            usize::try_from(self.graphics_int_field(buffer, "java/nio/Buffer.byteOffset:I")?)
                .map_err(|_| type_error())?;
        let bytes = self.graphics_reference_field(buffer, "java/nio/Buffer.bytes:[B")?;
        let byte_index = |index: usize| {
            index
                .checked_mul(element_size)
                .and_then(|index| offset.checked_add(index))
                .ok_or_else(type_error)
        };
        let start = byte_index(position)?;
        let end = byte_index(capacity)?;
        if position > capacity || end > self.heap.managed.array_length(bytes).map_err(heap_error)? {
            return Err(vm_error(
                "illegal-argument-exception",
                "invalid NIO buffer range",
            ));
        }
        if self.heap.managed.array_kind(bytes).map_err(heap_error)? != &ArrayKind::Byte {
            return Err(type_error());
        }
        Ok((bytes, start..end))
    }

    pub(super) fn nio_buffer_bytes(
        &self,
        buffer: Handle,
        element_size: usize,
    ) -> Result<&[HeapValue], EmuError> {
        let (bytes, range) = self.nio_buffer_range(buffer, element_size)?;
        let Allocation::Array { elements, .. } =
            self.heap.managed.get(bytes).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        Ok(&elements[range])
    }

    pub(super) fn nio_put_values(
        &mut self,
        buffer: Handle,
        source: Handle,
        kind: &ArrayKind,
    ) -> Result<(), EmuError> {
        if self.heap.managed.array_kind(source).map_err(heap_error)? != kind {
            return Err(type_error());
        }
        let count = self.heap.managed.array_length(source).map_err(heap_error)?;
        let (destination, range) = self.nio_buffer_range(buffer, 4)?;
        if count > range.len() / 4 {
            return Err(vm_error("buffer-overflow", "NIO buffer capacity exceeded"));
        }
        let position = self.graphics_int_field(buffer, "java/nio/Buffer.position:I")?;
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
        for (index, (value, target)) in source
            .iter()
            .zip(destination[range].chunks_exact_mut(4))
            .enumerate()
        {
            if index.is_multiple_of(1_024) && self.native_context.execution_cancelled() {
                return Err(vm_error("execution-cancelled", "NIO buffer copy cancelled"));
            }
            // JSR-239 fixes all typed views to the platform byte order.
            let bytes = match value {
                HeapValue::Int(value) => value.to_ne_bytes(),
                HeapValue::Float(value) => value.to_ne_bytes(),
                _ => return Err(type_error()),
            };
            for (target, byte) in target.iter_mut().zip(bytes) {
                *target = HeapValue::Int(i32::from(byte.cast_signed()));
            }
        }
        self.heap
            .managed
            .set_field(
                buffer,
                "java/nio/Buffer.position:I",
                HeapValue::Int(position + i32::try_from(count).map_err(|_| type_error())?),
            )
            .map_err(heap_error)
    }
}
