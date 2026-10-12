use super::pixel_access::IntPixelSource;
use super::{
    Allocation, Arc, ArrayKind, EmuError, Handle, HeapValue, ImmutableImagePixels, Machine, Value,
    heap_error, int_argument, reference_argument, transform_image_pixels, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn image_create_region(
        &mut self,
        args: &[Value],
    ) -> Result<Handle, EmuError> {
        let source = reference_argument(args, 0)?;
        let x = int_argument(args, 1)?;
        let y = int_argument(args, 2)?;
        let width = int_argument(args, 3)?;
        let height = int_argument(args, 4)?;
        let transform = int_argument(args, 5)?;
        let source_width =
            self.graphics_int_field(source, "javax/microedition/lcdui/Image.width:I")?;
        let source_height =
            self.graphics_int_field(source, "javax/microedition/lcdui/Image.height:I")?;
        let right = x.checked_add(width);
        let bottom = y.checked_add(height);
        if width <= 0
            || height <= 0
            || x < 0
            || y < 0
            || right.is_none_or(|right| right > source_width)
            || bottom.is_none_or(|bottom| bottom > source_height)
            || !(0..=7).contains(&transform)
        {
            return Err(vm_error(
                "illegal-argument",
                "invalid Image.createImage source region or transform",
            ));
        }
        let source_pixels =
            self.graphics_reference_field(source, "javax/microedition/lcdui/Image.pixels:[I")?;
        let region = self.graphics_int_array_region_snapshot(
            source_pixels,
            source_width,
            x,
            y,
            width,
            height,
        )?;
        let (output_width, output_height, pixels) =
            transform_image_pixels(region, width, height, transform)?;
        self.allocate_immutable_image(output_width, output_height, pixels, args)
    }

    pub(in crate::machine) fn image_create_copy(
        &mut self,
        args: &[Value],
    ) -> Result<Handle, EmuError> {
        let source = reference_argument(args, 0)?;
        // MIDP permits returning an immutable source directly. Mutable sources
        // still need a snapshot which later drawing cannot modify.
        if self.graphics_int_field(source, "javax/microedition/lcdui/Image.mutable:Z")? == 0 {
            return Ok(source);
        }
        let width = self.graphics_int_field(source, "javax/microedition/lcdui/Image.width:I")?;
        let height = self.graphics_int_field(source, "javax/microedition/lcdui/Image.height:I")?;
        let source_pixels =
            self.graphics_reference_field(source, "javax/microedition/lcdui/Image.pixels:[I")?;
        let pixels = self.graphics_int_array_snapshot(source_pixels)?;
        let expected = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| vm_error("illegal-argument", "invalid source image dimensions"))?;
        if pixels.len() != expected {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "source image pixels do not match its dimensions",
            ));
        }
        self.allocate_immutable_image(width, height, pixels, args)
    }

    pub(in crate::machine) fn graphics_create_nokia_image(
        &mut self,
        args: &[Value],
    ) -> Result<Handle, EmuError> {
        let width = int_argument(args, 0)?;
        let height = int_argument(args, 1)?;
        let color = int_argument(args, 2)?;
        let length = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .filter(|length| *length != 0 && *length <= 4_194_304)
            .ok_or_else(|| vm_error("illegal-argument", "invalid Nokia mutable image size"))?;
        self.allocate_image(width, height, vec![color; length], true, args)
    }

    pub(in crate::machine) fn allocate_immutable_image(
        &mut self,
        width: i32,
        height: i32,
        pixels: Vec<i32>,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        self.allocate_image(width, height, pixels, false, roots)
    }

    pub(in crate::machine) fn image_create_from_encoded(
        &mut self,
        args: &[Value],
        mutable: bool,
    ) -> Result<Handle, EmuError> {
        let source = reference_argument(args, 0)?;
        let offset = usize::try_from(int_argument(args, 1)?).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative image byte offset",
            )
        })?;
        let length = usize::try_from(int_argument(args, 2)?).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative image byte length",
            )
        })?;
        let Allocation::Array {
            kind: ArrayKind::Byte,
            elements,
        } = self.heap.managed.get(source).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= elements.len())
            .ok_or_else(|| {
                vm_error(
                    "array-index-out-of-bounds-exception",
                    "encoded image byte region exceeds source",
                )
            })?;
        // Resource packs can hold many images in one byte array. Materialize
        // only this image, without an intermediate i32 copy of the whole pack.
        let encoded = elements[offset..end]
            .iter()
            .map(|value| match value {
                HeapValue::Int(value) => Ok(*value as u8),
                _ => Err(type_error()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let image = graphics::Image::from_midp_encoded(&encoded).map_err(|error| {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let mut signature = String::with_capacity(16);
            for byte in encoded.iter().take(8) {
                signature.push(char::from(HEX[usize::from(byte >> 4)]));
                signature.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
            vm_error(
                "illegal-argument",
                format!(
                    "image data cannot be decoded: {}; length={} signature={signature}",
                    error.message(),
                    encoded.len()
                ),
            )
        })?;
        drop(encoded);
        let width = i32::try_from(image.width())
            .map_err(|_| vm_error("illegal-argument", "image width exceeds i32"))?;
        let height = i32::try_from(image.height())
            .map_err(|_| vm_error("illegal-argument", "image height exceeds i32"))?;
        // The decoder has already bounded the total pixel count. Do not add an
        // independent per-axis limit here: MIDP games commonly use long,
        // narrow sprite sheets whose area remains small.
        let pixels = image
            .into_pixels()
            .into_iter()
            .map(u32::cast_signed)
            .collect();
        self.allocate_image(width, height, pixels, mutable, args)
    }

    pub(in crate::machine) fn allocate_image(
        &mut self,
        width: i32,
        height: i32,
        pixels: Vec<i32>,
        mutable: bool,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let length = i32::try_from(pixels.len())
            .map_err(|_| vm_error("out-of-memory-error", "image is too large"))?;
        let pixel_array =
            self.allocate_array(ArrayKind::Int, if mutable { length } else { 0 }, &[], roots)?;
        if mutable {
            let Allocation::Array {
                kind: ArrayKind::Int,
                elements,
            } = self.heap.managed.get_mut(pixel_array).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            for (slot, pixel) in elements.iter_mut().zip(pixels) {
                *slot = HeapValue::Int(pixel);
            }
        } else {
            self.store_immutable_image_pixels(
                pixel_array,
                Arc::new(ImmutableImagePixels::from_argb(pixels)),
                &[],
                roots,
            )?;
        }

        let mut fields = self.initial_instance_fields("javax/microedition/lcdui/Image")?;
        for (key, value) in [
            (
                "javax/microedition/lcdui/Image.width:I",
                HeapValue::Int(width),
            ),
            (
                "javax/microedition/lcdui/Image.height:I",
                HeapValue::Int(height),
            ),
            (
                "javax/microedition/lcdui/Image.pixels:[I",
                HeapValue::Reference(Some(pixel_array)),
            ),
            (
                "javax/microedition/lcdui/Image.mutable:Z",
                HeapValue::Int(i32::from(mutable)),
            ),
        ] {
            let (_, _, slot) = fields
                .iter_mut()
                .find(|(field, _, _)| field.as_ref() == key)
                .ok_or_else(|| vm_error("field-not-found", key))?;
            *slot = value;
        }
        self.heap.temporary_roots.push(pixel_array);
        let image =
            self.allocate_linked_object("javax/microedition/lcdui/Image", fields, &[], roots);
        self.heap.temporary_roots.pop();
        if image.is_err() && !mutable {
            self.heap.immutable_image_pixels.remove(&pixel_array);
            self.heap
                .managed
                .set_external_bytes(pixel_array, 0)
                .map_err(heap_error)?;
        }
        image
    }

    pub(in crate::machine) fn image_copy_pixels(
        &mut self,
        args: &[Value],
    ) -> Result<Handle, EmuError> {
        let source = reference_argument(args, 0)?;
        let offset = usize::try_from(int_argument(args, 1)?).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative image pixel offset",
            )
        })?;
        let length = usize::try_from(int_argument(args, 2)?).map_err(|_| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "negative image pixel length",
            )
        })?;
        let process_alpha = int_argument(args, 3)? != 0;
        let end = offset.checked_add(length).ok_or_else(|| {
            vm_error(
                "array-index-out-of-bounds-exception",
                "image pixel copy exceeds source",
            )
        })?;
        let mut pixels = self
            .graphics_int_array_source(source)?
            .snapshot(offset..end)?;
        if !process_alpha {
            for pixel in &mut pixels {
                *pixel = (pixel.cast_unsigned() | 0xff00_0000).cast_signed();
            }
        }
        // The token avoids a second HeapValue array while external-byte
        // accounting still charges the immutable pixel payload.
        let token = self.allocate_array(ArrayKind::Int, 0, &[], args)?;
        self.store_immutable_image_pixels(
            token,
            Arc::new(ImmutableImagePixels::from_argb(pixels)),
            &[],
            args,
        )?;
        Ok(token)
    }

    pub(in crate::machine) fn graphics_cached_image_pixels(
        &mut self,
        image: Handle,
        pixels: Handle,
        draw_args: &[Value],
    ) -> Result<Option<Arc<ImmutableImagePixels>>, EmuError> {
        if self.graphics_int_field(image, "javax/microedition/lcdui/Image.mutable:Z")? != 0 {
            return Ok(None);
        }
        if let Some(cached) = self.heap.immutable_image_pixels.get(&pixels) {
            return Ok(Some(Arc::clone(cached)));
        }
        let cached = Arc::new(ImmutableImagePixels::from_argb(
            self.graphics_int_array_snapshot(pixels)?,
        ));
        let locals = [
            Some(Value::Reference(Some(image))),
            Some(Value::Reference(Some(pixels))),
        ];
        // Public wrapper intrinsics have not pushed a Java frame containing
        // the destination Graphics. Retain their arguments during cache GC.
        self.store_immutable_image_pixels(pixels, Arc::clone(&cached), &locals, draw_args)?;
        Ok(Some(cached))
    }

    pub(in crate::machine) fn image_get_rgb(&mut self, args: &[Value]) -> Result<(), EmuError> {
        let image = reference_argument(args, 0)?;
        let destination = reference_argument(args, 1)?;
        let offset = int_argument(args, 2)?;
        let scanlength = int_argument(args, 3)?;
        let x = int_argument(args, 4)?;
        let y = int_argument(args, 5)?;
        let width = int_argument(args, 6)?;
        let height = int_argument(args, 7)?;
        let source =
            self.graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")?;
        let source_width =
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?;
        let source_height =
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.height:I")?;
        if x < 0
            || y < 0
            || i64::from(x) + i64::from(width) > i64::from(source_width)
            || i64::from(y) + i64::from(height) > i64::from(source_height)
        {
            return Err(vm_error(
                "illegal-argument",
                "getRGB source region exceeds the image",
            ));
        }
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let offset = i64::from(offset);
        let scanlength = i64::from(scanlength);
        if scanlength.abs() < i64::from(width) {
            return Err(vm_error(
                "illegal-argument",
                "getRGB scanlength is shorter than the row width",
            ));
        }
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(destination).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let last_row = offset + i64::from(height - 1) * scanlength;
        let low = offset.min(last_row);
        let high = offset.max(last_row) + i64::from(width);
        if low < 0 || usize::try_from(high).map_or(true, |end| end > elements.len()) {
            return Err(vm_error(
                "array-index-out-of-bounds-exception",
                "getRGB destination range is out of bounds",
            ));
        }
        if let Some(pixels) = self.heap.immutable_image_pixels.get(&source) {
            IntPixelSource::Immutable(pixels).validate_region(source_width, x, y, width, height)?;
            let Allocation::Array {
                kind: ArrayKind::Int,
                elements,
            } = self.heap.managed.get_mut(destination).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            for row in 0..height as usize {
                let source_start = (y as usize + row) * source_width as usize + x as usize;
                let destination_start = (offset + row as i64 * scanlength) as usize;
                pixels.copy_into(
                    source_start,
                    &mut elements[destination_start..destination_start + width as usize],
                );
            }
            return Ok(());
        }
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(source).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if source != destination {
            IntPixelSource::Managed(elements).validate_region(source_width, x, y, width, height)?;
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
            let rows = source[y as usize * source_width as usize..]
                .chunks(source_width as usize)
                .take(height as usize)
                .map(|row| &row[x as usize..(x + width) as usize]);
            // Validate every requested value before writing, as the snapshot
            // path does. Distinct backing arrays then permit direct row copies.
            if rows
                .clone()
                .flatten()
                .any(|pixel| !matches!(pixel, HeapValue::Int(_)))
            {
                return Err(type_error());
            }
            for (row, source_row) in rows.enumerate() {
                let start = (offset + row as i64 * scanlength) as usize;
                destination[start..start + source_row.len()].copy_from_slice(source_row);
            }
            return Ok(());
        }
        let source_pixels = IntPixelSource::Managed(elements).snapshot_region(
            source_width,
            x,
            y,
            width,
            height,
            |work| super::check_raster_cancellation(self.native_context, work),
        )?;
        let destination_pixels = self.graphics_int_array_mut(destination)?;
        // The complete range was checked before any write: array bounds
        // errors must leave the destination unchanged, even with negative stride.
        for (row, source_row) in source_pixels.chunks_exact(width as usize).enumerate() {
            let start = (offset + row as i64 * scanlength) as usize;
            for (slot, &pixel) in destination_pixels[start..start + source_row.len()]
                .iter_mut()
                .zip(source_row)
            {
                *slot = HeapValue::Int(pixel);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/vm/machine/native_graphics/images.rs"]
mod tests;
