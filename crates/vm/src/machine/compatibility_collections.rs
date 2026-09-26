use super::{
    Allocation, ArrayKind, EmuError, Handle, HeapError, HeapValue, Machine, Value,
    check_intrinsic_cancellation, heap_error, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(super) fn vector_element_at(&self, vector: Handle, index: i32) -> Result<Value, EmuError> {
        let count = self.graphics_int_field(vector, "java/util/Vector.elementCount:I")?;
        if index < 0 || index >= count {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                format!("Vector index {index} outside 0..{count}"),
            ));
        }
        let data = self
            .graphics_reference_field(vector, "java/util/Vector.elementData:[Ljava/lang/Object;")?;
        self.heap.managed.array_get(data, index).map_err(heap_error)
    }

    pub(super) fn hashtable_find(&self, args: &[Value]) -> Result<Option<i32>, EmuError> {
        let table = reference_argument(args, 0)?;
        let Some(Value::Reference(Some(key))) = args.get(1) else {
            // Let the Java implementation produce its specified
            // NullPointerException for a null key.
            return Ok(None);
        };
        let count = usize::try_from(self.graphics_int_field(table, "java/util/Hashtable.count:I")?)
            .map_err(|_| type_error())?;
        let keys =
            self.graphics_reference_field(table, "java/util/Hashtable.keys:[Ljava/lang/Object;")?;
        let Allocation::Array {
            kind: ArrayKind::Reference(_),
            elements,
        } = self.heap.managed.get(keys).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if count > elements.len() {
            return Err(type_error());
        }
        let mut prepared = None;
        for (index, value) in elements[..count].iter().enumerate() {
            if index.is_multiple_of(1_024) {
                check_intrinsic_cancellation(self.native_context)?;
            }
            let HeapValue::Reference(Some(candidate)) = value else {
                return Err(type_error());
            };
            if key == candidate {
                return Ok(Some(i32::try_from(index).map_err(|_| type_error())?));
            }
            // Prepare only when equality is needed: empty tables and object
            // identity must not inspect a key's class or payload.
            let prepared = if let Some(ref prepared) = prepared {
                prepared
            } else {
                let Some(value) = StandardKey::prepare(self, *key)? else {
                    return Ok(None);
                };
                prepared.insert(value)
            };
            if prepared.equals(self, *candidate)? {
                return Ok(Some(i32::try_from(index).map_err(|_| type_error())?));
            }
        }
        Ok(Some(-1))
    }
}

/// Data shared by every comparison in one read-only Hashtable search.
enum StandardKey<'a> {
    String(&'a [u16]),
    Boxed {
        class: &'a str,
        descriptor: &'static str,
        value: Option<&'a HeapValue>,
    },
}

impl<'a> StandardKey<'a> {
    // This helper has one caller; avoid copying the prepared enum through a
    // separate return slot for every lookup, including one-element tables.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn prepare(machine: &'a Machine<'_, '_>, key: Handle) -> Result<Option<Self>, EmuError> {
        let Allocation::Object { class, fields } =
            machine.heap.managed.get(key).map_err(heap_error)?
        else {
            return Ok(None);
        };
        if class.as_ref() == "java/lang/String" {
            let value = machine
                .heap
                .string_values
                .get(&key)
                .ok_or_else(type_error)?;
            return Ok(Some(Self::String(value)));
        }
        let descriptor = match class.as_ref() {
            "java/lang/Byte" => "java/lang/Byte.value:B",
            "java/lang/Character" => "java/lang/Character.value:C",
            "java/lang/Integer" => "java/lang/Integer.value:I",
            "java/lang/Short" => "java/lang/Short.value:S",
            _ => return Ok(None),
        };
        Ok(Some(Self::Boxed {
            class,
            descriptor,
            // Keep malformed-field errors deferred until the candidate has
            // the same class, just as in a standalone equality comparison.
            value: fields.get(descriptor),
        }))
    }

    fn equals(&self, machine: &Machine<'_, '_>, candidate: Handle) -> Result<bool, EmuError> {
        match self {
            Self::String(value) => {
                let Some(candidate_value) = machine.heap.string_values.get(&candidate) else {
                    return Ok(false);
                };
                if value.len() != candidate_value.len() {
                    return Ok(false);
                }
                for (index, (left, right)) in value
                    .chunks(1_024)
                    .zip(candidate_value.chunks(1_024))
                    .enumerate()
                {
                    if index != 0 {
                        check_intrinsic_cancellation(machine.native_context)?;
                    }
                    if left != right {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Self::Boxed {
                class,
                descriptor,
                value,
            } => {
                let Allocation::Object {
                    class: candidate_class,
                    fields,
                } = machine.heap.managed.get(candidate).map_err(heap_error)?
                else {
                    return Ok(false);
                };
                if candidate_class.as_ref() != *class {
                    return Ok(false);
                }
                let HeapValue::Int(value) = value
                    .copied()
                    .ok_or(HeapError::TypeMismatch)
                    .map_err(heap_error)?
                else {
                    return Err(type_error());
                };
                let HeapValue::Int(candidate_value) = fields
                    .get(descriptor)
                    .copied()
                    .ok_or(HeapError::TypeMismatch)
                    .map_err(heap_error)?
                else {
                    return Err(type_error());
                };
                Ok(value == candidate_value)
            }
        }
    }
}
