use std::collections::hash_map::Entry;
use std::ops::Range;

use super::{
    Allocation, ArrayKind, EmuError, Handle, HashMap, HeapValue, MAX_LCD_PIXELS, Machine,
    check_raster_cancellation, heap_error, raster_work_chunks, type_error, vm_error,
};

/// Lossless LCDUI image backing. Uses byte-sized palette indices when smaller
/// than ARGB, so indexed sprite sheets do not exhaust small device heaps.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(in crate::machine) enum ImmutableImagePixels {
    Argb(Box<[i32]>),
    Indexed8 {
        palette: Box<[i32]>,
        indices: Box<[u8]>,
    },
}

impl ImmutableImagePixels {
    pub(in crate::machine) fn from_argb(pixels: Vec<i32>) -> Self {
        // An empty image or one pixel cannot be smaller with a palette.
        if pixels.len() <= 1 {
            return Self::Argb(pixels.into_boxed_slice());
        }
        let direct_bytes = pixels.len().saturating_mul(std::mem::size_of::<i32>());
        let mut palette = Vec::with_capacity(pixels.len().min(256));
        let mut palette_indices = HashMap::<i32, u8>::with_capacity(pixels.len().min(256));
        let mut indices = Vec::with_capacity(pixels.len());

        for &pixel in &pixels {
            let index = match palette_indices.entry(pixel) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    if palette.len() == 256 {
                        return Self::Argb(pixels.into_boxed_slice());
                    }
                    let index = u8::try_from(palette.len())
                        .expect("palette length was checked against the 256-entry limit");
                    palette.push(pixel);
                    entry.insert(index);
                    index
                }
            };
            indices.push(index);
        }

        let indexed_bytes = indices
            .len()
            .saturating_add(palette.len().saturating_mul(std::mem::size_of::<i32>()));
        if indexed_bytes >= direct_bytes {
            Self::Argb(pixels.into_boxed_slice())
        } else {
            Self::Indexed8 {
                palette: palette.into_boxed_slice(),
                indices: indices.into_boxed_slice(),
            }
        }
    }

    pub(in crate::machine) fn len(&self) -> usize {
        match self {
            Self::Argb(pixels) => pixels.len(),
            Self::Indexed8 { indices, .. } => indices.len(),
        }
    }

    pub(in crate::machine) fn get(&self, index: usize) -> Option<i32> {
        match self {
            Self::Argb(pixels) => pixels.get(index).copied(),
            Self::Indexed8 { palette, indices } => indices
                .get(index)
                .and_then(|index| palette.get(usize::from(*index)))
                .copied(),
        }
    }

    pub(in crate::machine) fn storage_bytes(&self) -> Option<usize> {
        match self {
            Self::Argb(pixels) => pixels.len().checked_mul(std::mem::size_of::<i32>()),
            Self::Indexed8 { palette, indices } => palette
                .len()
                .checked_mul(std::mem::size_of::<i32>())
                .and_then(|bytes| bytes.checked_add(indices.len())),
        }
    }

    // The caller validates the complete source rectangle before any write.
    pub(super) fn copy_into(&self, start: usize, output: &mut [HeapValue]) {
        let range = start..start + output.len();
        match self {
            Self::Argb(pixels) => {
                for (slot, &pixel) in output.iter_mut().zip(&pixels[range]) {
                    *slot = HeapValue::Int(pixel);
                }
            }
            Self::Indexed8 { palette, indices } => {
                for (slot, &index) in output.iter_mut().zip(&indices[range]) {
                    *slot = HeapValue::Int(palette[usize::from(index)]);
                }
            }
        }
    }

    #[cfg(test)]
    pub(in crate::machine) fn is_indexed(&self) -> bool {
        matches!(self, Self::Indexed8 { .. })
    }
}

pub(super) enum IntPixelSource<'a> {
    Immutable(&'a ImmutableImagePixels),
    Managed(&'a [HeapValue]),
}

pub(super) fn int_array_mut(
    heap: &mut heap::Heap,
    array: Handle,
) -> Result<&mut [HeapValue], EmuError> {
    let Allocation::Array {
        kind: ArrayKind::Int,
        elements,
    } = heap.get_mut(array).map_err(heap_error)?
    else {
        return Err(type_error());
    };
    Ok(elements.as_mut_slice())
}

impl IntPixelSource<'_> {
    pub(super) fn get(&self, index: usize) -> Result<i32, EmuError> {
        let pixel = match self {
            Self::Immutable(pixels) => pixels.get(index),
            Self::Managed(elements) => match elements.get(index) {
                Some(HeapValue::Int(pixel)) => Some(*pixel),
                Some(_) => return Err(type_error()),
                None => None,
            },
        };
        pixel.ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "image pixel is out of bounds",
            )
        })
    }

    fn len(&self) -> usize {
        match self {
            Self::Immutable(pixels) => pixels.len(),
            Self::Managed(elements) => elements.len(),
        }
    }

    pub(super) fn snapshot(&self, range: Range<usize>) -> Result<Vec<i32>, EmuError> {
        if range.start > range.end || range.end > self.len() {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "image pixel copy exceeds source",
            ));
        }
        let mut pixels = Vec::with_capacity(range.len());
        self.append_range(&mut pixels, range)?;
        Ok(pixels)
    }

    #[inline]
    pub(super) fn validate_region(
        &self,
        source_width: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<usize, EmuError> {
        if source_width <= 0
            || x < 0
            || y < 0
            || width < 0
            || height < 0
            || i64::from(x) + i64::from(width) > i64::from(source_width)
        {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "invalid graphics source region",
            ));
        }
        if width == 0 || height == 0 {
            return Ok(0);
        }
        let end = (i64::from(y) + i64::from(height) - 1) * i64::from(source_width)
            + i64::from(x)
            + i64::from(width);
        if usize::try_from(end).map_or(true, |end| end > self.len()) {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "graphics source region exceeds the pixel buffer",
            ));
        }
        // Positive, nonoverlapping rows fit in the validated backing buffer.
        Ok(width as usize * height as usize)
    }

    #[inline]
    pub(super) fn snapshot_region(
        &self,
        source_width: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        mut check_work: impl FnMut(usize) -> Result<(), EmuError>,
    ) -> Result<Vec<i32>, EmuError> {
        let capacity = self.validate_region(source_width, x, y, width, height)?;
        if capacity == 0 {
            return Ok(Vec::new());
        }
        let mut output = Vec::with_capacity(capacity);
        let (row_width, rows) = if width == source_width {
            (capacity, 1)
        } else {
            (width as usize, height)
        };
        for row in 0..rows {
            let start = (i64::from(y) + i64::from(row)) * i64::from(source_width) + i64::from(x);
            let start = start as usize;
            let row_work = output.len();
            for chunk in raster_work_chunks(row_work..row_work + row_width) {
                check_work(chunk.start)?;
                self.append_range(
                    &mut output,
                    start + chunk.start - row_work..start + chunk.end - row_work,
                )?;
            }
        }
        Ok(output)
    }

    // Callers validate the complete range before allocating the output.
    fn append_range(&self, output: &mut Vec<i32>, range: Range<usize>) -> Result<(), EmuError> {
        match self {
            Self::Immutable(ImmutableImagePixels::Argb(pixels)) => {
                output.extend_from_slice(&pixels[range]);
            }
            Self::Immutable(ImmutableImagePixels::Indexed8 { palette, indices }) => {
                output.extend(
                    indices[range]
                        .iter()
                        .map(|index| palette[usize::from(*index)]),
                );
            }
            Self::Managed(elements) => {
                for value in &elements[range] {
                    let HeapValue::Int(pixel) = *value else {
                        return Err(type_error());
                    };
                    output.push(pixel);
                }
            }
        }
        Ok(())
    }
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn graphics_int_field(
        &self,
        object: Handle,
        name: &str,
    ) -> Result<i32, EmuError> {
        match self.heap.managed.field(object, name).map_err(heap_error)? {
            HeapValue::Int(value) => Ok(value),
            _ => Err(type_error()),
        }
    }

    pub(in crate::machine) fn graphics_reference_field(
        &self,
        object: Handle,
        name: &str,
    ) -> Result<Handle, EmuError> {
        match self.heap.managed.field(object, name).map_err(heap_error)? {
            HeapValue::Reference(Some(value)) => Ok(value),
            HeapValue::Reference(None) => Err(vm_error("null-pointer-exception", "null reference")),
            _ => Err(type_error()),
        }
    }

    pub(in crate::machine) fn graphics_target(
        &self,
        graphics: Handle,
    ) -> Result<(Handle, i32, i32), EmuError> {
        let image = self.graphics_reference_field(
            graphics,
            "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;",
        )?;
        let pixels =
            self.graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")?;
        let width = self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?;
        Ok((
            pixels,
            width,
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.height:I")?,
        ))
    }

    pub(in crate::machine) fn graphics_int_array_snapshot(
        &self,
        array: Handle,
    ) -> Result<Vec<i32>, EmuError> {
        let pixels = self.graphics_int_array_source(array)?;
        pixels.snapshot(0..pixels.len())
    }

    pub(super) fn graphics_int_array_source(
        &self,
        array: Handle,
    ) -> Result<IntPixelSource<'_>, EmuError> {
        if let Some(pixels) = self.heap.immutable_image_pixels.get(&array) {
            return Ok(IntPixelSource::Immutable(pixels));
        }
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        Ok(IntPixelSource::Managed(elements))
    }

    pub(in crate::machine) fn graphics_int_array_region_snapshot(
        &self,
        array: Handle,
        source_width: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Vec<i32>, EmuError> {
        self.graphics_int_array_source(array)?.snapshot_region(
            source_width,
            x,
            y,
            width,
            height,
            |work| check_raster_cancellation(self.native_context, work),
        )
    }

    pub(in crate::machine) fn graphics_int_array_strided_snapshot_into_scratch(
        &mut self,
        array: Handle,
        offset: i32,
        scanlength: i32,
        width: i32,
        height: i32,
    ) -> Result<(), EmuError> {
        let capacity = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "invalid drawRGB source region",
                )
            })?;
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };

        if capacity == 0 {
            self.graphics.draw_rgb_scratch.clear();
            return Ok(());
        }
        let last_row = i64::from(offset) + i64::from(height - 1) * i64::from(scanlength);
        let low = i64::from(offset).min(last_row);
        let high = i64::from(offset).max(last_row) + i64::from(width);
        if low < 0 || usize::try_from(high).map_or(true, |end| end > elements.len()) {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "drawRGB source range is out of bounds",
            ));
        }
        if capacity > MAX_LCD_PIXELS {
            return Err(vm_error(
                "out-of-memory-error",
                "drawRGB scratch exceeds pixel limit",
            ));
        }
        self.graphics.draw_rgb_scratch.clear();
        self.graphics.draw_rgb_scratch.reserve(capacity);

        // A packed source is one continuous slice. Otherwise each row is
        // already covered by the range validation, including negative stride.
        let (row_width, rows) = if scanlength == width {
            (capacity, 1)
        } else {
            (width as usize, height)
        };
        for row in 0..rows {
            let start = (i64::from(offset) + i64::from(row) * i64::from(scanlength)) as usize;
            let row_work = self.graphics.draw_rgb_scratch.len();
            for chunk in raster_work_chunks(row_work..row_work + row_width) {
                check_raster_cancellation(self.native_context, chunk.start)?;
                for value in &elements[start + chunk.start - row_work..start + chunk.end - row_work]
                {
                    let HeapValue::Int(pixel) = *value else {
                        return Err(type_error());
                    };
                    self.graphics.draw_rgb_scratch.push(pixel);
                }
            }
        }
        Ok(())
    }

    pub(in crate::machine) fn graphics_int_array_mut(
        &mut self,
        array: Handle,
    ) -> Result<&mut [HeapValue], EmuError> {
        int_array_mut(&mut self.heap.managed, array)
    }
}
