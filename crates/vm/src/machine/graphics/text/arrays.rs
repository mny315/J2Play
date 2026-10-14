use super::super::{
    Allocation, ArrayKind, EmuError, HeapValue, Machine, Value, check_raster_cancellation,
    heap_error, int_argument, reference_argument, type_error, vm_error,
};
use super::TextSource;

impl Machine<'_, '_> {
    pub(in crate::machine) fn graphics_unicode_draw_chars(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        let graphics = reference_argument(args, 0)?;
        let array = reference_argument(args, 1)?;
        let offset = int_argument(args, 2)?;
        let length = int_argument(args, 3)?;
        let Allocation::Array {
            kind: ArrayKind::Char,
            elements,
        } = self.heap.managed.get(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let range = usize::try_from(offset)
            .ok()
            .zip(usize::try_from(length).ok())
            .and_then(|(start, length)| start.checked_add(length).map(|end| start..end));
        let elements = range.and_then(|range| elements.get(range)).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "drawChars source range is out of bounds",
            )
        })?;
        // The VM heap already bounds the array. Copy only the requested range,
        // polling while converting its typed heap slots to a UTF-16 slice.
        let mut units = Vec::with_capacity(elements.len());
        for (index, value) in elements.iter().enumerate() {
            check_raster_cancellation(self.native_context, index)?;
            let HeapValue::Int(unit) = value else {
                return Err(type_error());
            };
            units.push(*unit as u16);
        }
        self.graphics_unicode_draw_text(
            graphics,
            TextSource::Utf16(&units),
            int_argument(args, 4)?,
            int_argument(args, 5)?,
            int_argument(args, 6)?,
        )
    }
}
