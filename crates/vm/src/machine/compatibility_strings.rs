use super::{
    Allocation, ArrayKind, EmuError, Handle, HashMap, HeapValue, Machine, Value,
    check_intrinsic_cancellation, heap_error, int_argument, java_utf16_case_equal,
    reference_argument, simple_case_unit, two_way_utf16_index, type_error, vm_error,
};

const STRING_WORK_CHUNK: usize = 1_024;

impl Machine<'_, '_> {
    pub(super) fn string_units_equal(
        &self,
        left: &[u16],
        right: &[u16],
        ignore_case: bool,
    ) -> Result<bool, EmuError> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (left, right) in left
            .chunks(STRING_WORK_CHUNK)
            .zip(right.chunks(STRING_WORK_CHUNK))
        {
            check_intrinsic_cancellation(self.native_context)?;
            let equal = if ignore_case {
                utf16_equals_ignore_case(left, right)
            } else {
                left == right
            };
            if !equal {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn string_hash_code(&self, string: Handle) -> Result<i32, EmuError> {
        let units = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        let mut hash = 0_i32;
        for chunk in units.chunks(STRING_WORK_CHUNK) {
            check_intrinsic_cancellation(self.native_context)?;
            hash = chunk.iter().fold(hash, |hash, unit| {
                hash.wrapping_mul(31).wrapping_add(i32::from(*unit))
            });
        }
        Ok(hash)
    }

    pub(super) fn string_compare_to(&self, left: Handle, right: Handle) -> Result<i32, EmuError> {
        let left = self.heap.string_values.get(&left).ok_or_else(type_error)?;
        let right = self.heap.string_values.get(&right).ok_or_else(type_error)?;
        for (left, right) in left
            .chunks(STRING_WORK_CHUNK)
            .zip(right.chunks(STRING_WORK_CHUNK))
        {
            check_intrinsic_cancellation(self.native_context)?;
            if let Some(difference) = left.iter().zip(right).find_map(|(left, right)| {
                (left != right).then(|| i32::from(*left) - i32::from(*right))
            }) {
                return Ok(difference);
            }
        }
        Ok(i32::try_from(left.len()).unwrap_or(i32::MAX)
            - i32::try_from(right.len()).unwrap_or(i32::MAX))
    }

    pub(super) fn string_index_of_char(
        &self,
        string: Handle,
        needle: i32,
        from: i32,
    ) -> Result<i32, EmuError> {
        let units = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        let Ok(needle) = u16::try_from(needle) else {
            return Ok(-1);
        };
        let from = usize::try_from(from.max(0)).unwrap_or(usize::MAX);
        let Some(units) = units.get(from..) else {
            return Ok(-1);
        };
        for (index, chunk) in units.chunks(STRING_WORK_CHUNK).enumerate() {
            check_intrinsic_cancellation(self.native_context)?;
            if let Some(offset) = chunk.iter().position(|unit| *unit == needle) {
                return Ok(i32::try_from(from + index * STRING_WORK_CHUNK + offset).unwrap_or(-1));
            }
        }
        Ok(-1)
    }

    pub(super) fn string_equals_ignore_case(
        &self,
        left: Handle,
        right: Option<Handle>,
    ) -> Result<bool, EmuError> {
        let Some(right) = right else {
            return Ok(false);
        };
        let left = self.heap.string_values.get(&left).ok_or_else(type_error)?;
        let right = self.heap.string_values.get(&right).ok_or_else(type_error)?;
        // CLDC compares UTF-16 characters without allocating case-converted
        // Strings or expanding one character into several.
        self.string_units_equal(left, right, true)
    }

    pub(super) fn string_region_matches(&self, args: &[Value]) -> Result<bool, EmuError> {
        let left = reference_argument(args, 0)?;
        let ignore_case = int_argument(args, 1)? != 0;
        let left_offset = int_argument(args, 2)?;
        let right = reference_argument(args, 3)?;
        let right_offset = int_argument(args, 4)?;
        let length = int_argument(args, 5)?;
        if left_offset < 0 || right_offset < 0 {
            return Ok(false);
        }
        let Some(left_value) = self.heap.string_values.get(&left) else {
            return Err(type_error());
        };
        let Some(right_value) = self.heap.string_values.get(&right) else {
            return Err(type_error());
        };
        // CLDC bounds the end positions, not len itself. Negative lengths
        // compare no characters, provided those end positions are in bounds.
        // Widen before adding so malformed offsets cannot wrap into a match.
        if i64::from(left_offset) + i64::from(length) > left_value.len() as i64
            || i64::from(right_offset) + i64::from(length) > right_value.len() as i64
        {
            return Ok(false);
        }
        if length <= 0 {
            return Ok(true);
        }
        let left_offset = left_offset as usize;
        let right_offset = right_offset as usize;
        let length = length as usize;
        let Some(left_region) = left_offset
            .checked_add(length)
            .and_then(|end| left_value.get(left_offset..end))
        else {
            return Ok(false);
        };
        let Some(right_region) = right_offset
            .checked_add(length)
            .and_then(|end| right_value.get(right_offset..end))
        else {
            return Ok(false);
        };
        self.string_units_equal(left_region, right_region, ignore_case)
    }

    pub(super) fn string_get_chars_intrinsic(&mut self, args: &[Value]) -> Result<(), EmuError> {
        let string = reference_argument(args, 0)?;
        let begin = usize::try_from(int_argument(args, 1)?)
            .map_err(|_| vm_error("string-index", "negative String source offset"))?;
        let end = usize::try_from(int_argument(args, 2)?)
            .map_err(|_| vm_error("string-index", "negative String source end"))?;
        let destination = reference_argument(args, 3)?;
        let destination_offset = usize::try_from(int_argument(args, 4)?).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative char destination offset",
            )
        })?;
        let value = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        let source = value
            .get(begin..end)
            .ok_or_else(|| vm_error("string-index", "String source range is out of bounds"))?;
        let destination_end = destination_offset
            .checked_add(source.len())
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "char destination range overflow",
                )
            })?;
        let Allocation::Array {
            kind: ArrayKind::Char,
            elements,
        } = self.heap.managed.get_mut(destination).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let target = elements
            .get_mut(destination_offset..destination_end)
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "char destination range is outside the array",
                )
            })?;
        for (slot, value) in target.iter_mut().zip(source) {
            *slot = HeapValue::Int(i32::from(*value));
        }
        Ok(())
    }

    pub(super) fn string_starts_with(
        &self,
        string: Handle,
        prefix: Handle,
        offset: i32,
    ) -> Result<bool, EmuError> {
        let Ok(offset) = usize::try_from(offset) else {
            return Ok(false);
        };
        let value = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        let prefix = self
            .heap
            .string_values
            .get(&prefix)
            .ok_or_else(type_error)?;
        let Some(region) = offset
            .checked_add(prefix.len())
            .and_then(|end| value.get(offset..end))
        else {
            return Ok(false);
        };
        self.string_units_equal(region, prefix, false)
    }

    pub(super) fn string_index_of(
        &self,
        string: Handle,
        needle: Handle,
        from: i32,
    ) -> Result<i32, EmuError> {
        let value = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        let needle = self
            .heap
            .string_values
            .get(&needle)
            .ok_or_else(type_error)?;
        let start = usize::try_from(from.max(0)).unwrap_or(usize::MAX);
        if needle.is_empty() {
            return Ok(i32::try_from(start.min(value.len())).unwrap_or(i32::MAX));
        }
        if start >= value.len() || needle.len() > value.len() {
            return Ok(-1);
        }
        let Some(relative) = two_way_utf16_index(&value[start..], needle, || {
            check_intrinsic_cancellation(self.native_context)
        })?
        else {
            return Ok(-1);
        };
        Ok(i32::try_from(start + relative).unwrap_or(i32::MAX))
    }

    pub(super) fn string_to_char_array(
        &mut self,
        string: Handle,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let length = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?
            .len();
        let array = self.allocate_array(
            ArrayKind::Char,
            i32::try_from(length).map_err(|_| type_error())?,
            &[],
            roots,
        )?;
        // The receiver is in the call roots; borrow its payload after possible GC.
        let units = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        let Allocation::Array {
            kind: ArrayKind::Char,
            elements,
        } = self.heap.managed.get_mut(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        for (slot, unit) in elements.iter_mut().zip(units) {
            *slot = HeapValue::Int(i32::from(*unit));
        }
        Ok(array)
    }

    pub(super) fn string_substring(
        &mut self,
        string: Handle,
        begin: i32,
        end: i32,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let units = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        let length = i32::try_from(units.len()).map_err(|_| type_error())?;
        if begin < 0 || end > length || begin > end {
            return Err(vm_error(
                "string-index",
                "String substring range is out of bounds",
            ));
        }
        if begin == 0 && end == length {
            return Ok(string);
        }
        let begin = usize::try_from(begin).map_err(|_| type_error())?;
        let end = usize::try_from(end).map_err(|_| type_error())?;
        let substring = units[begin..end].to_vec();
        let output = self.allocate_object("java/lang/String", HashMap::new(), &[], roots)?;
        self.store_string_units(output, substring, 1, &[], roots)?;
        Ok(output)
    }

    pub(super) fn string_to_lower_case(
        &mut self,
        string: Handle,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        self.string_to_case(string, false, roots)
    }

    pub(super) fn string_to_upper_case(
        &mut self,
        string: Handle,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        self.string_to_case(string, true, roots)
    }

    fn string_to_case(
        &mut self,
        string: Handle,
        uppercase: bool,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let units = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        let mut first_change = None;
        for (index, chunk) in units.chunks(STRING_WORK_CHUNK).enumerate() {
            check_intrinsic_cancellation(self.native_context)?;
            if let Some(offset) = chunk
                .iter()
                .position(|unit| simple_case_unit(*unit, uppercase) != *unit)
            {
                first_change = Some(index * STRING_WORK_CHUNK + offset);
                break;
            }
        }
        let Some(first_change) = first_change else {
            return Ok(string);
        };
        let mut converted = Vec::with_capacity(units.len());
        converted.extend_from_slice(&units[..first_change]);
        for chunk in units[first_change..].chunks(STRING_WORK_CHUNK) {
            check_intrinsic_cancellation(self.native_context)?;
            converted.extend(chunk.iter().map(|unit| simple_case_unit(*unit, uppercase)));
        }
        let output = self.allocate_object("java/lang/String", HashMap::new(), &[], roots)?;
        self.store_string_units(output, converted, 1, &[], roots)?;
        Ok(output)
    }

    pub(super) fn integer_parse_int(
        &self,
        string: Option<Handle>,
        radix: i32,
    ) -> Result<i32, EmuError> {
        let radix = u32::try_from(radix)
            .ok()
            .filter(|radix| (2..=36).contains(radix))
            .ok_or_else(|| vm_error("number-format", "invalid integer radix"))?;
        let string = string.ok_or_else(|| vm_error("number-format", "null integer string"))?;
        let units = self
            .heap
            .string_values
            .get(&string)
            .ok_or_else(type_error)?;
        if units.is_empty() {
            return Err(vm_error("number-format", "empty integer"));
        }
        let (negative, digits) = match units[0] {
            value if value == u16::from(b'-') => (true, &units[1..]),
            _ => (false, units.as_slice()),
        };
        if digits.is_empty() {
            return Err(vm_error("number-format", "integer has no digits"));
        }
        let limit = if negative { i32::MIN } else { -i32::MAX };
        let multiply_limit = limit / radix as i32;
        let mut result = 0_i32;
        for (index, unit) in digits.iter().enumerate() {
            if index.is_multiple_of(STRING_WORK_CHUNK) {
                check_intrinsic_cancellation(self.native_context)?;
            }
            let digit = natives::character_digit(*unit, radix)
                .map(|digit| digit as i32)
                .ok_or_else(|| vm_error("number-format", "invalid integer digit"))?;
            if result < multiply_limit {
                return Err(vm_error("number-format", "integer overflow"));
            }
            result *= radix as i32;
            if result < limit + digit {
                return Err(vm_error("number-format", "integer overflow"));
            }
            result -= digit;
        }
        Ok(if negative { result } else { -result })
    }
}

pub(super) fn utf16_equals_ignore_case(left: &[u16], right: &[u16]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(&left, &right)| java_utf16_case_equal(left, right))
}
