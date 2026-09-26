//! Point and line coverage with shared depth and fragment composition.

use super::clipping::clip_line;
use super::{
    ClipVertex, DEPTH_MAX, SUBPIXEL_SCALE, ScreenVertex, SoftwareRenderer, Vertex, pack_color,
    render_error,
};
use crate::math::rounded_u32;
use diagnostics::EmuError;

impl SoftwareRenderer {
    /// Rasterizes one untextured point as one target pixel with the active
    /// clip, depth, fog, alpha and framebuffer compositing state.
    pub fn draw_point(&mut self, vertex: Vertex) -> Result<(), EmuError> {
        if self.stats.submitted_triangles >= self.limits.triangles {
            return Err(render_error(
                "render-budget",
                "primitive work budget exhausted",
            ));
        }
        self.stats.submitted_triangles += 1;
        vertex.validate_position()?;
        let position = vertex.position;
        if [
            position.x + position.w,
            position.w - position.x,
            position.y + position.w,
            position.w - position.y,
            position.z + position.w,
            position.w - position.z,
        ]
        .into_iter()
        .any(|distance| distance < 0.0)
        {
            self.stats.culled_triangles += 1;
            return Ok(());
        }
        if position.w == 0.0 {
            self.stats.culled_triangles += 1;
            return Ok(());
        }

        let inverse_w = f64::from(position.w).recip();
        let ndc_x = f64::from(position.x) * inverse_w;
        let ndc_y = f64::from(position.y) * inverse_w;
        let ndc_z = f64::from(position.z) * inverse_w;
        let screen_x =
            f64::from(self.viewport[0]) + (ndc_x + 1.0) * 0.5 * f64::from(self.viewport[2]);
        let screen_y =
            f64::from(self.viewport[1]) + (1.0 - ndc_y) * 0.5 * f64::from(self.viewport[3]);
        let [left, top, right, bottom] = self.viewport_bounds();
        if screen_x < f64::from(left)
            || screen_x >= f64::from(right)
            || screen_y < f64::from(top)
            || screen_y >= f64::from(bottom)
        {
            self.stats.culled_triangles += 1;
            return Ok(());
        }
        if self.stats.tested_fragments >= self.limits.fragments {
            return Err(render_error(
                "render-budget",
                "fragment work budget exhausted",
            ));
        }
        self.stats.tested_fragments += 1;
        self.stats.shaded_fragments += 1;

        let x = screen_x.floor() as usize;
        let y = screen_y.floor() as usize;
        let pixel_index = y * self.width as usize + x;
        let normalized = ((ndc_z + 1.0) * 0.5).clamp(0.0, 1.0);
        let depth = normalized.mul_add(
            self.depth_range[1] - self.depth_range[0],
            self.depth_range[0],
        ) + self.depth_offset[1] / DEPTH_MAX;
        let depth = rounded_u32(depth.clamp(0.0, 1.0) * DEPTH_MAX, DEPTH_MAX as u32);
        if self.depth_rejects_fragment(pixel_index, depth) {
            return Ok(());
        }

        if self.write_fragment(pixel_index, depth, vertex.color, f64::from(vertex.eye_z)) {
            self.stats.rasterized_triangles += 1;
        }
        Ok(())
    }

    /// Clips and rasterizes one untextured, one-pixel-wide line segment with
    /// the active depth, fog, alpha and framebuffer compositing state.
    pub fn draw_line(&mut self, vertices: [Vertex; 2]) -> Result<(), EmuError> {
        if self.stats.submitted_triangles >= self.limits.triangles {
            return Err(render_error(
                "render-budget",
                "primitive work budget exhausted",
            ));
        }
        self.stats.submitted_triangles += 1;
        let mut line = [
            ClipVertex::from_vertex(vertices[0])?,
            ClipVertex::from_vertex(vertices[1])?,
        ];
        if !self.smooth_shading {
            line[0].color = line[1].color;
        }
        let (visible, clipped) = clip_line(&mut line);
        if clipped {
            self.stats.clipped_triangles += 1;
        }
        if !visible {
            self.stats.culled_triangles += 1;
            return Ok(());
        }
        let mut screen = [ScreenVertex::default(); 2];
        for (destination, source) in screen.iter_mut().zip(line) {
            let Some(projected) = ScreenVertex::from_clip(source, self.viewport, self.depth_range)?
            else {
                self.stats.culled_triangles += 1;
                return Ok(());
            };
            *destination = projected;
        }

        let delta_x = screen[1].x - screen[0].x;
        let delta_y = screen[1].y - screen[0].y;
        let steps = delta_x
            .unsigned_abs()
            .max(delta_y.unsigned_abs())
            .div_ceil(SUBPIXEL_SCALE as u64)
            .max(1);
        let candidates = steps.saturating_add(1);
        if self.stats.tested_fragments.saturating_add(candidates) > self.limits.fragments {
            return Err(render_error(
                "render-budget",
                "fragment work budget exhausted",
            ));
        }
        self.stats.tested_fragments += candidates;

        let [left, top, right, bottom] = self.viewport_bounds();
        let needs_depth = self.depth_test || self.depth_write;
        // Equal projected endpoints give exactly the same quotient at every
        // sample. Keep the normal path when perspective changes reciprocal W.
        #[allow(clippy::float_cmp)]
        let constant_color = (screen[0].inverse_w == screen[1].inverse_w
            && screen[0].color_over_w == screen[1].color_over_w)
            .then(|| {
                pack_color(
                    screen[0]
                        .color_over_w
                        .map(|value| value / screen[0].inverse_w),
                )
            });
        let mut rasterized = false;
        let mut previous_pixel = None;
        for step in 0..=steps {
            let amount = step as f64 / steps as f64;
            let interpolate_screen = |first: i64, second: i64| {
                (amount.mul_add((second - first) as f64, first as f64) / SUBPIXEL_SCALE as f64)
                    .floor() as i64
            };
            let x = interpolate_screen(screen[0].x, screen[1].x);
            let y = interpolate_screen(screen[0].y, screen[1].y);
            // Subpixel steps can land in the same pixel, especially for short
            // lines. A primitive must not blend that pixel with itself.
            if previous_pixel.replace((x, y)) == Some((x, y)) {
                continue;
            }
            if x < i64::from(left)
                || x >= i64::from(right)
                || y < i64::from(top)
                || y >= i64::from(bottom)
            {
                continue;
            }
            let inverse_w = amount.mul_add(
                screen[1].inverse_w - screen[0].inverse_w,
                screen[0].inverse_w,
            );
            if inverse_w == 0.0 {
                continue;
            }
            let pixel_index = y as usize * self.width as usize + x as usize;
            let depth = if needs_depth {
                let depth = amount.mul_add(screen[1].depth - screen[0].depth, screen[0].depth);
                let depth = rounded_u32(
                    (depth + self.depth_offset[1] / DEPTH_MAX).clamp(0.0, 1.0) * DEPTH_MAX,
                    DEPTH_MAX as u32,
                );
                if self.depth_rejects_fragment(pixel_index, depth) {
                    continue;
                }
                depth
            } else {
                0
            };
            self.stats.shaded_fragments += 1;
            let source = constant_color.unwrap_or_else(|| {
                let mut components = [0.0; 4];
                for (component, value) in components.iter_mut().enumerate() {
                    *value = amount.mul_add(
                        screen[1].color_over_w[component] - screen[0].color_over_w[component],
                        screen[0].color_over_w[component],
                    ) / inverse_w;
                }
                pack_color(components)
            });
            let eye_z = if self.fog.is_some() {
                amount.mul_add(
                    screen[1].eye_z_over_w - screen[0].eye_z_over_w,
                    screen[0].eye_z_over_w,
                ) / inverse_w
            } else {
                0.0
            };
            if self.write_fragment(pixel_index, depth, source, eye_z) {
                rasterized = true;
            }
        }
        if rasterized {
            self.stats.rasterized_triangles += 1;
        } else {
            self.stats.culled_triangles += 1;
        }
        Ok(())
    }
}
