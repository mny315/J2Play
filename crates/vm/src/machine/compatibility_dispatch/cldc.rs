//! Intrinsics for standard CLDC strings, numeric wrappers, streams and collections.

use crate::machine::{
    CallOutcome, EmuError, HeapValue, Machine, Method, Value, heap_error, int_argument,
    long_argument, optional_reference_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(super) fn invoke_cldc_intrinsic(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Result<Option<CallOutcome>, EmuError> {
        if !method.is_static && method.key.class == "java/lang/String" {
            let receiver = reference_argument(args, 0)?;
            match (method.key.name.as_str(), method.key.descriptor.as_str()) {
                ("length", "()I") => {
                    let length = self
                        .heap
                        .string_values
                        .get(&receiver)
                        .ok_or_else(type_error)?
                        .len();
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(
                        i32::try_from(length).map_err(|_| type_error())?,
                    )))));
                }
                ("charAt", "(I)C") => {
                    let index = usize::try_from(int_argument(args, 1)?)
                        .map_err(|_| vm_error("string-index", "negative String index"))?;
                    let value = self
                        .heap
                        .string_values
                        .get(&receiver)
                        .ok_or_else(type_error)?
                        .get(index)
                        .copied()
                        .ok_or_else(|| vm_error("string-index", "String index is out of bounds"))?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(i32::from(
                        value,
                    ))))));
                }
                ("equals", "(Ljava/lang/Object;)Z") => {
                    let equal = match args.get(1) {
                        Some(Value::Reference(Some(other))) => {
                            match self
                                .heap
                                .string_values
                                .get(&receiver)
                                .zip(self.heap.string_values.get(other))
                            {
                                Some((left, right)) => {
                                    self.string_units_equal(left, right, false)?
                                }
                                None => false,
                            }
                        }
                        Some(Value::Reference(None)) => false,
                        _ => return Err(type_error()),
                    };
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(i32::from(
                        equal,
                    ))))));
                }
                ("equalsIgnoreCase", "(Ljava/lang/String;)Z") => {
                    let other = optional_reference_argument(args, 1)?;
                    let equal = self.string_equals_ignore_case(receiver, other)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(i32::from(
                        equal,
                    ))))));
                }
                ("hashCode", "()I") => {
                    let hash = self.string_hash_code(receiver)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(hash)))));
                }
                ("compareTo", "(Ljava/lang/String;)I") => {
                    let other = reference_argument(args, 1)?;
                    let difference = self.string_compare_to(receiver, other)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(difference)))));
                }
                ("indexOf", "(I)I" | "(II)I") => {
                    let needle = int_argument(args, 1)?;
                    let from = if args.len() == 3 {
                        int_argument(args, 2)?
                    } else {
                        0
                    };
                    let index = self.string_index_of_char(receiver, needle, from)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(index)))));
                }
                ("getChars", "(II[CI)V") => {
                    self.string_get_chars_intrinsic(args)?;
                    return Ok(Some(CallOutcome::Return(None)));
                }
                ("startsWith", "(Ljava/lang/String;I)Z") => {
                    let prefix = reference_argument(args, 1)?;
                    let offset = int_argument(args, 2)?;
                    let result = self.string_starts_with(receiver, prefix, offset)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(i32::from(
                        result,
                    ))))));
                }
                ("startsWith", "(Ljava/lang/String;)Z") => {
                    let prefix = reference_argument(args, 1)?;
                    let result = self.string_starts_with(receiver, prefix, 0)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(i32::from(
                        result,
                    ))))));
                }
                ("indexOf", "(Ljava/lang/String;I)I") => {
                    let needle = reference_argument(args, 1)?;
                    let result = self.string_index_of(receiver, needle, int_argument(args, 2)?)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(result)))));
                }
                ("indexOf", "(Ljava/lang/String;)I") => {
                    let needle = reference_argument(args, 1)?;
                    let result = self.string_index_of(receiver, needle, 0)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(result)))));
                }
                ("toCharArray", "()[C") => {
                    let array = self.string_to_char_array(receiver, args)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Reference(Some(
                        array,
                    ))))));
                }
                ("substring", "(I)Ljava/lang/String;")
                | ("substring", "(II)Ljava/lang/String;") => {
                    let begin = int_argument(args, 1)?;
                    let end = if args.len() == 3 {
                        int_argument(args, 2)?
                    } else {
                        i32::try_from(
                            self.heap
                                .string_values
                                .get(&receiver)
                                .ok_or_else(type_error)?
                                .len(),
                        )
                        .map_err(|_| type_error())?
                    };
                    let string = self.string_substring(receiver, begin, end, args)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Reference(Some(
                        string,
                    ))))));
                }
                ("toLowerCase", "()Ljava/lang/String;") => {
                    let string = self.string_to_lower_case(receiver, args)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Reference(Some(
                        string,
                    ))))));
                }
                ("toUpperCase", "()Ljava/lang/String;") => {
                    let string = self.string_to_upper_case(receiver, args)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Reference(Some(
                        string,
                    ))))));
                }
                ("toString", "()Ljava/lang/String;") => {
                    return Ok(Some(CallOutcome::Return(Some(Value::Reference(Some(
                        receiver,
                    ))))));
                }
                _ => {}
            }
        }

        if method.is_static
            && method.key.class == "java/lang/Integer"
            && method.key.name == "parseInt"
            && matches!(
                method.key.descriptor.as_str(),
                "(Ljava/lang/String;)I" | "(Ljava/lang/String;I)I"
            )
        {
            let string = optional_reference_argument(args, 0)?;
            let radix = if args.len() == 2 {
                int_argument(args, 1)?
            } else {
                10
            };
            return Ok(Some(CallOutcome::Return(Some(Value::Int(
                self.integer_parse_int(string, radix)?,
            )))));
        }

        if method.is_static && method.key.class == "java/lang/Math" && method.key.name == "abs" {
            let value = match method.key.descriptor.as_str() {
                "(I)I" => Value::Int(int_argument(args, 0)?.wrapping_abs()),
                "(J)J" => Value::Long(long_argument(args, 0)?.wrapping_abs()),
                "(F)F" => match args.first() {
                    Some(Value::Float(value)) => Value::Float(value.abs()),
                    _ => return Err(type_error()),
                },
                "(D)D" => match args.first() {
                    Some(Value::Double(value)) => Value::Double(value.abs()),
                    _ => return Err(type_error()),
                },
                _ => return Ok(None),
            };
            return Ok(Some(CallOutcome::Return(Some(value))));
        }

        if !method.is_static
            && matches!(
                method.key.class.as_str(),
                "java/lang/Byte" | "java/lang/Character" | "java/lang/Integer" | "java/lang/Short"
            )
            && method.key.name == "equals"
            && method.key.descriptor == "(Ljava/lang/Object;)Z"
        {
            let receiver = reference_argument(args, 0)?;
            let equal = match args.get(1) {
                Some(Value::Reference(Some(other)))
                    if self.object_class(*other)? == method.key.class =>
                {
                    let descriptor = match method.key.class.as_str() {
                        "java/lang/Byte" => "java/lang/Byte.value:B",
                        "java/lang/Character" => "java/lang/Character.value:C",
                        "java/lang/Integer" => "java/lang/Integer.value:I",
                        "java/lang/Short" => "java/lang/Short.value:S",
                        _ => return Ok(None),
                    };
                    self.graphics_int_field(receiver, descriptor)?
                        == self.graphics_int_field(*other, descriptor)?
                }
                Some(Value::Reference(_)) => false,
                _ => return Err(type_error()),
            };
            return Ok(Some(CallOutcome::Return(Some(Value::Int(i32::from(
                equal,
            ))))));
        }

        if !method.is_static
            && method.key.class == "java/io/ByteArrayOutputStream"
            && method.key.name == "write"
            && method.key.descriptor == "(I)V"
        {
            self.byte_array_output_write_byte(args)?;
            return Ok(Some(CallOutcome::Return(None)));
        }
        if !method.is_static
            && method.key.class == "java/io/ByteArrayInputStream"
            && method.key.name == "read"
        {
            if method.key.descriptor == "()I" {
                return Ok(Some(CallOutcome::Return(Some(Value::Int(
                    self.byte_array_input_read_byte(args)?,
                )))));
            }
            if method.key.descriptor == "([BII)I" {
                return Ok(Some(CallOutcome::Return(Some(Value::Int(
                    self.byte_array_input_read_slice(args)?,
                )))));
            }
        }
        if !method.is_static && method.key.class == "java/io/DataInputStream" {
            if method.key.name == "read" && method.key.descriptor == "()I" {
                if let Some(outcome) = self.data_input_stream_byte(args, false)? {
                    return Ok(Some(outcome));
                }
            } else if method.key.name == "required"
                && method.key.descriptor == "()I"
                && let Some(outcome) = self.data_input_stream_byte(args, true)?
            {
                return Ok(Some(outcome));
            } else {
                let primitive = match (method.key.name.as_str(), method.key.descriptor.as_str()) {
                    ("readBoolean", "()Z") | ("readUnsignedByte", "()I") => Some((1, false)),
                    ("readByte", "()B") => Some((1, true)),
                    ("readUnsignedShort", "()I") | ("readChar", "()C") => Some((2, false)),
                    ("readShort", "()S") => Some((2, true)),
                    ("readInt", "()I") => Some((4, true)),
                    ("readLong", "()J") => Some((8, true)),
                    _ => None,
                };
                if let Some((width, signed)) = primitive
                    && let Some(mut outcome) =
                        self.data_input_stream_primitive(args, width, signed)?
                {
                    if method.key.name == "readBoolean"
                        && let CallOutcome::Return(Some(Value::Int(value))) = &mut outcome
                    {
                        *value = i32::from(*value != 0);
                    }
                    return Ok(Some(outcome));
                }
            }
        }

        if !method.is_static && method.key.class == "java/lang/StringBuffer" {
            let buffer = reference_argument(args, 0)?;
            match (method.key.name.as_str(), method.key.descriptor.as_str()) {
                ("ensureCapacity", "(I)V") => {
                    if let Ok(minimum) = usize::try_from(int_argument(args, 1)?) {
                        let state = self.string_buffer_state(buffer)?;
                        self.string_buffer_ensure_capacity(buffer, &state, minimum, args)?;
                    }
                    return Ok(Some(CallOutcome::Return(None)));
                }
                ("append", "(C)Ljava/lang/StringBuffer;") => {
                    let buffer = self.string_buffer_append_char(args)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Reference(Some(
                        buffer,
                    ))))));
                }
                ("append", "(Ljava/lang/String;)Ljava/lang/StringBuffer;") => {
                    let buffer = self.string_buffer_append_string(args)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Reference(Some(
                        buffer,
                    ))))));
                }
                ("length", "()I") => {
                    let count = self.string_buffer_state(buffer)?.count;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(
                        i32::try_from(count).map_err(|_| type_error())?,
                    )))));
                }
                ("charAt", "(I)C") => {
                    let index = int_argument(args, 1)?;
                    let state = self.string_buffer_state(buffer)?;
                    let index_usize = usize::try_from(index)
                        .map_err(|_| vm_error("string-index", "negative StringBuffer index"))?;
                    if index_usize >= state.count {
                        return Err(vm_error(
                            "string-index",
                            "StringBuffer index is out of bounds",
                        ));
                    }
                    let HeapValue::Int(value) = self
                        .heap
                        .managed
                        .array_get(state.value, index)
                        .map_err(heap_error)?
                    else {
                        return Err(type_error());
                    };
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(value & 0xffff)))));
                }
                ("toString", "()Ljava/lang/String;") => {
                    let string = self.string_buffer_to_string(buffer, args)?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Reference(Some(
                        string,
                    ))))));
                }
                _ => {}
            }
        }

        if !method.is_static && method.key.class == "java/util/Vector" {
            let vector = reference_argument(args, 0)?;
            match (method.key.name.as_str(), method.key.descriptor.as_str()) {
                ("size", "()I") => {
                    let count =
                        self.graphics_int_field(vector, "java/util/Vector.elementCount:I")?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(count)))));
                }
                ("isEmpty", "()Z") => {
                    let count =
                        self.graphics_int_field(vector, "java/util/Vector.elementCount:I")?;
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(i32::from(
                        count == 0,
                    ))))));
                }
                ("elementAt", "(I)Ljava/lang/Object;") => {
                    let value = self.vector_element_at(vector, int_argument(args, 1)?)?;
                    return Ok(Some(CallOutcome::Return(Some(value))));
                }
                _ => {}
            }
        }

        if !method.is_static
            && method.key.class == "java/util/Hashtable"
            && method.key.name == "find"
            && method.key.descriptor == "(Ljava/lang/Object;)I"
            && let Some(index) = self.hashtable_find(args)?
        {
            return Ok(Some(CallOutcome::Return(Some(Value::Int(index)))));
        }

        Ok(None)
    }
}
