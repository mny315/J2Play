use super::{Color32, Duration, Instant, Rect, SessionState, Vec2, egui};
use platform::VmDebugStats;

const DEBUG_HUD_FONT_SIZE: f32 = 10.0;
const DEBUG_HUD_PADDING: f32 = 5.0;
const DEBUG_VM_RATE_INTERVAL: Duration = Duration::from_millis(250);

pub(super) struct RuntimeDebugState {
    pub(super) enabled: bool,
    last_frame_at: Option<Instant>,
    frame_window_at: Instant,
    frame_window_count: u64,
    fps: f64,
    frame_ms: f64,
    frame_ema_ms: f64,
    frame_max_ms: f64,
    vm_sample_at: Option<Instant>,
    vm_sample_stats: VmDebugStats,
    vm_mips: f64,
    native_per_second: u64,
    arraycopy_per_second: u64,
    arraycopy_elements_per_second: u64,
    draw_region_per_second: u64,
    draw_image_per_second: u64,
    draw_rgb_per_second: u64,
    fill_rect_per_second: u64,
    vm: Option<VmDebugStats>,
}

impl Default for RuntimeDebugState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl RuntimeDebugState {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last_frame_at: None,
            frame_window_at: Instant::now(),
            frame_window_count: 0,
            fps: 0.0,
            frame_ms: 0.0,
            frame_ema_ms: 0.0,
            frame_max_ms: 0.0,
            vm_sample_at: None,
            vm_sample_stats: VmDebugStats::default(),
            vm_mips: 0.0,
            native_per_second: 0,
            arraycopy_per_second: 0,
            arraycopy_elements_per_second: 0,
            draw_region_per_second: 0,
            draw_image_per_second: 0,
            draw_rgb_per_second: 0,
            fill_rect_per_second: 0,
            vm: None,
        }
    }

    pub(super) fn set_enabled(&mut self, enabled: bool) {
        *self = Self::new(enabled);
    }

    pub(super) fn reset_session(&mut self) {
        *self = Self::new(self.enabled);
    }

    #[allow(clippy::cast_precision_loss)]
    pub(super) fn note_frame(&mut self, now: Instant) {
        if !self.enabled {
            return;
        }
        if let Some(previous) = self.last_frame_at.replace(now) {
            self.frame_ms = now.saturating_duration_since(previous).as_secs_f64() * 1_000.0;
            self.frame_ema_ms = if self.frame_ema_ms == 0.0 {
                self.frame_ms
            } else {
                self.frame_ema_ms.mul_add(0.85, self.frame_ms * 0.15)
            };
            self.frame_max_ms = self.frame_max_ms.max(self.frame_ms);
        }
        self.frame_window_count = self.frame_window_count.saturating_add(1);
        let elapsed = now.saturating_duration_since(self.frame_window_at);
        if elapsed >= Duration::from_secs(1) {
            self.fps = self.frame_window_count as f64 / elapsed.as_secs_f64();
            self.frame_window_count = 0;
            self.frame_window_at = now;
            self.frame_max_ms = self.frame_ms;
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub(super) fn note_vm(&mut self, now: Instant, stats: VmDebugStats) {
        if !self.enabled {
            return;
        }
        if let Some(previous_at) = self.vm_sample_at {
            let elapsed = now.saturating_duration_since(previous_at);
            if elapsed >= DEBUG_VM_RATE_INTERVAL {
                let seconds = elapsed.as_secs_f64();
                self.vm_mips = stats
                    .instructions
                    .saturating_sub(self.vm_sample_stats.instructions)
                    as f64
                    / seconds
                    / 1_000_000.0;
                let rate = |current: u64, previous: u64| {
                    rate_per_second(current.saturating_sub(previous), elapsed)
                };
                self.native_per_second =
                    rate(stats.native_calls, self.vm_sample_stats.native_calls);
                self.arraycopy_per_second =
                    rate(stats.arraycopy_calls, self.vm_sample_stats.arraycopy_calls);
                self.arraycopy_elements_per_second = rate(
                    stats.arraycopy_elements,
                    self.vm_sample_stats.arraycopy_elements,
                );
                self.draw_region_per_second = rate(
                    stats.draw_region_calls,
                    self.vm_sample_stats.draw_region_calls,
                );
                self.draw_image_per_second = rate(
                    stats.draw_image_calls,
                    self.vm_sample_stats.draw_image_calls,
                );
                self.draw_rgb_per_second =
                    rate(stats.draw_rgb_calls, self.vm_sample_stats.draw_rgb_calls);
                self.fill_rect_per_second =
                    rate(stats.fill_rect_calls, self.vm_sample_stats.fill_rect_calls);
                self.vm_sample_at = Some(now);
                self.vm_sample_stats = stats;
            }
        } else {
            self.vm_sample_at = Some(now);
            self.vm_sample_stats = stats;
        }
        self.vm = Some(stats);
    }
}

fn rate_per_second(count: u64, elapsed: Duration) -> u64 {
    let nanos = elapsed.as_nanos();
    if nanos == 0 {
        return 0;
    }
    u64::try_from(u128::from(count) * 1_000_000_000 / nanos).unwrap_or(u64::MAX)
}

const fn runtime_state_label(state: SessionState) -> &'static str {
    match state {
        SessionState::Idle => "IDLE",
        SessionState::Starting => "STARTING",
        SessionState::Running => "RUNNING",
        SessionState::Paused { user: true, .. } => "USER PAUSE",
        SessionState::Paused {
            user: false,
            lifecycle: true,
        } => "BACKGROUND",
        SessionState::Paused {
            user: false,
            lifecycle: false,
        } => "PAUSED",
        SessionState::Stopping => "STOPPING",
        SessionState::Failed => "FAILED",
    }
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn runtime_debug_lines(
    debug: &RuntimeDebugState,
    state: SessionState,
    canvas_dimensions: (u32, u32),
    fast_forward: bool,
    diagnostic_count: usize,
) -> Vec<String> {
    // Technical telemetry uses stable English labels in every UI language.
    let mut lines = Vec::with_capacity(8);
    lines.push(format!(
        "RUNTIME {} {}x{} FF {}",
        runtime_state_label(state),
        canvas_dimensions.0,
        canvas_dimensions.1,
        if fast_forward { "On" } else { "Off" }
    ));
    lines.push(format!(
        "FPS {:.1} FRAME {:.1} AVG {:.1} MAX {:.1}MS",
        debug.fps, debug.frame_ms, debug.frame_ema_ms, debug.frame_max_ms
    ));
    let diagnostics = format!("DIAGNOSTICS {diagnostic_count}");
    let Some(vm) = debug.vm else {
        lines.push("VM WAITING FOR TELEMETRY".to_owned());
        lines.push(diagnostics);
        return lines;
    };
    lines.push(format!(
        "VM {:.2}MIPS INS {}",
        debug.vm_mips, vm.instructions
    ));
    lines.push(format!(
        "HEAP {:.1}MB THR {} CONT {}",
        vm.heap_bytes as f64 / (1024.0 * 1024.0),
        vm.runnable_threads,
        vm.continuations
    ));
    lines.push(format!(
        "DRAW R{} I{} RGB{} F{}/S",
        debug.draw_region_per_second,
        debug.draw_image_per_second,
        debug.draw_rgb_per_second,
        debug.fill_rect_per_second
    ));
    lines.push(format!(
        "COPY {}/S {}KEL/S NATIVE {}/S",
        debug.arraycopy_per_second,
        debug.arraycopy_elements_per_second / 1_000,
        debug.native_per_second
    ));
    if vm.m3g_live_objects != 0
        || vm.m3g_loaded_files != 0
        || vm.m3g_submitted_triangles != 0
        || vm.m3g_shaded_pixels != 0
    {
        lines.push(format!(
            "M3G OBJ {} MEM {:.1}MB TRI {} PIX {}",
            vm.m3g_live_objects,
            vm.m3g_live_bytes as f64 / (1024.0 * 1024.0),
            vm.m3g_rasterized_triangles,
            vm.m3g_shaded_pixels
        ));
    }
    lines.push(diagnostics);
    lines
}

pub(super) fn paint_runtime_debug_hud(ui: &egui::Ui, available: Rect, lines: &[String]) {
    if available.width() <= 0.0 || available.height() <= 0.0 || lines.is_empty() {
        return;
    }
    let color = Color32::from_rgb(235, 235, 235);
    let galley = ui.painter().layout(
        lines.join("\n"),
        egui::FontId::monospace(DEBUG_HUD_FONT_SIZE),
        color,
        (available.width() - 8.0 - DEBUG_HUD_PADDING * 2.0).max(1.0),
    );
    let desired = galley.size() + Vec2::splat(DEBUG_HUD_PADDING * 2.0);
    let panel = Rect::from_min_size(
        available.min + Vec2::splat(4.0),
        Vec2::new(
            desired.x.min((available.width() - 8.0).max(0.0)),
            desired.y.min((available.height() - 8.0).max(0.0)),
        ),
    );
    let painter = ui.painter().with_clip_rect(panel.intersect(available));
    painter.rect_filled(panel, 3.0, Color32::from_black_alpha(220));
    painter.galley(panel.min + Vec2::splat(DEBUG_HUD_PADDING), galley, color);
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/runtime_debug.rs"]
mod tests;
