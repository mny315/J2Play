/// Cumulative VM counters sampled by host debug overlays.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VmDebugStats {
    pub instructions: u64,
    pub heap_bytes: usize,
    pub runnable_threads: usize,
    pub continuations: usize,
    pub native_calls: u64,
    pub arraycopy_calls: u64,
    pub arraycopy_elements: u64,
    pub draw_region_calls: u64,
    pub draw_image_calls: u64,
    pub draw_rgb_calls: u64,
    pub fill_rect_calls: u64,
    pub m3g_live_bytes: usize,
    pub m3g_peak_bytes: usize,
    pub m3g_live_objects: usize,
    pub m3g_peak_objects: usize,
    pub m3g_loaded_files: u64,
    pub m3g_loaded_objects: u64,
    pub m3g_submitted_triangles: u64,
    pub m3g_rasterized_triangles: u64,
    pub m3g_shaded_pixels: u64,
}
