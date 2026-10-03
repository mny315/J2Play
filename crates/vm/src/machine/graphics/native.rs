//! Native LCDUI and Nokia graphics entrypoints, owned by the graphics module.

use crate::machine::{
    CallOutcome, EmuError, HeapValue, Machine, Method, Value, heap_error, int_argument,
    reference_argument, type_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn invoke_graphics_native(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Result<Option<CallOutcome>, EmuError> {
        let signature = (
            method.key.class.as_str(),
            method.key.name.as_str(),
            method.key.descriptor.as_str(),
        );
        let outcome = match signature {
            (
                "javax/microedition/lcdui/game/Sprite",
                "collision",
                "(Ljavax/microedition/lcdui/Image;IIIIIIIIIIIZ)Z",
            ) => CallOutcome::Return(Some(Value::Int(i32::from(
                self.graphics_sprite_collision(args)?,
            )))),
            (
                "com/nokia/mid/ui/DirectUtils",
                "getDirectGraphics",
                "(Ljavax/microedition/lcdui/Graphics;)Lcom/nokia/mid/ui/DirectGraphics;",
            ) => {
                // DirectGraphics is another view of the same Nokia graphics
                // context, so retaining the Graphics handle also preserves
                // clip, translation, color and target state.
                let graphics = reference_argument(args, 0)?;
                let class = self.object_class(graphics)?;
                if !self.is_instance(&class, "javax/microedition/lcdui/Graphics") {
                    return Err(type_error());
                }
                CallOutcome::Return(Some(Value::Reference(Some(graphics))))
            }
            (
                "com/nokia/mid/ui/DirectUtils",
                "createImage",
                "(III)Ljavax/microedition/lcdui/Image;",
            ) => CallOutcome::Return(Some(Value::Reference(Some(
                self.graphics_create_nokia_image(args)?,
            )))),
            (
                "com/nokia/mid/ui/DirectUtils",
                "createImage",
                "([BII)Ljavax/microedition/lcdui/Image;",
            ) => CallOutcome::Return(Some(Value::Reference(Some(
                self.image_create_from_encoded(args, true)?,
            )))),
            ("javax/microedition/lcdui/Image", "getRGBPixels", "([IIIIIII)V") => {
                self.image_get_rgb(args)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/lcdui/Image", "copyPixels", "([IIIZ)[I") => {
                CallOutcome::Return(Some(Value::Reference(Some(self.image_copy_pixels(args)?))))
            }
            (
                "javax/microedition/lcdui/Image",
                "createImage",
                "([BII)Ljavax/microedition/lcdui/Image;",
            ) => CallOutcome::Return(Some(Value::Reference(Some(
                self.image_create_from_encoded(args, false)?,
            )))),
            (
                "javax/microedition/lcdui/Image",
                "createImage",
                "(Ljavax/microedition/lcdui/Image;IIIII)Ljavax/microedition/lcdui/Image;",
            ) => CallOutcome::Return(Some(Value::Reference(Some(
                self.image_create_region(args)?,
            )))),
            (
                "javax/microedition/lcdui/Image",
                "createImage",
                "(Ljavax/microedition/lcdui/Image;)Ljavax/microedition/lcdui/Image;",
            ) => CallOutcome::Return(Some(Value::Reference(Some(self.image_create_copy(args)?)))),
            ("javax/microedition/lcdui/Canvas", "getWidth", "()I") => {
                CallOutcome::Return(Some(Value::Int(self.canvas_dimension(args, true)?)))
            }
            ("javax/microedition/lcdui/Canvas", "getHeight", "()I") => {
                CallOutcome::Return(Some(Value::Int(self.canvas_dimension(args, false)?)))
            }
            ("javax/microedition/lcdui/Canvas", "setFullScreenMode", "(Z)V") => {
                self.canvas_set_full_screen_mode(args)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/lcdui/Canvas" | "javax/microedition/lcdui/Screen",
                "present",
                "([I)V",
            ) => self.schedule_frame_presentation(method, args)?,
            ("javax/microedition/lcdui/Graphics", "fillRectPixels", "(IIIII)V") => {
                self.graphics_fill_rect(args)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/lcdui/Graphics", "fillTriangle", "(IIIIII)V") => {
                self.graphics_fill_triangle(args)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/lcdui/Graphics", "arc", "(IIIIIIZ)V") => {
                self.graphics_draw_arc(args, int_argument(args, 7)? != 0)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/lcdui/Graphics", "setClip", "(IIII)V") => {
                self.graphics_set_clip(args)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/lcdui/Graphics", "drawRGBPixels", "([IIIIIIIZ)V") => {
                self.graphics_draw_rgb(args)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/lcdui/Graphics",
                "drawImagePixels",
                "(Ljavax/microedition/lcdui/Image;II)V",
            ) => {
                self.graphics_draw_image(args)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/lcdui/Graphics",
                "drawRegionPixels",
                "(Ljavax/microedition/lcdui/Image;IIIIIIIII)V",
            ) => {
                self.graphics_draw_region(args)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/lcdui/Graphics",
                "drawImage",
                "(Ljavax/microedition/lcdui/Image;IIII)V",
            ) => {
                self.graphics_draw_nokia_image(args)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/lcdui/Graphics", "setARGBColor", "(I)V") => {
                self.graphics_set_argb_color(args)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/lcdui/Graphics", "setGrayScale", "(I)V") => {
                let graphics = reference_argument(args, 0)?;
                let gray = int_argument(args, 1)?;
                if (0..=255).contains(&gray) {
                    let color =
                        0xff00_0000_u32 | (u32::try_from(gray).unwrap_or_default() * 0x0001_0101);
                    self.heap
                        .managed
                        .set_field(
                            graphics,
                            "javax/microedition/lcdui/Graphics.color:I",
                            HeapValue::Int(color.cast_signed()),
                        )
                        .map_err(heap_error)?;
                    CallOutcome::Return(None)
                } else {
                    self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("gray scale must be in 0..255"),
                    )?
                }
            }
            ("javax/microedition/lcdui/Graphics", "getStrokeStyle", "()I") => {
                let graphics = reference_argument(args, 0)?;
                CallOutcome::Return(Some(Value::Int(self.graphics_int_field(
                    graphics,
                    "javax/microedition/lcdui/Graphics.stroke:I",
                )?)))
            }
            (
                "javax/microedition/lcdui/Graphics",
                "getAlphaComponent" | "getRedComponent" | "getGreenComponent" | "getBlueComponent",
                "()I",
            ) => {
                let shift = match method.key.name.as_str() {
                    "getAlphaComponent" => 24,
                    "getRedComponent" => 16,
                    "getGreenComponent" => 8,
                    "getBlueComponent" => 0,
                    _ => unreachable!("matched color component getter"),
                };
                CallOutcome::Return(Some(Value::Int(
                    self.graphics_color_component(args, shift)?,
                )))
            }
            ("javax/microedition/lcdui/Graphics", "getNativePixelFormat", "()I") => {
                CallOutcome::Return(Some(Value::Int(8888)))
            }
            ("javax/microedition/lcdui/Graphics", "drawPolygon" | "fillPolygon", "([II[IIII)V") => {
                self.graphics_nokia_polygon(args, method.key.name == "fillPolygon")?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/lcdui/Graphics",
                "drawTriangle" | "fillTriangle",
                "(IIIIIII)V",
            ) => {
                self.graphics_nokia_triangle(args, method.key.name == "fillTriangle")?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/lcdui/Graphics",
                "drawPixels",
                "([IZIIIIIIII)V" | "([SZIIIIIIII)V",
            ) => {
                self.graphics_nokia_draw_pixels(args)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/lcdui/Graphics", "getPixels", "([IIIIIIII)V" | "([SIIIIIII)V") => {
                self.graphics_nokia_get_pixels(args)?;
                CallOutcome::Return(None)
            }
            _ => return Ok(None),
        };
        Ok(Some(outcome))
    }
}
