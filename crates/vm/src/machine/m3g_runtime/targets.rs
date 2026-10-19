use super::{
    Allocation, ArrayKind, EmuError, Handle, HeapValue, Machine, heap_error, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_publish_bound_target(&mut self) -> Result<(), EmuError> {
        let target = self.m3g.graphics.target.ok_or_else(|| {
            vm_error(
                "illegal-state-exception",
                "Graphics3D operation requires a bound target",
            )
        })?;
        let class = self.object_class(target)?;
        if self.is_instance(&class, "javax/microedition/lcdui/Graphics") {
            let (array, width, height) = self.graphics_target(target)?;
            let expected = usize::try_from(width)
                .ok()
                .and_then(|width| {
                    usize::try_from(height)
                        .ok()
                        .and_then(|height| width.checked_mul(height))
                })
                .ok_or_else(|| vm_error("illegal-argument-exception", "target size overflow"))?;
            let Allocation::Array {
                kind: ArrayKind::Int,
                elements: destination,
            } = self.heap.managed.get_mut(array).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            if destination.len() != expected {
                return Err(vm_error(
                    "illegal-state-exception",
                    "M3G renderer and Graphics target dimensions diverged",
                ));
            }
            if self.m3g.graphics.renderer.resolution_scale() == 2 {
                if let Some(clip) = self.m3g.graphics.renderer.take_rgb_damage() {
                    self.graphics.enhanced.store_resolved(
                        array,
                        self.m3g.graphics.renderer.pixels(),
                        clip,
                        true,
                        Some(destination),
                    )?;
                }
                self.graphics.enhanced.sync_m3g(Some(array));
                return Ok(());
            }
            if self.m3g.graphics.renderer.pixels().len() != expected
                || self.m3g.graphics.renderer.width() != width as u32
                || self.m3g.graphics.renderer.height() != height as u32
            {
                return Err(vm_error(
                    "illegal-state-exception",
                    "M3G renderer and Graphics target dimensions diverged",
                ));
            }
            // Publication normalizes renderer alpha as a reload from the RGB
            // target would, retaining unmodified color and depth locally.
            let damage = self.m3g.graphics.renderer.take_rgb_damage();
            let [x, y, dirty_width, dirty_height] =
                damage.unwrap_or([0, 0, width as u32, height as u32]);
            let pixels = self.m3g.graphics.renderer.pixels();
            for row in y as usize..(y + dirty_height) as usize {
                let start = row * width as usize + x as usize;
                let end = start + dirty_width as usize;
                for (destination, source) in
                    destination[start..end].iter_mut().zip(&pixels[start..end])
                {
                    // MIDP Graphics targets are RGB; LCDUI storage uses ARGB words.
                    *destination = HeapValue::Int((source | 0xff00_0000).cast_signed());
                }
            }
            return Ok(());
        }
        if self.is_instance(&class, "javax/microedition/m3g/Image2D") {
            let native = self.m3g_handle(target)?;
            let m3g::ObjectKind::Image2D(image) = self.m3g.runtime.kind_mut(native)? else {
                return Err(type_error());
            };
            return image.load_argb(self.m3g.graphics.renderer.pixels());
        }
        Err(vm_error(
            "illegal-argument-exception",
            "Graphics3D target must be Graphics or mutable Image2D",
        ))
    }

    pub(in crate::machine) fn m3g_refresh_bound_target(&mut self) -> Result<(), EmuError> {
        let target = self.m3g.graphics.target.ok_or_else(|| {
            vm_error(
                "illegal-state-exception",
                "Graphics3D operation requires a bound target",
            )
        })?;
        let class = self.object_class(target)?;
        if self.is_instance(&class, "javax/microedition/lcdui/Graphics") {
            let (array, width, height) = self.m3g_graphics_target(target)?;
            if self.m3g.graphics.renderer.resolution_scale() == 1
                && self.native_context.render_3d_scale() != 2
                && !self.m3g.graphics.renderer.has_color_damage()
                && self.m3g.graphics.renderer.width() == width
                && self.m3g.graphics.renderer.height() == height
                && !self.heap.immutable_image_pixels.contains_key(&array)
                && self.m3g_managed_pixels_match(array)?
            {
                return Ok(());
            }
            if self.m3g.graphics.renderer.resolution_scale() == 2
                && !self.m3g.graphics.renderer.has_color_damage()
                && self.graphics.enhanced.m3g_is_synced(array)
            {
                return Ok(());
            }
            // Host sample planes are rebuilt after Continue, even when the guest
            // keeps Graphics3D bound across the checkpoint boundary.
            let scale = if self.ensure_enhanced_target(array, width, height)? {
                2
            } else {
                1
            };
            self.m3g.graphics.renderer.set_resolution_scale(scale)?;
            if self.m3g.graphics.renderer.resolution_scale() == 2
                && let Some(pixels) = self.graphics.enhanced.pixels(array)?
            {
                return self.m3g.graphics.renderer.replace_pixels(pixels);
            }
            let pixels = self
                .graphics_int_array_snapshot(array)?
                .into_iter()
                .map(i32::cast_unsigned)
                .collect::<Vec<_>>();
            return self.m3g.graphics.renderer.replace_pixels(pixels);
        }
        if self.is_instance(&class, "javax/microedition/m3g/Image2D") {
            let native = self.m3g_handle(target)?;
            let m3g::ObjectKind::Image2D(image) = self.m3g.runtime.kind(native)? else {
                return Err(type_error());
            };
            validate_image_target(image)?;
            return self.m3g.graphics.renderer.load_pixels(image.pixels());
        }
        Err(vm_error(
            "illegal-argument-exception",
            "Graphics3D target must be Graphics or mutable Image2D",
        ))
    }

    fn m3g_managed_pixels_match(&self, array: Handle) -> Result<bool, EmuError> {
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let pixels = self.m3g.graphics.renderer.pixels();
        if elements.len() != pixels.len() {
            return Ok(false);
        }
        // Reduce small blocks without a branch per pixel. Stop at a changed
        // block before allocating a checked snapshot; unrelated heap writes
        // cannot force a full attachment reload.
        for (values, pixels) in elements.chunks(1024).zip(pixels.chunks(1024)) {
            self.m3g_check_geometry_cancellation()?;
            let matches = values
                .chunks(64)
                .zip(pixels.chunks(64))
                .all(|(values, pixels)| {
                    values
                        .iter()
                        .zip(pixels)
                        .fold(true, |equal, (value, pixel)| {
                            let same = match value {
                                HeapValue::Int(value) => value.cast_unsigned() == *pixel,
                                _ => false,
                            };
                            equal & same
                        })
                });
            if !matches {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(in crate::machine) fn m3g_target_snapshot(
        &self,
        target: Handle,
    ) -> Result<(u32, u32, Vec<u32>, [u32; 4]), EmuError> {
        let class = self.object_class(target)?;
        if self.is_instance(&class, "javax/microedition/lcdui/Graphics") {
            let (pixels, width, height) = self.m3g_graphics_target(target)?;
            let pixels = self
                .graphics_int_array_snapshot(pixels)?
                .into_iter()
                .map(i32::cast_unsigned)
                .collect();
            let clip_x = self
                .graphics_int_field(target, "javax/microedition/lcdui/Graphics.clipX:I")
                .unwrap_or(0)
                .clamp(0, width as i32) as u32;
            let clip_y = self
                .graphics_int_field(target, "javax/microedition/lcdui/Graphics.clipY:I")
                .unwrap_or(0)
                .clamp(0, height as i32) as u32;
            let clip_width = self
                .graphics_int_field(target, "javax/microedition/lcdui/Graphics.clipW:I")
                .unwrap_or(width as i32)
                .max(0) as u32;
            let clip_height = self
                .graphics_int_field(target, "javax/microedition/lcdui/Graphics.clipH:I")
                .unwrap_or(height as i32)
                .max(0) as u32;
            let clip_width = clip_width.min(width - clip_x);
            let clip_height = clip_height.min(height - clip_y);
            return Ok((
                width,
                height,
                pixels,
                [clip_x, clip_y, clip_width, clip_height],
            ));
        }
        if self.is_instance(&class, "javax/microedition/m3g/Image2D") {
            let native = self.m3g_handle(target)?;
            let m3g::ObjectKind::Image2D(image) = self.m3g.runtime.kind(native)? else {
                return Err(type_error());
            };
            validate_image_target(image)?;
            return Ok((
                image.width(),
                image.height(),
                image.pixels().to_vec(),
                [0, 0, image.width(), image.height()],
            ));
        }
        Err(vm_error(
            "illegal-argument-exception",
            "Graphics3D target must be Graphics or mutable Image2D",
        ))
    }

    fn m3g_graphics_target(&self, target: Handle) -> Result<(Handle, u32, u32), EmuError> {
        let (pixels, width, height) = self.graphics_target(target)?;
        if width <= 0 || height <= 0 {
            return Err(vm_error(
                "illegal-argument-exception",
                "Graphics3D target dimensions must be positive",
            ));
        }
        let width = width.cast_unsigned();
        let height = height.cast_unsigned();
        if self.heap.managed.array_length(pixels).map_err(heap_error)? as u64
            != u64::from(width) * u64::from(height)
        {
            return Err(vm_error(
                "illegal-argument-exception",
                "Graphics3D target storage does not match its dimensions",
            ));
        }
        Ok((pixels, width, height))
    }
}

fn validate_image_target(image: &m3g::Image2DState) -> Result<(), EmuError> {
    if !image.is_mutable() {
        return Err(vm_error(
            "illegal-argument-exception",
            "Graphics3D target Image2D must be mutable",
        ));
    }
    if !matches!(
        image.format(),
        m3g::ImageFormat::Rgb | m3g::ImageFormat::Rgba
    ) {
        return Err(vm_error(
            "illegal-argument-exception",
            "Graphics3D Image2D target must use RGB or RGBA format",
        ));
    }
    Ok(())
}
