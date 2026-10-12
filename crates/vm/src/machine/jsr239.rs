//! JSR-239 GL command dispatch and suite-owned state.

use super::{
    CallOutcome, EmuError, Handle, HeapValue, Machine, Method, Value, display_key, float_argument,
    int_argument, jsr239_argb, jsr239_frustum, optional_reference_argument, reference_argument,
    type_error, vm_error,
};
use std::collections::HashMap;

mod checkpoint;
mod egl;
mod rendering;
mod textures;

use rendering::Jsr239FloatPointer;
use textures::Jsr239Texture;

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Jsr239State {
    pub(super) target: Option<Handle>,
    pub(super) matrix_mode: i32,
    pub(super) projection: m3g::Mat4,
    pub(super) model_view: m3g::Mat4,
    pub(super) model_view_stack: Vec<m3g::Mat4>,
    pub(super) vertex_pointer: Option<Jsr239FloatPointer>,
    pub(super) texture_pointer: Option<Jsr239FloatPointer>,
    pub(super) vertex_array_enabled: bool,
    pub(super) texture_array_enabled: bool,
    pub(super) texture_enabled: bool,
    pub(super) texture_environment: m3g::BlendFunction,
    // Generated or bound names remain reserved before their first image upload.
    pub(super) textures: HashMap<i32, Jsr239Texture>,
    pub(super) texture_bytes: usize,
    pub(super) bound_texture: i32,
    pub(super) next_texture: i32,
    pub(super) color: u32,
    pub(super) blending: bool,
    pub(super) blend_function: m3g::FrameBlend,
    pub(super) egl: Option<Handle>,
    pub(super) gl: Option<Handle>,
}

impl Default for Jsr239State {
    fn default() -> Self {
        Self {
            target: None,
            matrix_mode: 0x1700,
            projection: m3g::Mat4::IDENTITY,
            model_view: m3g::Mat4::IDENTITY,
            model_view_stack: Vec::new(),
            vertex_pointer: None,
            texture_pointer: None,
            vertex_array_enabled: false,
            texture_array_enabled: false,
            texture_enabled: false,
            texture_environment: m3g::BlendFunction::Modulate,
            textures: HashMap::new(),
            texture_bytes: 0,
            bound_texture: 0,
            next_texture: 1,
            color: 0xffff_ffff,
            blending: false,
            blend_function: m3g::FrameBlend::Replace,
            egl: None,
            gl: None,
        }
    }
}

impl Jsr239State {
    pub(super) fn append_roots(&self, roots: &mut Vec<Handle>) {
        roots.extend(self.target);
        roots.extend(self.egl);
        roots.extend(self.gl);
        roots.extend(self.vertex_pointer.as_ref().map(|pointer| pointer.buffer));
        roots.extend(self.texture_pointer.as_ref().map(|pointer| pointer.buffer));
    }
}

fn jsr239_word_bytes(values: &[HeapValue]) -> Result<[u8; 4], EmuError> {
    let [
        HeapValue::Int(a),
        HeapValue::Int(b),
        HeapValue::Int(c),
        HeapValue::Int(d),
    ] = values
    else {
        return Err(type_error());
    };
    Ok([*a as u8, *b as u8, *c as u8, *d as u8])
}

impl Machine<'_, '_> {
    #[allow(clippy::too_many_lines)]
    pub(super) fn invoke_jsr239_native(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let class = method.key.class.as_str();
        let name = method.key.name.as_str();
        let descriptor = method.key.descriptor.as_str();
        if class.starts_with("javax/microedition/khronos/egl/") {
            return self.invoke_egl_native(name, descriptor, args);
        }
        if !class.starts_with("javax/microedition/khronos/opengles/") {
            return Err(vm_error("method-not-found", display_key(&method.key)));
        }

        match (name, descriptor) {
            ("glBindTexture", "(II)V") => {
                if int_argument(args, 1)? != 0x0de1 {
                    return Err(vm_error(
                        "illegal-argument-exception",
                        "only GL_TEXTURE_2D texture bindings are supported",
                    ));
                }
                let texture = int_argument(args, 2)?;
                if texture != 0 && !self.jsr239.textures.contains_key(&texture) {
                    self.jsr239_reserve_texture_slots(1)?;
                    self.jsr239
                        .textures
                        .insert(texture, Jsr239Texture::default());
                }
                self.jsr239.bound_texture = texture;
            }
            ("glBlendFunc", "(II)V") => {
                let source = int_argument(args, 1)?;
                let destination = int_argument(args, 2)?;
                self.jsr239.blend_function = match (source, destination) {
                    (1, 0) => m3g::FrameBlend::Replace,
                    (0x0302, 0x0303) => m3g::FrameBlend::Alpha,
                    _ => {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("unsupported OpenGL ES blend function"),
                        );
                    }
                };
            }
            ("glColor4f", "(FFFF)V") => {
                self.jsr239.color = jsr239_argb(
                    float_argument(args, 1)?,
                    float_argument(args, 2)?,
                    float_argument(args, 3)?,
                    float_argument(args, 4)?,
                )?;
            }
            ("glEnable" | "glDisable", "(I)V") => {
                let enabled = name == "glEnable";
                match int_argument(args, 1)? {
                    0x0be2 => self.jsr239.blending = enabled,
                    0x0de1 => self.jsr239.texture_enabled = enabled,
                    _ => {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "unsupported OpenGL ES capability",
                        ));
                    }
                }
            }
            ("glDrawArrays", "(III)V") => {
                self.jsr239_draw_arrays(
                    int_argument(args, 1)?,
                    int_argument(args, 2)?,
                    int_argument(args, 3)?,
                )?;
            }
            ("glEnableClientState" | "glDisableClientState", "(I)V") => {
                let enabled = name == "glEnableClientState";
                match int_argument(args, 1)? {
                    0x8074 => self.jsr239.vertex_array_enabled = enabled,
                    0x8078 => self.jsr239.texture_array_enabled = enabled,
                    _ => {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "unsupported OpenGL ES client array",
                        ));
                    }
                }
            }
            ("glTexEnvi", "(III)V") => {
                if int_argument(args, 1)? != 0x2300 || int_argument(args, 2)? != 0x2200 {
                    return Err(vm_error(
                        "illegal-argument-exception",
                        "only GL_TEXTURE_ENV_MODE is supported",
                    ));
                }
                self.jsr239.texture_environment = match int_argument(args, 3)? {
                    0x0104 => m3g::BlendFunction::Add,
                    0x0be2 => m3g::BlendFunction::Blend,
                    0x1e01 => m3g::BlendFunction::Replace,
                    0x2100 => m3g::BlendFunction::Modulate,
                    0x2101 => m3g::BlendFunction::Decal,
                    _ => {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "unsupported OpenGL ES texture environment mode",
                        ));
                    }
                };
            }
            ("glFrustumf", "(FFFFFF)V") => {
                let left = float_argument(args, 1)?;
                let right = float_argument(args, 2)?;
                let bottom = float_argument(args, 3)?;
                let top = float_argument(args, 4)?;
                let near = float_argument(args, 5)?;
                let far = float_argument(args, 6)?;
                let frustum = jsr239_frustum(left, right, bottom, top, near, far)?;
                let matrix = self.jsr239_current_matrix_mut();
                *matrix = matrix.multiplied(frustum);
            }
            ("glGenTextures", "(I[II)V") => {
                let count = int_argument(args, 1)?;
                let array = optional_reference_argument(args, 2)?.ok_or_else(|| {
                    vm_error(
                        "illegal-argument-exception",
                        "texture name destination must not be null",
                    )
                })?;
                let offset = int_argument(args, 3)?;
                self.jsr239_generate_texture_names(count, array, offset)?;
            }
            ("glLoadIdentity", "()V") => {
                *self.jsr239_current_matrix_mut() = m3g::Mat4::IDENTITY;
            }
            ("glMatrixMode", "(I)V") => {
                let mode = int_argument(args, 1)?;
                if mode != 0x1700 && mode != 0x1701 {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("unsupported OpenGL ES matrix mode"),
                    );
                }
                self.jsr239.matrix_mode = mode;
            }
            ("glPopMatrix", "()V") => {
                if self.jsr239.matrix_mode != 0x1700 {
                    return self.thread_exception(
                        "java/lang/IllegalStateException",
                        Some("only the model-view matrix stack is available"),
                    );
                }
                self.jsr239.model_view = self.jsr239.model_view_stack.pop().ok_or_else(|| {
                    vm_error(
                        "illegal-state-exception",
                        "OpenGL ES matrix stack underflow",
                    )
                })?;
            }
            ("glPushMatrix", "()V") => {
                if self.jsr239.matrix_mode != 0x1700 || self.jsr239.model_view_stack.len() >= 64 {
                    return self.thread_exception(
                        "java/lang/IllegalStateException",
                        Some("OpenGL ES matrix stack unavailable or full"),
                    );
                }
                self.jsr239.model_view_stack.push(self.jsr239.model_view);
            }
            ("glScalef", "(FFF)V") => {
                let scale = m3g::Mat4::scale(
                    float_argument(args, 1)?,
                    float_argument(args, 2)?,
                    float_argument(args, 3)?,
                )?;
                let matrix = self.jsr239_current_matrix_mut();
                *matrix = matrix.multiplied(scale);
            }
            ("glTexCoordPointer" | "glVertexPointer", "(IIILjava/nio/Buffer;)V") => {
                let size = int_argument(args, 1)?;
                if int_argument(args, 2)? != 0x1406 {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("only GL_FLOAT array pointers are supported"),
                    );
                }
                let stride = int_argument(args, 3)?;
                let buffer = reference_argument(args, 4)?;
                let pointer = Some(self.jsr239_float_pointer(buffer, size, stride)?);
                if name == "glVertexPointer" {
                    self.jsr239.vertex_pointer = pointer;
                } else {
                    self.jsr239.texture_pointer = pointer;
                }
            }
            ("glTexImage2D", "(IIIIIIIILjava/nio/Buffer;)V") => {
                if int_argument(args, 1)? != 0x0de1
                    || int_argument(args, 2)? != 0
                    || int_argument(args, 3)? != 0x1908
                    || int_argument(args, 6)? != 0
                    || int_argument(args, 7)? != 0x1908
                    || int_argument(args, 8)? != 0x1401
                {
                    return Err(vm_error(
                        "illegal-argument-exception",
                        "only level-zero GL_RGBA/GL_UNSIGNED_BYTE textures without a border are supported",
                    ));
                }
                self.jsr239_upload_texture(
                    int_argument(args, 4)?,
                    int_argument(args, 5)?,
                    reference_argument(args, 9)?,
                )?;
            }
            ("glTexParameteri", "(III)V") => {
                self.jsr239_set_texture_parameter(
                    int_argument(args, 1)?,
                    int_argument(args, 2)?,
                    int_argument(args, 3)?,
                )?;
            }
            ("glTranslatef", "(FFF)V") => {
                let translation = m3g::Mat4::translation(
                    float_argument(args, 1)?,
                    float_argument(args, 2)?,
                    float_argument(args, 3)?,
                )?;
                let matrix = self.jsr239_current_matrix_mut();
                *matrix = matrix.multiplied(translation);
            }
            _ => return Err(vm_error("method-not-found", display_key(&method.key))),
        }
        Ok(CallOutcome::Return(None))
    }

    pub(super) fn jsr239_current_matrix_mut(&mut self) -> &mut m3g::Mat4 {
        if self.jsr239.matrix_mode == 0x1701 {
            &mut self.jsr239.projection
        } else {
            &mut self.jsr239.model_view
        }
    }
}
