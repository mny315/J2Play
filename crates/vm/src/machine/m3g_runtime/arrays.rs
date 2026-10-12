use super::{
    Allocation, ArrayKind, EmuError, Handle, HeapValue, Machine, Value, heap_error,
    m3g_non_negative_usize, optional_reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_optional_transform(
        &self,
        args: &[Value],
        index: usize,
    ) -> Result<m3g::Mat4, EmuError> {
        optional_reference_argument(args, index)?.map_or(Ok(m3g::Mat4::IDENTITY), |guest| {
            let handle = self.m3g_handle(guest)?;
            self.m3g.runtime.transform_value(handle)
        })
    }

    pub(in crate::machine) fn m3g_validate_texture_image(
        &self,
        image: m3g::Handle,
    ) -> Result<(), EmuError> {
        let m3g::ObjectKind::Image2D(image) = self.m3g.runtime.kind(image)? else {
            return Err(type_error());
        };
        let maximum = self.limits.m3g_compatibility_max_texture_dimension;
        if image.width() > maximum
            || image.height() > maximum
            || !image.width().is_power_of_two()
            || !image.height().is_power_of_two()
        {
            return Err(vm_error(
                "illegal-argument-exception",
                format!(
                    "Texture2D image dimensions {}x{} must be powers of two no greater than compatibility limit {maximum} (device property {})",
                    image.width(),
                    image.height(),
                    self.limits.m3g_max_texture_dimension,
                ),
            ));
        }
        Ok(())
    }

    pub(in crate::machine) fn m3g_texture_unit(&self, value: i32) -> Result<usize, EmuError> {
        let unit = m3g_non_negative_usize(value)?;
        if unit >= self.limits.m3g_num_texture_units {
            return Err(vm_error(
                "index-out-of-bounds-exception",
                "texture unit exceeds the active profile limit",
            ));
        }
        Ok(unit)
    }

    #[cfg(test)]
    pub(in crate::machine) fn m3g_float_array(
        &self,
        array: Handle,
        minimum: usize,
    ) -> Result<Vec<f32>, EmuError> {
        self.m3g_float_array_elements(array, minimum)?
            .iter()
            .map(|value| match value {
                HeapValue::Float(value) => Ok(*value),
                _ => Err(type_error()),
            })
            .collect()
    }

    /// Copies only the prefix consumed by the native operation.
    pub(in crate::machine) fn m3g_read_float_array(
        &self,
        array: Handle,
        destination: &mut [f32],
    ) -> Result<(), EmuError> {
        let elements = self.m3g_float_array_elements(array, destination.len())?;
        for (destination, value) in destination.iter_mut().zip(elements) {
            let HeapValue::Float(value) = value else {
                return Err(type_error());
            };
            *destination = *value;
        }
        Ok(())
    }

    pub(in crate::machine) fn m3g_float_array_elements(
        &self,
        array: Handle,
        minimum: usize,
    ) -> Result<&[HeapValue], EmuError> {
        let Allocation::Array { kind, elements } =
            self.heap.managed.get(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if !matches!(kind, ArrayKind::Float) || elements.len() < minimum {
            return Err(vm_error(
                "illegal-argument-exception",
                "M3G float array is too short or has the wrong type",
            ));
        }
        Ok(elements)
    }

    pub(in crate::machine) fn m3g_int_array(&self, array: Handle) -> Result<Vec<i32>, EmuError> {
        self.m3g_int_array_prefix(array, usize::MAX)
    }

    /// Copies at most the integers consumed by the native operation; short inputs stay short.
    pub(in crate::machine) fn m3g_int_array_prefix(
        &self,
        array: Handle,
        maximum: usize,
    ) -> Result<Vec<i32>, EmuError> {
        self.m3g_primitive_int_array(array, &ArrayKind::Int, maximum, std::convert::identity)
    }

    pub(in crate::machine) fn m3g_byte_array(&self, array: Handle) -> Result<Vec<u8>, EmuError> {
        self.m3g_byte_array_prefix(array, usize::MAX)
    }

    /// Copies at most the bytes the native operation consumes; short inputs stay short.
    pub(in crate::machine) fn m3g_byte_array_prefix(
        &self,
        array: Handle,
        maximum: usize,
    ) -> Result<Vec<u8>, EmuError> {
        self.m3g_primitive_int_array(array, &ArrayKind::Byte, maximum, |value| value as u8)
    }

    fn m3g_primitive_int_array<T>(
        &self,
        array: Handle,
        expected: &ArrayKind,
        maximum: usize,
        convert: impl Fn(i32) -> T,
    ) -> Result<Vec<T>, EmuError> {
        self.m3g_primitive_array_elements(array, expected)?
            .iter()
            .take(maximum)
            .map(|value| match value {
                HeapValue::Int(value) => Ok(convert(*value)),
                _ => Err(type_error()),
            })
            .collect()
    }

    pub(in crate::machine) fn m3g_primitive_array_elements(
        &self,
        array: Handle,
        expected: &ArrayKind,
    ) -> Result<&[HeapValue], EmuError> {
        let Allocation::Array { kind, elements } =
            self.heap.managed.get(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if kind != expected {
            return Err(type_error());
        }
        Ok(elements)
    }

    pub(in crate::machine) fn m3g_reference_array(
        &self,
        array: Handle,
    ) -> Result<Vec<Option<Handle>>, EmuError> {
        let Allocation::Array { kind, elements } =
            self.heap.managed.get(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if !matches!(kind, ArrayKind::Reference(_)) {
            return Err(type_error());
        }
        elements
            .iter()
            .map(|value| match value {
                HeapValue::Reference(value) => Ok(*value),
                _ => Err(type_error()),
            })
            .collect()
    }

    pub(in crate::machine) fn m3g_write_float_array(
        &mut self,
        array: Handle,
        values: &[f32],
    ) -> Result<(), EmuError> {
        let elements = self.m3g_float_array_elements_mut(array, values.len())?;
        for (destination, source) in elements.iter_mut().zip(values) {
            *destination = HeapValue::Float(*source);
        }
        Ok(())
    }

    pub(in crate::machine) fn m3g_float_array_elements_mut(
        &mut self,
        array: Handle,
        minimum: usize,
    ) -> Result<&mut [HeapValue], EmuError> {
        let Allocation::Array { kind, elements } =
            self.heap.managed.get_mut(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if !matches!(kind, ArrayKind::Float) || elements.len() < minimum {
            return Err(vm_error(
                "illegal-argument-exception",
                "M3G destination float array is too short or has the wrong type",
            ));
        }
        Ok(elements)
    }

    pub(in crate::machine) fn m3g_write_int_array(
        &mut self,
        array: Handle,
        values: &[i32],
    ) -> Result<(), EmuError> {
        self.m3g_write_primitive_int_array(array, &ArrayKind::Int, values)
    }

    pub(in crate::machine) fn m3g_write_primitive_int_array<T: Copy + Into<i32>>(
        &mut self,
        array: Handle,
        expected: &ArrayKind,
        values: &[T],
    ) -> Result<(), EmuError> {
        let Allocation::Array { kind, elements } =
            self.heap.managed.get_mut(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if kind != expected || elements.len() < values.len() {
            return Err(vm_error(
                "illegal-argument-exception",
                "M3G destination array is too short or has the wrong type",
            ));
        }
        for (destination, source) in elements.iter_mut().zip(values) {
            *destination = HeapValue::Int((*source).into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/vm/machine/m3g_runtime/arrays.rs"]
mod tests;
