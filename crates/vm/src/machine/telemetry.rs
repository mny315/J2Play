//! VM counters, renderer batch accounting and bounded host telemetry updates.

use super::{M3gExecutionMetrics, Machine, VmTelemetry};

impl Machine<'_, '_> {
    pub(super) fn publish_vm_telemetry(&mut self) {
        const TELEMETRY_INTERVAL_INSTRUCTIONS: u64 = 65_536;
        if self
            .execution
            .instructions
            .saturating_sub(self.execution.counters.telemetry_last_instructions)
            < TELEMETRY_INTERVAL_INSTRUCTIONS
        {
            return;
        }
        self.execution.counters.telemetry_last_instructions = self.execution.instructions;
        let m3g = self.m3g_execution_metrics();
        let micro3d = self.micro3d.runtime.metrics();
        self.native_context.update_vm_telemetry(VmTelemetry {
            instructions: self.execution.instructions,
            heap_bytes: self.heap.managed.bytes(),
            runnable_threads: self.scheduler.runnable_threads.len(),
            continuations: self.scheduler.thread_continuations.len()
                + usize::from(self.scheduler.suspended_driver_call.is_some()),
            native_calls: self.execution.counters.native_calls,
            arraycopy_calls: self.execution.counters.arraycopy_calls,
            arraycopy_elements: self.execution.counters.arraycopy_elements,
            draw_region_calls: self.execution.counters.draw_region_calls,
            draw_image_calls: self.execution.counters.draw_image_calls,
            draw_rgb_calls: self.execution.counters.draw_rgb_calls,
            fill_rect_calls: self.execution.counters.fill_rect_calls,
            m3g_live_bytes: m3g.live_bytes,
            m3g_peak_bytes: m3g.peak_bytes,
            m3g_live_objects: m3g.live_objects,
            m3g_peak_objects: m3g.peak_objects,
            m3g_loaded_files: m3g.loaded_files,
            m3g_loaded_sections: m3g.loaded_sections,
            m3g_loaded_objects: m3g.loaded_objects,
            m3g_decompressed_bytes: m3g.decompressed_bytes,
            m3g_scene_nodes: m3g.scene_nodes,
            m3g_active_lights: m3g.active_lights,
            m3g_texture_binds: m3g.texture_binds,
            m3g_animation_samples: m3g.animation_samples,
            m3g_render_calls: m3g.render_calls,
            m3g_render_time_nanos: m3g.render_time_nanos,
            m3g_max_render_time_nanos: m3g.max_render_time_nanos,
            m3g_submitted_triangles: m3g.submitted_triangles,
            m3g_clipped_triangles: m3g.clipped_triangles,
            m3g_culled_triangles: m3g.culled_triangles,
            m3g_rasterized_triangles: m3g.rasterized_triangles,
            m3g_shaded_pixels: m3g.shaded_pixels,
            m3g_depth_rejected_pixels: m3g.depth_rejected_pixels,
            m3g_blended_pixels: m3g.blended_pixels,
            micro3d_live_bytes: micro3d.live_bytes,
            micro3d_peak_bytes: micro3d.peak_bytes,
            micro3d_live_objects: micro3d.live_objects,
            micro3d_peak_objects: micro3d.peak_objects,
            micro3d_loaded_figures: micro3d.loaded_figures,
            micro3d_loaded_actions: micro3d.loaded_actions,
            micro3d_loaded_textures: micro3d.loaded_textures,
            micro3d_render_calls: micro3d.render_calls,
            micro3d_rasterized_triangles: micro3d.rasterized_triangles,
            micro3d_shaded_pixels: micro3d.shaded_pixels,
        });
    }

    pub(super) fn m3g_execution_metrics(&self) -> M3gExecutionMetrics {
        let (live_objects, peak_objects, live_bytes, peak_bytes) = self.m3g.runtime.counters();
        let renderer_bytes = self.m3g.graphics.renderer.allocated_bytes();
        let current = self.m3g.graphics.renderer.stats();
        let total = self.m3g.render_totals;
        M3gExecutionMetrics {
            live_bytes: live_bytes.saturating_add(renderer_bytes),
            peak_bytes: peak_bytes.saturating_add(renderer_bytes),
            live_objects,
            peak_objects,
            active_lights: self.m3g.graphics.lights.len(),
            submitted_triangles: total
                .submitted_triangles
                .saturating_add(current.submitted_triangles),
            clipped_triangles: total
                .clipped_triangles
                .saturating_add(current.clipped_triangles),
            culled_triangles: total
                .culled_triangles
                .saturating_add(current.culled_triangles),
            rasterized_triangles: total
                .rasterized_triangles
                .saturating_add(current.rasterized_triangles),
            shaded_pixels: total
                .shaded_fragments
                .saturating_add(current.shaded_fragments),
            depth_rejected_pixels: total
                .depth_rejected_fragments
                .saturating_add(current.depth_rejected_fragments),
            blended_pixels: total
                .blended_fragments
                .saturating_add(current.blended_fragments),
            ..self.m3g.metrics
        }
    }

    pub(super) fn m3g_begin_render_batch(&mut self) {
        let current = self.m3g.graphics.renderer.stats();
        let total = &mut self.m3g.render_totals;
        total.submitted_triangles = total
            .submitted_triangles
            .saturating_add(current.submitted_triangles);
        total.clipped_triangles = total
            .clipped_triangles
            .saturating_add(current.clipped_triangles);
        total.culled_triangles = total
            .culled_triangles
            .saturating_add(current.culled_triangles);
        total.rasterized_triangles = total
            .rasterized_triangles
            .saturating_add(current.rasterized_triangles);
        total.tested_fragments = total
            .tested_fragments
            .saturating_add(current.tested_fragments);
        total.shaded_fragments = total
            .shaded_fragments
            .saturating_add(current.shaded_fragments);
        total.depth_rejected_fragments = total
            .depth_rejected_fragments
            .saturating_add(current.depth_rejected_fragments);
        total.blended_fragments = total
            .blended_fragments
            .saturating_add(current.blended_fragments);
        self.m3g.graphics.renderer.reset_stats();
    }

    pub(super) fn m3g_record_render_time(&mut self, started: std::time::Instant) {
        let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.m3g.metrics.render_calls = self.m3g.metrics.render_calls.saturating_add(1);
        self.m3g.metrics.render_time_nanos =
            self.m3g.metrics.render_time_nanos.saturating_add(elapsed);
        self.m3g.metrics.max_render_time_nanos =
            self.m3g.metrics.max_render_time_nanos.max(elapsed);
    }
}
