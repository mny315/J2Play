//! Retained NIO array pointers and GL triangle-strip rendering.

use super::super::{
    Allocation, ArrayKind, EmuError, Handle, HeapValue, Machine, heap_error, m3g_non_negative_u32,
    type_error, vm_error,
};
use super::{Jsr239Texture, jsr239_word_bytes};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(in crate::machine) struct Jsr239FloatPointer {
    pub(in crate::machine) buffer: Handle,
    pub(in crate::machine) bytes: std::ops::Range<usize>,
}

impl Jsr239FloatPointer {
    pub(in crate::machine) fn components(&self) -> usize {
        self.bytes.len() / 4
    }
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn jsr239_float_pointer(
        &self,
        buffer: Handle,
        size: i32,
        stride: i32,
    ) -> Result<Jsr239FloatPointer, EmuError> {
        if size != 2 || stride != 0 {
            return Err(vm_error(
                "illegal-argument-exception",
                "only packed two-component float pointers are supported",
            ));
        }
        if self.object_class(buffer)? != "java/nio/FloatBuffer" {
            return Err(vm_error(
                "illegal-argument-exception",
                "GL_FLOAT requires a FloatBuffer",
            ));
        }
        let (_, bytes) = self.nio_buffer_range(buffer, 4)?;
        Ok(Jsr239FloatPointer { buffer, bytes })
    }

    pub(in crate::machine) fn jsr239_float_pointer_words(
        &self,
        pointer: &Jsr239FloatPointer,
        components: std::ops::Range<usize>,
    ) -> Result<&[[HeapValue; 4]], EmuError> {
        if components.start > components.end || components.end > pointer.components() {
            return Err(vm_error(
                "illegal-argument-exception",
                "JSR-239 draw exceeds the active array pointer",
            ));
        }
        let array = self.graphics_reference_field(pointer.buffer, "java/nio/Buffer.bytes:[B")?;
        let Allocation::Array {
            kind: ArrayKind::Byte,
            elements,
        } = self.heap.managed.get(array).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let start = pointer.bytes.start + components.start * 4;
        let end = pointer.bytes.start + components.end * 4;
        let bytes = elements.get(start..end).ok_or_else(type_error)?;
        if self.native_context.execution_cancelled() {
            return Err(vm_error(
                "execution-cancelled",
                "JSR-239 float pointer access cancelled",
            ));
        }
        // The validated component range selects complete words. The guest
        // heap stays unchanged until the completed framebuffer is published.
        Ok(bytes.as_chunks::<4>().0)
    }

    pub(in crate::machine) fn jsr239_draw_arrays(
        &mut self,
        mode: i32,
        first: i32,
        count: i32,
    ) -> Result<(), EmuError> {
        if mode != 5 || first < 0 || count < 0 {
            return Err(vm_error(
                "illegal-argument-exception",
                "only GL_TRIANGLE_STRIP draws with non-negative ranges are supported",
            ));
        }
        if count == 0 || !self.jsr239.vertex_array_enabled {
            return Ok(());
        }
        let first = usize::try_from(first).map_err(|_| type_error())?;
        let count = usize::try_from(count).map_err(|_| type_error())?;
        let end = first.checked_add(count).ok_or_else(|| {
            vm_error(
                "illegal-argument-exception",
                "JSR-239 vertex range overflows",
            )
        })?;
        let required_components = end.checked_mul(2).ok_or_else(|| {
            vm_error(
                "illegal-argument-exception",
                "JSR-239 vertex component range overflows",
            )
        })?;
        if count > self.limits.m3g_vertices {
            return Err(vm_error(
                "resource-limit",
                "JSR-239 draw exceeds the active vertex budget",
            ));
        }
        let pointer = self.jsr239.vertex_pointer.as_ref().ok_or_else(|| {
            vm_error(
                "illegal-argument-exception",
                "JSR-239 draw has no vertex pointer",
            )
        })?;
        let positions = self.jsr239_float_pointer_words(pointer, first * 2..required_components)?;
        let texture_coordinates = if self.jsr239.texture_array_enabled {
            let pointer = self.jsr239.texture_pointer.as_ref().ok_or_else(|| {
                vm_error(
                    "illegal-argument-exception",
                    "JSR-239 draw has no texture coordinate pointer",
                )
            })?;
            self.jsr239_float_pointer_words(pointer, first * 2..required_components)?
        } else {
            &[]
        };
        if count < 3 {
            return Ok(());
        }
        let target = self
            .jsr239
            .target
            .ok_or_else(|| vm_error("illegal-state-exception", "no current EGL window surface"))?;
        let framebuffer = self.graphics_reference_field(
            target,
            "javax/microedition/lcdui/Canvas.framebuffer:Ljavax/microedition/lcdui/Image;",
        )?;
        let pixels =
            self.graphics_reference_field(framebuffer, "javax/microedition/lcdui/Image.pixels:[I")?;
        let width = m3g_non_negative_u32(
            self.graphics_int_field(framebuffer, "javax/microedition/lcdui/Image.width:I")?,
        )?;
        let height = m3g_non_negative_u32(
            self.graphics_int_field(framebuffer, "javax/microedition/lcdui/Image.height:I")?,
        )?;
        let original = self
            .graphics_int_array_snapshot(pixels)?
            .into_iter()
            .map(i32::cast_unsigned)
            .collect::<Vec<_>>();
        let mut renderer =
            m3g::SoftwareRenderer::from_pixels(width, height, original, self.limits.m3g_render)?;
        renderer.set_compositing_mode(
            false,
            false,
            true,
            true,
            if self.jsr239.blending {
                self.jsr239.blend_function
            } else {
                m3g::FrameBlend::Replace
            },
            0.0,
            0.0,
        )?;
        renderer.set_cull_mode(m3g::CullMode::None);
        let mut texture = if self.jsr239.texture_enabled {
            self.jsr239
                .textures
                .get(&self.jsr239.bound_texture)
                .and_then(Jsr239Texture::image_for_sampling)
                .cloned()
        } else {
            None
        };
        if let Some(texture) = &mut texture {
            texture.set_blend_function(self.jsr239.texture_environment);
        }
        renderer.set_texture(texture);

        let transform = self.jsr239.projection.multiplied(self.jsr239.model_view);
        // A strip needs only its preceding edge, regardless of the draw size.
        let mut previous = [m3g::Vertex::default(); 2];
        for (index, position) in positions.chunks_exact(2).enumerate() {
            if index.is_multiple_of(1_024) && self.native_context.execution_cancelled() {
                return Err(vm_error("execution-cancelled", "JSR-239 draw cancelled"));
            }
            let component = index * 2;
            let [x, y] = float_pair(position)?;
            let [s, t] = texture_coordinates
                .get(component..component + 2)
                .map_or(Ok([0.0; 2]), float_pair)?;
            let vertex = m3g::Vertex::textured(
                transform.transform(m3g::Vec4::new(x, y, 0.0, 1.0)),
                self.jsr239.color,
                s,
                t,
            );
            if index >= 2 {
                let edge = if index % 2 == 0 {
                    previous
                } else {
                    [previous[1], previous[0]]
                };
                renderer.draw_triangle([edge[0], edge[1], vertex])?;
            }
            previous = [previous[1], vertex];
        }
        let destination = self.graphics_int_array_mut(pixels)?;
        for (destination, source) in destination.iter_mut().zip(renderer.pixels()) {
            *destination = HeapValue::Int(source.cast_signed());
        }
        Ok(())
    }
}

fn float_pair(words: &[[HeapValue; 4]]) -> Result<[f32; 2], EmuError> {
    let [x, y] = words else {
        return Err(type_error());
    };
    Ok([
        f32::from_ne_bytes(jsr239_word_bytes(x)?),
        f32::from_ne_bytes(jsr239_word_bytes(y)?),
    ])
}
