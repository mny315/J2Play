//! Profile-selected VM capacity, display geometry and backend limits.

use super::{EmuError, vm_error};

pub(super) const MAX_LCD_PIXELS: usize = 4_194_304;

#[derive(Clone, Debug)]
// These booleans are independent device capabilities, not mutually exclusive state.
#[allow(clippy::struct_excessive_bools)]
pub struct Limits {
    pub max_instructions: u64,
    pub max_frames: usize,
    pub max_threads: usize,
    pub max_stack_slots: usize,
    pub max_runtime_bytes: usize,
    pub max_heap_bytes: usize,
    /// Full-screen Canvas width selected by the device profile.
    pub lcd_width: u32,
    /// Full-screen Canvas height selected by the device profile.
    pub lcd_height: u32,
    /// Drawable Canvas width before full-screen mode is requested.
    pub lcd_normal_width: u32,
    /// Drawable Canvas height before full-screen mode is requested.
    pub lcd_normal_height: u32,
    /// Whether the active device profile accepts full-screen Canvas requests.
    pub lcd_fullscreen_available: bool,
    pub m3g_arena: m3g::ArenaLimits,
    pub m3g_loader: m3g::LoaderLimits,
    pub m3g_render: m3g::RenderLimits,
    /// Maximum vertex preparation/transformation work per native graphics operation.
    pub m3g_vertices: usize,
    pub m3g_texture_pixels: usize,
    /// Boolean capabilities advertised by the active device profile.
    pub m3g_support_antialiasing: bool,
    pub m3g_support_true_color: bool,
    pub m3g_support_dithering: bool,
    pub m3g_support_mipmapping: bool,
    pub m3g_support_perspective_correction: bool,
    pub m3g_support_local_camera_lighting: bool,
    /// Maximum simultaneous lights selected by the active device profile.
    pub m3g_max_lights: usize,
    /// Viewport properties advertised by the active device profile.
    pub m3g_max_viewport_width: u32,
    pub m3g_max_viewport_height: u32,
    pub m3g_max_viewport_dimension: u32,
    /// Number of texture units advertised by the active device profile.
    pub m3g_num_texture_units: usize,
    /// Device limit advertised through `Graphics3D.getProperties()`.
    pub m3g_max_texture_dimension: u32,
    pub m3g_max_sprite_crop_dimension: u32,
    pub m3g_max_transforms_per_vertex: u32,
    /// Optional profile-selected acceptance limit for cross-device content.
    /// This never changes the device property visible to guest code.
    pub m3g_compatibility_max_texture_dimension: u32,
    pub m3g_keyframes: usize,
    pub m3g_graph_depth: usize,
    pub micro3d_objects: usize,
    pub micro3d_bytes: usize,
    pub micro3d_loader: micro3d::LoaderLimits,
    /// Collect a bounded method profile without the instruction trace.
    pub profile_methods: bool,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_instructions: 1_000_000,
            // The interpreter is currently recursive, so the Java frame
            // bound still protects the host thread stack. 32 frames turned
            // out to be below call depths reached by deployed software. Keep
            // a conservative host-stack guard while allowing normal MIDlet
            // nesting and recursive resource decoders.
            max_frames: 128,
            max_threads: 256,
            max_stack_slots: 65_536,
            // Parsed bytecode, decoded instructions, constant pools and link
            // caches are host-side emulator metadata, not the emulated Java
            // heap. Large late-generation MIDlets can exceed 16 MiB here
            // while staying within the handset heap, so keep a bounded but
            // realistic host representation budget.
            max_runtime_bytes: 64 * 1024 * 1024,
            max_heap_bytes: 16 * 1024 * 1024,
            lcd_width: 240,
            lcd_height: 320,
            lcd_normal_width: 240,
            lcd_normal_height: 320,
            lcd_fullscreen_available: true,
            m3g_arena: m3g::ArenaLimits::default(),
            m3g_loader: m3g::LoaderLimits::default(),
            m3g_render: m3g::RenderLimits::default(),
            m3g_vertices: 262_144,
            m3g_texture_pixels: 1_048_576,
            m3g_support_antialiasing: false,
            m3g_support_true_color: false,
            m3g_support_dithering: false,
            m3g_support_mipmapping: true,
            m3g_support_perspective_correction: true,
            m3g_support_local_camera_lighting: false,
            m3g_max_lights: m3g::MAX_LIGHTS,
            m3g_max_viewport_width: 1_024,
            m3g_max_viewport_height: 1_024,
            m3g_max_viewport_dimension: 1_024,
            m3g_num_texture_units: m3g::MAX_TEXTURE_UNITS,
            m3g_max_texture_dimension: 256,
            m3g_max_sprite_crop_dimension: 1_024,
            m3g_max_transforms_per_vertex: 3,
            m3g_compatibility_max_texture_dimension: 256,
            m3g_keyframes: 262_144,
            m3g_graph_depth: 4_096,
            micro3d_objects: 16_384,
            micro3d_bytes: 8 * 1024 * 1024,
            micro3d_loader: micro3d::LoaderLimits::default(),
            profile_methods: false,
        }
    }
}

impl Limits {
    /// Validates limits that affect eager host allocations and renderer setup.
    ///
    /// # Errors
    ///
    /// Returns `invalid-limits` when a display or `Micro3D` configuration cannot
    /// be represented safely by the runtime.
    pub fn validate(&self) -> Result<(), EmuError> {
        let fullscreen_pixels =
            validate_lcd_dimensions(self.lcd_width, self.lcd_height, "full-screen")?;
        let normal_pixels = validate_lcd_dimensions(
            self.lcd_normal_width,
            self.lcd_normal_height,
            "normal Canvas",
        )?;
        if fullscreen_pixels == normal_pixels
            && (self.lcd_width, self.lcd_height) != (self.lcd_normal_width, self.lcd_normal_height)
        {
            return Err(vm_error(
                "invalid-limits",
                "Canvas modes with different dimensions must not have the same pixel count",
            ));
        }
        if self.micro3d_objects == 0 || self.micro3d_bytes == 0 {
            return Err(vm_error(
                "invalid-limits",
                "Micro3D object and byte limits must be non-zero",
            ));
        }
        if self.m3g_max_lights == 0
            || self.m3g_max_lights > m3g::MAX_LIGHTS
            || self.m3g_num_texture_units == 0
            || self.m3g_num_texture_units > m3g::MAX_TEXTURE_UNITS
            || self.m3g_render.max_viewport_width == 0
            || self.m3g_render.max_viewport_height == 0
            || [
                self.m3g_max_viewport_width,
                self.m3g_max_viewport_height,
                self.m3g_max_viewport_dimension,
                self.m3g_max_texture_dimension,
                self.m3g_max_sprite_crop_dimension,
                self.m3g_max_transforms_per_vertex,
            ]
            .into_iter()
            .any(|value| value == 0 || value > i32::MAX.cast_unsigned())
        {
            return Err(vm_error(
                "invalid-limits",
                "M3G profile capabilities exceed the software backend",
            ));
        }
        Ok(())
    }
}

fn validate_lcd_dimensions(width: u32, height: u32, label: &str) -> Result<usize, EmuError> {
    if width == 0
        || height == 0
        || width > i32::MAX.cast_unsigned()
        || height > i32::MAX.cast_unsigned()
    {
        return Err(vm_error(
            "invalid-limits",
            format!("{label} dimensions must be non-zero Java int values"),
        ));
    }
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .filter(|pixels| *pixels <= MAX_LCD_PIXELS)
        .ok_or_else(|| {
            vm_error(
                "invalid-limits",
                format!("{label} dimensions exceed the {MAX_LCD_PIXELS}-pixel limit"),
            )
        })?;
    Ok(pixels)
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/limits.rs"]
mod tests;
