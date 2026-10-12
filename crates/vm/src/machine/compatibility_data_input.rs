//! `DataInput` primitive reads and resumable modified UTF-8 decoding.

use super::{
    Allocation, ArrayKind, CallOutcome, EmuError, Handle, HashMap, HeapValue, Machine, Method,
    NativeResume, SuspendedCall, Value, check_intrinsic_cancellation, data_input_utf_byte,
    data_input_utf_length, decode_data_input_utf, heap_error, reference_argument,
    suspended_native_pending_call, type_error,
};

impl Machine<'_, '_> {
    pub(super) fn data_input_stream_byte(
        &mut self,
        args: &[Value],
        required: bool,
    ) -> Result<Option<CallOutcome>, EmuError> {
        let stream = reference_argument(args, 0)?;
        let input = self
            .graphics_reference_field(stream, "java/io/DataInputStream.in:Ljava/io/InputStream;")?;
        if !self.byte_array_input_read_is_unmodified(input, "()I")? {
            return Ok(None);
        }
        let value = self.byte_array_input_read_byte(&[Value::Reference(Some(input))])?;
        if required && value < 0 {
            return Ok(Some(self.thread_exception("java/io/EOFException", None)?));
        }
        Ok(Some(CallOutcome::Return(Some(Value::Int(value)))))
    }

    pub(super) fn data_input_stream_primitive(
        &mut self,
        args: &[Value],
        width: usize,
        signed: bool,
    ) -> Result<Option<CallOutcome>, EmuError> {
        let stream = reference_argument(args, 0)?;
        let input = self
            .graphics_reference_field(stream, "java/io/DataInputStream.in:Ljava/io/InputStream;")?;
        if !self.byte_array_input_read_is_unmodified(input, "()I")? {
            return Ok(None);
        }
        let bits = if width > 1
            && let Some(bits) = self.try_byte_array_input_word(input, width)?
        {
            bits
        } else {
            let mut bits = 0_u64;
            for _ in 0..width {
                let byte = self.byte_array_input_read_byte(&[Value::Reference(Some(input))])?;
                if byte < 0 {
                    return Ok(Some(self.thread_exception("java/io/EOFException", None)?));
                }
                bits = (bits << 8) | u64::from(byte.cast_unsigned());
            }
            bits
        };
        let value = if width == 8 {
            Value::Long(bits as i64)
        } else {
            let shift = if signed { 64 - width * 8 } else { 0 };
            let value = if signed {
                ((bits << shift) as i64 >> shift) as i32
            } else {
                bits as i32
            };
            Value::Int(value)
        };
        Ok(Some(CallOutcome::Return(Some(value))))
    }

    fn try_byte_array_input_word(
        &mut self,
        input: Handle,
        width: usize,
    ) -> Result<Option<u64>, EmuError> {
        // Only batch a complete, valid read. EOF and altered protected fields
        // keep the bytewise path's exception order and partial cursor updates.
        let word = (|| {
            let position = self
                .graphics_int_field(input, "java/io/ByteArrayInputStream.pos:I")
                .ok()?;
            let count = self
                .graphics_int_field(input, "java/io/ByteArrayInputStream.count:I")
                .ok()?;
            let end = position.checked_add(i32::try_from(width).ok()?)?;
            if end > count {
                return None;
            }
            let buffer = self
                .graphics_reference_field(input, "java/io/ByteArrayInputStream.buf:[B")
                .ok()?;
            let Allocation::Array {
                kind: ArrayKind::Byte,
                elements,
            } = self.heap.managed.get(buffer).ok()?
            else {
                return None;
            };
            let bytes =
                elements.get(usize::try_from(position).ok()?..usize::try_from(end).ok()?)?;
            let bits = bytes.iter().try_fold(0_u64, |bits, byte| {
                let HeapValue::Int(byte) = byte else {
                    return None;
                };
                Some((bits << 8) | u64::from(byte.cast_unsigned() & 255))
            })?;
            Some((end, bits))
        })();
        let Some((end, bits)) = word else {
            return Ok(None);
        };
        self.heap
            .managed
            .set_field(input, "java/io/ByteArrayInputStream.pos:I", Value::Int(end))
            .map_err(heap_error)?;
        Ok(Some(bits))
    }

    pub(super) fn data_input_read_utf(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let input = reference_argument(args, 0)?;
        self.continue_data_input_read_utf(method, input, None, Vec::new(), depth)
    }

    pub(super) fn continue_data_input_read_utf(
        &mut self,
        method: &Method,
        input: Handle,
        mut length: Option<usize>,
        mut bytes: Vec<u8>,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        if length.is_none() {
            match self.call_int_virtual(input, "readUnsignedShort", "()I", depth)? {
                Ok(value) => length = Some(data_input_utf_length(value)?),
                Err(CallOutcome::Suspend(child)) => {
                    return Ok(suspended_native_pending_call(
                        method,
                        NativeResume::DataInputReadUtf {
                            input,
                            length,
                            bytes,
                        },
                        child,
                    ));
                }
                Err(outcome) => return Ok(outcome),
            }
        }
        let length = length.expect("readUTF length was initialized above");
        bytes.reserve(length.saturating_sub(bytes.len()));
        while bytes.len() < length {
            // Intrinsic byte reads do not enter the interpreter's polling loop.
            if bytes.len().is_multiple_of(1_024) {
                check_intrinsic_cancellation(self.native_context)?;
            }
            match self.call_int_virtual(input, "readUnsignedByte", "()I", depth)? {
                Ok(value) => bytes.push(data_input_utf_byte(value)?),
                Err(CallOutcome::Suspend(child)) => {
                    return Ok(suspended_native_pending_call(
                        method,
                        NativeResume::DataInputReadUtf {
                            input,
                            length: Some(length),
                            bytes,
                        },
                        child,
                    ));
                }
                Err(outcome) => return Ok(outcome),
            }
        }
        let units = match decode_data_input_utf(&bytes) {
            Ok(units) => units,
            Err(()) => return self.thread_exception("java/io/UTFDataFormatException", None),
        };
        let roots = [Value::Reference(Some(input))];
        let string = self.allocate_object("java/lang/String", HashMap::new(), &[], &roots)?;
        self.store_string_units(string, units, 1, &[], &roots)?;
        Ok(CallOutcome::Return(Some(Value::Reference(Some(string)))))
    }

    pub(super) fn resume_data_input_read_utf(
        &mut self,
        method: &Method,
        input: Handle,
        mut length: Option<usize>,
        mut bytes: Vec<u8>,
        child: Box<SuspendedCall>,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        match self.resume_suspended_call(child, depth + 1)? {
            CallOutcome::Return(Some(Value::Int(value))) => {
                if length.is_none() {
                    length = Some(data_input_utf_length(value)?);
                } else {
                    bytes.push(data_input_utf_byte(value)?);
                }
                self.continue_data_input_read_utf(method, input, length, bytes, depth)
            }
            CallOutcome::Return(_) => Err(type_error()),
            CallOutcome::Throw(handle) => Ok(CallOutcome::Throw(handle)),
            CallOutcome::Suspend(child) => Ok(suspended_native_pending_call(
                method,
                NativeResume::DataInputReadUtf {
                    input,
                    length,
                    bytes,
                },
                child,
            )),
        }
    }
}
