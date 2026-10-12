//! `Canvas` modes, framebuffer replacement and graphics retargeting.

use super::{
    EmuError, Handle, HeapValue, MAX_LCD_PIXELS, Machine, Value, heap_error, int_argument,
    reference_argument, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn canvas_dimension(
        &self,
        args: &[Value],
        width: bool,
    ) -> Result<i32, EmuError> {
        let canvas = reference_argument(args, 0)?;
        let framebuffer = self.graphics_reference_field(
            canvas,
            "javax/microedition/lcdui/Canvas.framebuffer:Ljavax/microedition/lcdui/Image;",
        )?;
        self.graphics_int_field(
            framebuffer,
            if width {
                "javax/microedition/lcdui/Image.width:I"
            } else {
                "javax/microedition/lcdui/Image.height:I"
            },
        )
    }

    pub(in crate::machine) fn canvas_set_full_screen_mode(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        let canvas = reference_argument(args, 0)?;
        let requested = int_argument(args, 1)? != 0;
        let (width, height) = if requested && self.limits.lcd_fullscreen_available {
            (self.limits.lcd_width, self.limits.lcd_height)
        } else {
            (self.limits.lcd_normal_width, self.limits.lcd_normal_height)
        };
        let width = i32::try_from(width)
            .map_err(|_| vm_error("framebuffer-size", "Canvas width exceeds Java int"))?;
        let height = i32::try_from(height)
            .map_err(|_| vm_error("framebuffer-size", "Canvas height exceeds Java int"))?;
        let framebuffer = self.graphics_reference_field(
            canvas,
            "javax/microedition/lcdui/Canvas.framebuffer:Ljavax/microedition/lcdui/Image;",
        )?;
        if self.graphics_int_field(framebuffer, "javax/microedition/lcdui/Image.width:I")? == width
            && self.graphics_int_field(framebuffer, "javax/microedition/lcdui/Image.height:I")?
                == height
        {
            return Ok(());
        }
        let pixel_count = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .filter(|count| *count != 0 && *count <= MAX_LCD_PIXELS)
            .ok_or_else(|| vm_error("framebuffer-size", "Canvas framebuffer size is invalid"))?;
        let framebuffer = self.allocate_image(width, height, vec![0; pixel_count], true, args)?;
        self.heap
            .managed
            .set_field(
                canvas,
                "javax/microedition/lcdui/Canvas.framebuffer:Ljavax/microedition/lcdui/Image;",
                HeapValue::Reference(Some(framebuffer)),
            )
            .map_err(heap_error)?;

        for field in [
            "javax/microedition/lcdui/Canvas.paintGraphics:Ljavax/microedition/lcdui/Graphics;",
            "javax/microedition/lcdui/Canvas.gameGraphics:Ljavax/microedition/lcdui/Graphics;",
        ] {
            let graphics = self.graphics_reference_field(canvas, field)?;
            self.retarget_canvas_graphics(graphics, framebuffer, width, height)?;
        }
        for (field, value) in [
            ("javax/microedition/lcdui/Canvas.pending:Z", 1),
            ("javax/microedition/lcdui/Canvas.damageX:I", 0),
            ("javax/microedition/lcdui/Canvas.damageY:I", 0),
            ("javax/microedition/lcdui/Canvas.damageW:I", width),
            ("javax/microedition/lcdui/Canvas.damageH:I", height),
            ("javax/microedition/lcdui/Canvas.sizeChangePending:Z", 1),
        ] {
            self.heap
                .managed
                .set_field(canvas, field, HeapValue::Int(value))
                .map_err(heap_error)?;
        }
        Ok(())
    }

    pub(in crate::machine) fn retarget_canvas_graphics(
        &mut self,
        graphics: Handle,
        framebuffer: Handle,
        width: i32,
        height: i32,
    ) -> Result<(), EmuError> {
        for (field, value) in [
            (
                "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;",
                HeapValue::Reference(Some(framebuffer)),
            ),
            ("javax/microedition/lcdui/Graphics.tx:I", HeapValue::Int(0)),
            ("javax/microedition/lcdui/Graphics.ty:I", HeapValue::Int(0)),
            (
                "javax/microedition/lcdui/Graphics.clipX:I",
                HeapValue::Int(0),
            ),
            (
                "javax/microedition/lcdui/Graphics.clipY:I",
                HeapValue::Int(0),
            ),
            (
                "javax/microedition/lcdui/Graphics.clipW:I",
                HeapValue::Int(width),
            ),
            (
                "javax/microedition/lcdui/Graphics.clipH:I",
                HeapValue::Int(height),
            ),
        ] {
            self.heap
                .managed
                .set_field(graphics, field, value)
                .map_err(heap_error)?;
        }
        Ok(())
    }
}
