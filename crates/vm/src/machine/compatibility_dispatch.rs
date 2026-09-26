//! Dispatch of shared platform API intrinsics; application methods use bytecode.

use super::graphics_dispatch::TextSource;
use super::{
    CallOutcome, EmuError, MAIN_THREAD_ID, Machine, Method, Value, heap_error, int_argument,
    reference_argument, type_error,
};

mod cldc;

impl Machine<'_, '_> {
    pub(super) fn invoke_compatibility_intrinsic(
        &mut self,
        method: &Method,
        args: &[Value],
        _depth: usize,
    ) -> Result<Option<CallOutcome>, EmuError> {
        if let Some(outcome) = self.invoke_lcd_ui_graphics_public_intrinsic(method, args)? {
            return Ok(Some(outcome));
        }
        if let Some(outcome) = Self::invoke_lcd_ui_graphics_scalar_intrinsic(method, args)? {
            return Ok(Some(outcome));
        }
        if !method.is_static
            && method.key.class == "javax/microedition/lcdui/Display"
            && method.key.name == "__shouldDeferCurrent"
            && method.key.descriptor == "(Ljavax/microedition/lcdui/Displayable;)Z"
        {
            let defer = match args.get(1) {
                Some(Value::Reference(Some(target))) => {
                    self.heap.managed.get(*target).map_err(heap_error)?;
                    // Defer showNotify until construction/startup completes.
                    // Worker requests use a later display turn so isShown()
                    // still observes the previous state on the calling thread.
                    self.scheduler.current_thread != MAIN_THREAD_ID
                        || self.execution.call_stack.iter().any(|frame| {
                            let key = self.program.active_stack_key(frame);
                            key.name == "<init>"
                                || (key.name == "startApp" && key.descriptor == "()V")
                        })
                }
                Some(Value::Reference(None)) => false,
                _ => return Err(type_error()),
            };
            return Ok(Some(CallOutcome::Return(Some(Value::Int(i32::from(
                defer,
            ))))));
        }
        if let Some(outcome) = self.invoke_cldc_intrinsic(method, args)? {
            return Ok(Some(outcome));
        }

        if !method.is_static
            && method.key.class == "javax/microedition/lcdui/Canvas"
            && method.key.name == "repaint"
            && matches!(method.key.descriptor.as_str(), "()V" | "(IIII)V")
        {
            self.canvas_repaint(args)?;
            return Ok(Some(CallOutcome::Return(None)));
        }

        if !method.is_static && method.key.class == "javax/microedition/lcdui/Font" {
            if method.key.name == "getHeight" && method.key.descriptor == "()I" {
                let font = reference_argument(args, 0)?;
                let size = self.graphics_int_field(font, "javax/microedition/lcdui/Font.size:I")?;
                let (height, _) = self.active_lcd_ui_font_dimensions(size);
                return Ok(Some(CallOutcome::Return(Some(Value::Int(height)))));
            }
            if method.key.name == "charWidth" && method.key.descriptor == "(C)I" {
                let font = reference_argument(args, 0)?;
                let codepoint = int_argument(args, 1)? as u16;
                if let Some(width) = self.unicode_fallback_char_width(font, codepoint)? {
                    return Ok(Some(CallOutcome::Return(Some(Value::Int(width)))));
                }
            }
        }

        if !method.is_static && method.key.class == "javax/microedition/lcdui/Graphics" {
            if method.key.name == "drawLine" && method.key.descriptor == "(IIII)V" {
                self.graphics_draw_line(args)?;
                return Ok(Some(CallOutcome::Return(None)));
            } else if matches!(method.key.name.as_str(), "drawArc" | "fillArc")
                && method.key.descriptor == "(IIIIII)V"
            {
                self.graphics_draw_arc(args, method.key.name == "fillArc")?;
                return Ok(Some(CallOutcome::Return(None)));
            } else if method.key.name == "drawString"
                && method.key.descriptor == "(Ljava/lang/String;III)V"
            {
                let graphics = reference_argument(args, 0)?;
                let string = reference_argument(args, 1)?;
                self.graphics_unicode_draw_text(
                    graphics,
                    TextSource::JavaString(string),
                    int_argument(args, 2)?,
                    int_argument(args, 3)?,
                    int_argument(args, 4)?,
                )?;
                return Ok(Some(CallOutcome::Return(None)));
            } else if method.key.name == "drawChar" && method.key.descriptor == "(CIII)V" {
                let graphics = reference_argument(args, 0)?;
                let codepoint = int_argument(args, 1)? as u16;
                self.graphics_unicode_draw_text(
                    graphics,
                    TextSource::Utf16(&[codepoint]),
                    int_argument(args, 2)?,
                    int_argument(args, 3)?,
                    int_argument(args, 4)?,
                )?;
                return Ok(Some(CallOutcome::Return(None)));
            }
        }

        if !method.is_static
            && method.key.class == "javax/microedition/lcdui/game/TiledLayer"
            && method.key.name == "paint"
            && method.key.descriptor == "(Ljavax/microedition/lcdui/Graphics;)V"
            && self.graphics_tiled_layer_paint(args)?
        {
            return Ok(Some(CallOutcome::Return(None)));
        }

        Ok(None)
    }
}
