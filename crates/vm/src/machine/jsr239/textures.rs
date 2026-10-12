//! GL texture names, parameters, upload and retained resource budgets.

use super::super::{
    EmuError, Handle, HeapValue, Limits, Machine, jsr239_texture_retained_bytes, m3g_bounded_image,
    m3g_non_negative_u32, type_error, vm_error,
};
use super::checkpoint::invalid_checkpoint;
use super::jsr239_word_bytes;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::machine) struct Jsr239Texture {
    pub(in crate::machine) image: Option<m3g::Texture2DState>,
    parameters: Jsr239TextureParameters,
}

impl Jsr239Texture {
    pub(super) fn validate_checkpoint(&self, limits: &Limits) -> Result<usize, EmuError> {
        if !matches!(
            self.parameters.min_filter,
            0x2600 | 0x2601 | 0x2700..=0x2703
        ) {
            return Err(invalid_checkpoint());
        }
        let Some(image) = &self.image else {
            return Ok(0);
        };
        let (width, height) = image.dimensions();
        if !width.is_power_of_two() || !height.is_power_of_two() {
            return Err(invalid_checkpoint());
        }
        m3g_bounded_image(width, height, limits.m3g_texture_pixels)
            .map_err(|_| invalid_checkpoint())?;
        image
            .validate_checkpoint()
            .map_err(|_| invalid_checkpoint())?;
        let bytes = jsr239_texture_retained_bytes(width, height).ok_or_else(invalid_checkpoint)?;
        if image.allocated_bytes() > bytes {
            return Err(invalid_checkpoint());
        }
        Ok(bytes)
    }

    pub(super) fn image_for_sampling(&self) -> Option<&m3g::Texture2DState> {
        let image = self.image.as_ref()?;
        // Only level zero is stored. ES 1.1 disables incomplete mipmapped
        // textures; a 1x1 base image is already a complete mip chain.
        if !matches!(self.parameters.min_filter, 0x2600 | 0x2601) && image.dimensions() != (1, 1) {
            return None;
        }
        Some(image)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct Jsr239TextureParameters {
    wrap_s: m3g::WrapMode,
    wrap_t: m3g::WrapMode,
    min_filter: i32,
    magnification_linear: bool,
}

impl Default for Jsr239TextureParameters {
    fn default() -> Self {
        Self {
            wrap_s: m3g::WrapMode::Repeat,
            wrap_t: m3g::WrapMode::Repeat,
            min_filter: 0x2702,
            magnification_linear: true,
        }
    }
}

impl Jsr239TextureParameters {
    fn apply(self, image: &mut m3g::Texture2DState) {
        image.set_wrapping(self.wrap_s, self.wrap_t);
        image.set_image_filters(
            matches!(self.min_filter, 0x2601 | 0x2701 | 0x2703),
            self.magnification_linear,
        );
    }
}

impl Machine<'_, '_> {
    pub(super) fn jsr239_reserve_texture_slots(
        &mut self,
        additional: usize,
    ) -> Result<(), EmuError> {
        if self
            .jsr239
            .textures
            .len()
            .checked_add(additional)
            .is_none_or(|count| count > self.limits.m3g_arena.objects || count > u32::MAX as usize)
        {
            return Err(vm_error(
                "resource-limit",
                "JSR-239 texture object limit reached",
            ));
        }
        self.jsr239
            .textures
            .try_reserve(additional)
            .map_err(|_| vm_error("resource-limit", "cannot reserve JSR-239 texture names"))
    }

    pub(super) fn jsr239_set_texture_parameter(
        &mut self,
        target: i32,
        parameter: i32,
        value: i32,
    ) -> Result<(), EmuError> {
        if target != 0x0de1 {
            return Err(vm_error(
                "illegal-argument-exception",
                "only GL_TEXTURE_2D texture parameters are supported",
            ));
        }
        let mut parameters = self
            .jsr239
            .textures
            .get(&self.jsr239.bound_texture)
            .map(|texture| texture.parameters)
            .unwrap_or_default();
        match parameter {
            0x2800 if matches!(value, 0x2600 | 0x2601) => {
                parameters.magnification_linear = value == 0x2601;
            }
            0x2801 if matches!(value, 0x2600 | 0x2601 | 0x2700..=0x2703) => {
                parameters.min_filter = value;
            }
            0x2802 | 0x2803 if matches!(value, 0x812f | 0x2901) => {
                let wrap = if value == 0x812f {
                    m3g::WrapMode::Clamp
                } else {
                    m3g::WrapMode::Repeat
                };
                if parameter == 0x2802 {
                    parameters.wrap_s = wrap;
                } else {
                    parameters.wrap_t = wrap;
                }
            }
            _ => {
                return Err(vm_error(
                    "illegal-argument-exception",
                    "unsupported OpenGL ES texture parameter or value",
                ));
            }
        }
        if !self
            .jsr239
            .textures
            .contains_key(&self.jsr239.bound_texture)
        {
            self.jsr239_reserve_texture_slots(1)?;
        }
        let texture = self
            .jsr239
            .textures
            .entry(self.jsr239.bound_texture)
            .or_default();
        texture.parameters = parameters;
        if let Some(image) = &mut texture.image {
            parameters.apply(image);
        }
        Ok(())
    }

    pub(super) fn jsr239_generate_texture_names(
        &mut self,
        count: i32,
        array: Handle,
        offset: i32,
    ) -> Result<(), EmuError> {
        let invalid_range = || {
            vm_error(
                "illegal-argument-exception",
                "texture name destination range is invalid",
            )
        };
        let count = usize::try_from(count).map_err(|_| invalid_range())?;
        let offset = usize::try_from(offset).map_err(|_| invalid_range())?;
        let end = offset.checked_add(count).ok_or_else(invalid_range)?;
        if end > self.graphics_int_array_mut(array)?.len() {
            return Err(invalid_range());
        }
        if count == 0 {
            return Ok(());
        }
        self.jsr239_reserve_texture_slots(count)?;
        let mut names = Vec::new();
        names
            .try_reserve_exact(count)
            .map_err(|_| vm_error("resource-limit", "cannot allocate JSR-239 texture names"))?;
        let mut next = self.jsr239.next_texture;
        let mut work = 0_usize;
        while names.len() < count {
            if work.is_multiple_of(1_024) && self.native_context.execution_cancelled() {
                return Err(vm_error(
                    "execution-cancelled",
                    "JSR-239 texture name generation cancelled",
                ));
            }
            work += 1;
            let texture = next;
            // Java int preserves all bits of an unsigned OpenGL name; zero is
            // reserved for the default texture, including after wrapping.
            next = next.wrapping_add(1);
            if texture != 0 && !self.jsr239.textures.contains_key(&texture) {
                names.push(texture);
            }
        }
        let destination = self.graphics_int_array_mut(array)?;
        for (slot, &name) in destination[offset..end].iter_mut().zip(&names) {
            *slot = HeapValue::Int(name);
        }
        self.jsr239.textures.extend(
            names
                .into_iter()
                .map(|name| (name, Jsr239Texture::default())),
        );
        self.jsr239.next_texture = next;
        Ok(())
    }

    pub(in crate::machine) fn jsr239_upload_texture(
        &mut self,
        width: i32,
        height: i32,
        buffer: Handle,
    ) -> Result<(), EmuError> {
        let width = m3g_non_negative_u32(width)?;
        let height = m3g_non_negative_u32(height)?;
        if !width.is_power_of_two() || !height.is_power_of_two() {
            return Err(vm_error(
                "illegal-argument-exception",
                "OpenGL ES texture dimensions must be positive powers of two",
            ));
        }
        m3g_bounded_image(width, height, self.limits.m3g_texture_pixels)?;
        let retained_bytes = jsr239_texture_retained_bytes(width, height).ok_or_else(|| {
            vm_error(
                "illegal-argument-exception",
                "JSR-239 texture dimensions are invalid",
            )
        })?;
        let previous_dimensions = self
            .jsr239
            .textures
            .get(&self.jsr239.bound_texture)
            .and_then(|texture| texture.image.as_ref())
            .map(m3g::Texture2DState::dimensions);
        let previous_bytes = previous_dimensions
            .and_then(|(width, height)| jsr239_texture_retained_bytes(width, height))
            .unwrap_or(0);
        if !self
            .jsr239
            .textures
            .contains_key(&self.jsr239.bound_texture)
        {
            self.jsr239_reserve_texture_slots(1)?;
        }
        let projected_bytes = self
            .jsr239
            .texture_bytes
            .checked_sub(previous_bytes)
            .and_then(|bytes| bytes.checked_add(retained_bytes))
            .filter(|bytes| *bytes <= self.limits.m3g_arena.bytes)
            .ok_or_else(|| {
                vm_error(
                    "resource-limit",
                    "JSR-239 textures exceed the native memory budget",
                )
            })?;
        let count =
            usize::try_from(u64::from(width) * u64::from(height)).map_err(|_| type_error())?;
        let bytes = self.nio_buffer_bytes(buffer, 1)?;
        if count > bytes.len() / 4 {
            return Err(vm_error(
                "illegal-argument-exception",
                "texture exceeds the remaining NIO buffer",
            ));
        }
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(count).map_err(|_| {
            vm_error(
                "resource-limit",
                "cannot allocate the JSR-239 texture pixels",
            )
        })?;
        for (index, bytes) in bytes.chunks_exact(4).take(count).enumerate() {
            if index.is_multiple_of(1_024) && self.native_context.execution_cancelled() {
                return Err(vm_error(
                    "execution-cancelled",
                    "JSR-239 texture upload cancelled",
                ));
            }
            let [red, green, blue, alpha] = jsr239_word_bytes(bytes)?;
            pixels.push(u32::from_be_bytes([alpha, red, green, blue]));
        }
        let image = m3g::Image2DState::from_shared_argb(width, height, pixels.into())?;
        let mut texture = m3g::Texture2DState::new(image);
        texture.set_blend_function(m3g::BlendFunction::Modulate);
        let state = self
            .jsr239
            .textures
            .entry(self.jsr239.bound_texture)
            .or_default();
        state.parameters.apply(&mut texture);
        state.image = Some(texture);
        self.jsr239.texture_bytes = projected_bytes;
        Ok(())
    }
}
