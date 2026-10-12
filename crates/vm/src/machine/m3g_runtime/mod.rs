use super::{
    Allocation, ArrayKind, EmuError, Handle, HashMap, HeapValue, Machine, Value, heap_error,
    m3g_bounded_image, m3g_non_negative_usize, m3g_object_class, m3g_png_format,
    m3g_scaled_sprite_half_extent, m3g_sprite_crop_coordinate, m3g_sprite_crop_sample,
    normalize_m3g_resource, optional_reference_argument, reference_argument, type_error, vm_error,
};

mod animation;
mod appearance;
mod arrays;
mod handles;
mod loading;
mod properties;
mod retained;
mod state;
mod targets;

mod work;
pub(in crate::machine) use work::GeometryWork;

pub(in crate::machine) use state::{M3gGraphicsState, M3gSpritePickContext, M3gState};
