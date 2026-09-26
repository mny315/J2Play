//! Values exchanged with host services.

/// One requested RMS value. Only available capacity requires a suite storage scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RmsMetadataField {
    Version,
    RecordCount,
    Size,
    SizeAvailable,
    NextRecordId,
    LastModified,
}

/// HTTP request crossing the VM/host boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GcfHttpRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// HTTP response returned to the Java connection object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GcfHttpResponse {
    pub status: i32,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Filesystem metadata snapshot for one suite-sandbox path.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct GcfFileMetadata {
    pub exists: bool,
    pub directory: bool,
    pub size: u64,
    pub modified_millis: i64,
    pub readable: bool,
    pub writable: bool,
}

/// VM counters for the host's diagnostic overlay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VmTelemetry {
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
    pub m3g_loaded_sections: u64,
    pub m3g_loaded_objects: u64,
    pub m3g_decompressed_bytes: u64,
    pub m3g_scene_nodes: u64,
    pub m3g_active_lights: usize,
    pub m3g_texture_binds: u64,
    pub m3g_animation_samples: u64,
    pub m3g_render_calls: u64,
    pub m3g_render_time_nanos: u64,
    pub m3g_max_render_time_nanos: u64,
    pub m3g_submitted_triangles: u64,
    pub m3g_clipped_triangles: u64,
    pub m3g_culled_triangles: u64,
    pub m3g_rasterized_triangles: u64,
    pub m3g_shaded_pixels: u64,
    pub m3g_depth_rejected_pixels: u64,
    pub m3g_blended_pixels: u64,
    pub micro3d_live_bytes: usize,
    pub micro3d_peak_bytes: usize,
    pub micro3d_live_objects: usize,
    pub micro3d_peak_objects: usize,
    pub micro3d_loaded_figures: u64,
    pub micro3d_loaded_actions: u64,
    pub micro3d_loaded_textures: u64,
    pub micro3d_render_calls: u64,
    pub micro3d_rasterized_triangles: u64,
    pub micro3d_shaded_pixels: u64,
}

/// A burst of managed-heap failures caught at the same guest exception handler.
/// A single caught OOM may be a deliberate fallback and does not trigger this notice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagedHeapLimitNotice {
    pub caught_count: u64,
    pub heap_bytes: usize,
    pub heap_limit_bytes: usize,
}

/// Frontend response to a repeated managed-heap-limit notice.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ManagedHeapLimitDecision {
    /// Let the guest's catch handler continue normally.
    #[default]
    Continue,
    /// Stop this attempt so the frontend can ask for another device profile.
    RequestProfileChange,
}

/// Replaces the current vibration effect without blocking.
/// `level` is a logical intensity in `1..=100`; `None` uses the host default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VibrationRequest {
    #[default]
    Stop,
    Continuous {
        level: Option<u8>,
    },
    Timed {
        duration_millis: u64,
        level: Option<u8>,
    },
}
