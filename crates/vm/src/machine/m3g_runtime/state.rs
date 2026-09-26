//! VM ownership, heap roots and target bindings for the shared M3G runtime.

use crate::machine::{EmuError, Handle, Heap, M3gExecutionMetrics};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(in crate::machine) struct M3gGraphicsState {
    pub(in crate::machine) target: Option<Handle>,
    pub(in crate::machine) target_origin: [i32; 2],
    pub(in crate::machine) depth_target: Option<Handle>,
    pub(in crate::machine) binding_thread: Option<u64>,
    pub(in crate::machine) hints: i32,
    pub(in crate::machine) depth_enabled: bool,
    pub(in crate::machine) viewport: [i32; 4],
    pub(in crate::machine) depth_range: [f32; 2],
    pub(in crate::machine) camera: Option<(Handle, m3g::Mat4)>,
    pub(in crate::machine) lights: Vec<(Handle, m3g::Mat4)>,
    pub(in crate::machine) renderer: m3g::SoftwareRenderer,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(in crate::machine) struct M3gState {
    pub(in crate::machine) runtime: m3g::Runtime,
    pub(in crate::machine) graphics3d: Option<Handle>,
    pub(in crate::machine) graphics: M3gGraphicsState,
    pub(in crate::machine) metrics: M3gExecutionMetrics,
    pub(in crate::machine) render_totals: m3g::RenderStats,
}

impl M3gState {
    pub(in crate::machine) fn append_roots(&self, roots: &mut Vec<Handle>) {
        roots.extend(self.graphics3d);
        roots.extend(self.graphics.target);
        roots.extend(self.graphics.camera.map(|(camera, _)| camera));
        roots.extend(self.graphics.lights.iter().map(|(light, _)| *light));
    }

    pub(in crate::machine) fn sweep(&mut self, heap: &Heap) {
        self.runtime
            .sweep_guest_objects(|reference| heap.get(Handle::from_raw(reference)).is_ok());
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::machine) struct M3gSpritePickContext {
    pub(in crate::machine) viewport_point: [f32; 2],
    pub(in crate::machine) projection: m3g::Mat4,
    pub(in crate::machine) group_to_camera: m3g::Mat4,
}

impl M3gGraphicsState {
    pub(in crate::machine) fn new(
        width: u32,
        height: u32,
        render_limits: m3g::RenderLimits,
    ) -> Self {
        Self {
            target: None,
            target_origin: [0, 0],
            depth_target: None,
            binding_thread: None,
            hints: 0,
            depth_enabled: true,
            viewport: [
                0,
                0,
                i32::try_from(width).expect("validated LCD width fits Java int"),
                i32::try_from(height).expect("validated LCD height fits Java int"),
            ],
            depth_range: [0.0, 1.0],
            camera: None,
            lights: Vec::new(),
            renderer: m3g::SoftwareRenderer::new(width, height, render_limits)
                .expect("initial M3G target dimensions are valid"),
        }
    }

    pub(in crate::machine) fn bind_target_surface(
        &mut self,
        target: Handle,
        width: u32,
        height: u32,
        pixels: Vec<u32>,
        scissor: [u32; 4],
        depth_enabled: bool,
        render_limits: m3g::RenderLimits,
    ) -> Result<(), EmuError> {
        let preserves_depth = depth_enabled
            && self.depth_enabled
            && self.depth_target == Some(target)
            && self.renderer.width() == width
            && self.renderer.height() == height;
        if preserves_depth {
            self.renderer.replace_pixels(pixels)?;
        } else {
            self.renderer =
                m3g::SoftwareRenderer::from_pixels(width, height, pixels, render_limits)?;
        }
        self.renderer
            .set_scissor(scissor[0], scissor[1], scissor[2], scissor[3])?;
        // An empty MIDP clip still binds successfully. Keep the projection
        // dimensions nonzero; the empty scissor prevents all pixel writes.
        self.renderer.set_viewport(
            i32::try_from(scissor[0]).unwrap_or(i32::MAX),
            i32::try_from(scissor[1]).unwrap_or(i32::MAX),
            scissor[2].max(1),
            scissor[3].max(1),
        )?;
        self.renderer
            .set_depth_range(self.depth_range[0], self.depth_range[1])?;
        // JSR-184 fixes the depth comparison function to LEQUAL.  The generic
        // software rasterizer defaults to a strict comparison, so select the
        // M3G contract explicitly whenever a target surface is (re)created.
        self.renderer.set_depth_test_allows_equal(true);
        self.depth_enabled = depth_enabled;
        self.renderer
            .set_compositing(depth_enabled, depth_enabled, true, false);
        self.depth_target = depth_enabled.then_some(target);
        Ok(())
    }
}
