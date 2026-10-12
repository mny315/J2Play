use super::{
    Allocation, ArrayKind, EmuError, Handle, HeapValue, Machine, heap_error, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn micro3d_bound_target(&self) -> Result<(Handle, i32, i32), EmuError> {
        let graphics = self
            .micro3d
            .target
            .ok_or_else(|| vm_error("illegal-state-exception", "Graphics3D is not bound"))?;
        self.graphics_target(graphics)
    }

    pub(in crate::machine) fn micro3d_graphics_scissor(
        &self,
        graphics: Handle,
        width: i32,
        height: i32,
    ) -> Result<[u32; 4], EmuError> {
        if width <= 0 || height <= 0 {
            return Err(vm_error(
                "framebuffer-size",
                "Micro3D target dimensions must be positive",
            ));
        }
        let clip_x = self
            .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipX:I")
            .unwrap_or(0);
        let clip_y = self
            .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipY:I")
            .unwrap_or(0);
        let clip_width = self
            .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipW:I")
            .unwrap_or(width)
            .max(0);
        let clip_height = self
            .graphics_int_field(graphics, "javax/microedition/lcdui/Graphics.clipH:I")
            .unwrap_or(height)
            .max(0);
        let axis = |origin: i32, extent: i32, limit: i32| {
            let start = i64::from(origin).clamp(0, i64::from(limit));
            let end = (i64::from(origin) + i64::from(extent)).clamp(0, i64::from(limit));
            [
                u32::try_from(start).unwrap_or(0),
                u32::try_from(end.saturating_sub(start)).unwrap_or(0),
            ]
        };
        let x = axis(clip_x, clip_width, width);
        let y = axis(clip_y, clip_height, height);
        Ok([x[0], y[0], x[1], y[1]])
    }

    pub(in crate::machine) fn micro3d_prepare_render_target(&mut self) -> Result<(), EmuError> {
        if !self.micro3d.render_pending {
            let (pixels, width, height) = self.micro3d_bound_target()?;
            let snapshot = self
                .graphics_int_array_snapshot(pixels)?
                .into_iter()
                .map(i32::cast_unsigned)
                .collect::<Vec<_>>();
            self.micro3d.runtime.replace_target_clipped(
                u32::try_from(width).map_err(|_| type_error())?,
                u32::try_from(height).map_err(|_| type_error())?,
                snapshot,
                self.micro3d.target_scissor,
            )?;
        }
        self.micro3d
            .runtime
            .set_target_scissor(self.micro3d.command_scissor)
    }

    pub(in crate::machine) fn micro3d_flush_target(&mut self) -> Result<(), EmuError> {
        let (pixels, width, height) = self.micro3d_bound_target()?;
        if !self.micro3d.render_pending {
            return Ok(());
        }
        let [x, y, clip_width, clip_height] = self.micro3d.target_scissor;
        let source = self.micro3d.runtime.target_pixels();
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements: destination,
        } = self.heap.managed.get_mut(pixels).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let target_width = usize::try_from(width).map_err(|_| type_error())?;
        let target_height = usize::try_from(height).map_err(|_| type_error())?;
        if target_width.checked_mul(target_height) != Some(source.len())
            || destination.len() != source.len()
            || u64::from(x) + u64::from(clip_width) > target_width as u64
            || u64::from(y) + u64::from(clip_height) > target_height as u64
        {
            return Err(vm_error(
                "framebuffer-size",
                "Micro3D target size changed while bound",
            ));
        }
        for row in y..y + clip_height {
            let start = row as usize * target_width + x as usize;
            let end = start + clip_width as usize;
            for (slot, pixel) in destination[start..end].iter_mut().zip(&source[start..end]) {
                *slot = HeapValue::Int(pixel.cast_signed());
            }
        }
        self.micro3d.render_pending = false;
        Ok(())
    }
}
