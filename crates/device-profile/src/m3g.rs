//! JSR-184 hardware properties, compatibility allowances and runtime budgets.

use serde::Deserialize;

use crate::Evidence;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3gProfile {
    pub(super) version: Evidence<String>,
    #[serde(default)]
    pub(super) compatibility: M3gCompatibility,
    pub(super) properties: M3gProperties,
    pub(super) budgets: M3gBudgets,
}

impl M3gProfile {
    #[must_use]
    pub const fn version(&self) -> &Evidence<String> {
        &self.version
    }
    #[must_use]
    pub const fn compatibility(&self) -> M3gCompatibility {
        self.compatibility
    }
    #[must_use]
    pub const fn properties(&self) -> &M3gProperties {
        &self.properties
    }
    #[must_use]
    pub const fn budgets(&self) -> &M3gBudgets {
        &self.budgets
    }
}

/// Explicit emulator compatibility allowances selected by a device profile.
///
/// These values do not change the hardware properties advertised to guest
/// code. A missing allowance keeps the profile strictly device-accurate.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct M3gCompatibility {
    pub(super) max_texture_dimension: Option<u32>,
}

impl M3gCompatibility {
    #[must_use]
    pub const fn max_texture_dimension(self) -> Option<u32> {
        self.max_texture_dimension
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3gProperties {
    pub(super) support_antialiasing: Evidence<bool>,
    pub(super) support_true_color: Evidence<bool>,
    pub(super) support_dithering: Evidence<bool>,
    pub(super) support_mipmapping: Evidence<bool>,
    pub(super) support_perspective_correction: Evidence<bool>,
    pub(super) support_local_camera_lighting: Evidence<bool>,
    pub(super) max_lights: Evidence<u32>,
    pub(super) max_viewport_width: Evidence<u32>,
    pub(super) max_viewport_height: Evidence<u32>,
    pub(super) max_viewport_dimension: Evidence<u32>,
    pub(super) max_texture_dimension: Evidence<u32>,
    pub(super) max_sprite_crop_dimension: Evidence<u32>,
    pub(super) max_transforms_per_vertex: Evidence<u32>,
    pub(super) num_texture_units: Evidence<u32>,
    pub(super) depth_bits: Evidence<u32>,
    pub(super) color_format: Evidence<String>,
}

macro_rules! m3g_property_getter {
    ($name:ident, $kind:ty) => {
        #[must_use]
        pub const fn $name(&self) -> &Evidence<$kind> {
            &self.$name
        }
    };
}

impl M3gProperties {
    m3g_property_getter!(support_antialiasing, bool);
    m3g_property_getter!(support_true_color, bool);
    m3g_property_getter!(support_dithering, bool);
    m3g_property_getter!(support_mipmapping, bool);
    m3g_property_getter!(support_perspective_correction, bool);
    m3g_property_getter!(support_local_camera_lighting, bool);
    m3g_property_getter!(max_lights, u32);
    m3g_property_getter!(max_viewport_width, u32);
    m3g_property_getter!(max_viewport_height, u32);
    m3g_property_getter!(max_viewport_dimension, u32);
    m3g_property_getter!(max_texture_dimension, u32);
    m3g_property_getter!(max_sprite_crop_dimension, u32);
    m3g_property_getter!(max_transforms_per_vertex, u32);
    m3g_property_getter!(num_texture_units, u32);
    m3g_property_getter!(depth_bits, u32);
    m3g_property_getter!(color_format, String);
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct M3gBudgets {
    pub(super) native_bytes: usize,
    pub(super) objects: usize,
    pub(super) file_bytes: usize,
    pub(super) decompressed_bytes: usize,
    pub(super) sections: usize,
    pub(super) vertices: usize,
    pub(super) triangles_per_render: usize,
    pub(super) texture_pixels: usize,
    pub(super) keyframes: usize,
    pub(super) graph_depth: usize,
    pub(super) fragments_per_render: usize,
}

impl M3gBudgets {
    pub(super) const fn values(self) -> [usize; 11] {
        [
            self.native_bytes,
            self.objects,
            self.file_bytes,
            self.decompressed_bytes,
            self.sections,
            self.vertices,
            self.triangles_per_render,
            self.texture_pixels,
            self.keyframes,
            self.graph_depth,
            self.fragments_per_render,
        ]
    }

    #[must_use]
    pub const fn native_bytes(self) -> usize {
        self.native_bytes
    }
    #[must_use]
    pub const fn objects(self) -> usize {
        self.objects
    }
    #[must_use]
    pub const fn file_bytes(self) -> usize {
        self.file_bytes
    }
    #[must_use]
    pub const fn decompressed_bytes(self) -> usize {
        self.decompressed_bytes
    }
    #[must_use]
    pub const fn sections(self) -> usize {
        self.sections
    }
    #[must_use]
    pub const fn vertices(self) -> usize {
        self.vertices
    }
    #[must_use]
    pub const fn triangles_per_render(self) -> usize {
        self.triangles_per_render
    }
    #[must_use]
    pub const fn texture_pixels(self) -> usize {
        self.texture_pixels
    }
    #[must_use]
    pub const fn keyframes(self) -> usize {
        self.keyframes
    }
    #[must_use]
    pub const fn graph_depth(self) -> usize {
        self.graph_depth
    }
    #[must_use]
    pub const fn fragments_per_render(self) -> usize {
        self.fragments_per_render
    }
}
