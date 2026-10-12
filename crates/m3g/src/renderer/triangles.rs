//! Indexed triangles, fixed-point coverage and perspective-correct attributes.

use super::clipping::clip_triangle;
use super::{
    ClipVertex, CullMode, DEPTH_MAX, SUBPIXEL_SCALE, ScreenVertex, SoftwareRenderer, Vertex,
    pack_color, render_error,
};
use crate::math::rounded_u32;
use crate::{Mat4, Texture2DState, TriangleStripArrayState, VertexBufferState};
use diagnostics::EmuError;

impl SoftwareRenderer {
    /// Clips and rasterizes one triangle with top-left coverage and fixed depth.
    pub fn draw_triangle(&mut self, vertices: [Vertex; 3]) -> Result<(), EmuError> {
        if self.stats.submitted_triangles >= self.limits.triangles {
            return Err(render_error(
                "render-budget",
                "triangle work budget exhausted",
            ));
        }
        self.stats.submitted_triangles += 1;
        for (destination, source) in self.clip_a.iter_mut().zip(vertices) {
            *destination = ClipVertex::from_vertex(source)?;
        }
        if !self.smooth_shading {
            // JSR-184 PolygonMode uses the original third vertex, even if
            // clipping removes it or introduces new vertices.
            self.clip_a[0].color = self.clip_a[2].color;
            self.clip_a[1].color = self.clip_a[2].color;
        }
        let (count, clipped) = clip_triangle(&mut self.clip_a, &mut self.clip_b);
        if clipped {
            self.stats.clipped_triangles += 1;
        }
        if count < 3 {
            self.stats.culled_triangles += 1;
            return Ok(());
        }
        for index in 1..count - 1 {
            self.rasterize(&[self.clip_a[0], self.clip_a[index], self.clip_a[index + 1]])?;
        }
        Ok(())
    }

    /// Transforms and draws every non-degenerate triangle in an indexed strip array.
    /// Index validation completes before the renderer or its counters are mutated.
    pub fn draw_indexed(
        &mut self,
        vertices: &VertexBufferState,
        indices: &TriangleStripArrayState,
        transform: Mat4,
    ) -> Result<(), EmuError> {
        let transformed = vertices.transformed_vertices(transform)?;
        self.draw_transformed_indexed(&transformed, indices)
    }

    /// Draws already transformed vertices with the same all-indices-first validation.
    pub fn draw_transformed_indexed(
        &mut self,
        transformed: &[Vertex],
        indices: &TriangleStripArrayState,
    ) -> Result<(), EmuError> {
        let triangles = indices.triangles(transformed.len())?;
        let submitted = u64::try_from(triangles.len()).unwrap_or(u64::MAX);
        if self.stats.submitted_triangles.saturating_add(submitted) > self.limits.triangles {
            return Err(render_error(
                "render-budget",
                "indexed geometry exceeds the remaining triangle budget",
            ));
        }
        for triangle in triangles {
            self.draw_triangle([
                transformed[triangle[0]],
                transformed[triangle[1]],
                transformed[triangle[2]],
            ])?;
        }
        Ok(())
    }

    // Keep the rasterizer's large stack frame out of fully clipped draws.
    #[inline(never)]
    fn rasterize(&mut self, triangle: &[ClipVertex; 3]) -> Result<(), EmuError> {
        let mut screen = [ScreenVertex::default(); 3];
        for (destination, source) in screen.iter_mut().zip(triangle) {
            let Some(projected) =
                ScreenVertex::from_clip(*source, self.viewport, self.depth_range)?
            else {
                self.stats.culled_triangles += 1;
                return Ok(());
            };
            *destination = projected;
        }

        let signed_area = edge(screen[0], screen[1], screen[2].x, screen[2].y);
        if signed_area == 0
            || matches!(self.cull_mode, CullMode::Clockwise) && signed_area > 0
            || matches!(self.cull_mode, CullMode::CounterClockwise) && signed_area < 0
        {
            self.stats.culled_triangles += 1;
            return Ok(());
        }
        if signed_area < 0 {
            screen.swap(1, 2);
        }
        let area = signed_area.unsigned_abs() as f64;
        // `edge` uses the opposite sign; winding was normalized above.
        let determinant = -area;
        let depth_dx = ((screen[1].depth - screen[0].depth) * (screen[2].y - screen[0].y) as f64
            - (screen[2].depth - screen[0].depth) * (screen[1].y - screen[0].y) as f64)
            / determinant
            * SUBPIXEL_SCALE as f64;
        let depth_dy = ((screen[2].depth - screen[0].depth) * (screen[1].x - screen[0].x) as f64
            - (screen[1].depth - screen[0].depth) * (screen[2].x - screen[0].x) as f64)
            / determinant
            * SUBPIXEL_SCALE as f64;
        let depth_offset = self.depth_offset[0] * depth_dx.abs().max(depth_dy.abs())
            + self.depth_offset[1] / DEPTH_MAX;
        let [clip_x, clip_y, clip_right, clip_bottom] = self.viewport_bounds();
        let [first, second, third] = &screen;
        let min_x = floor_pixel(first.x)
            .min(floor_pixel(second.x))
            .min(floor_pixel(third.x))
            .max(clip_x as i32);
        let max_x = ceil_pixel(first.x)
            .max(ceil_pixel(second.x))
            .max(ceil_pixel(third.x))
            .min(clip_right as i32 - 1);
        let min_y = floor_pixel(first.y)
            .min(floor_pixel(second.y))
            .min(floor_pixel(third.y))
            .max(clip_y as i32);
        let max_y = ceil_pixel(first.y)
            .max(ceil_pixel(second.y))
            .max(ceil_pixel(third.y))
            .min(clip_bottom as i32 - 1);
        if min_x > max_x || min_y > max_y {
            self.stats.culled_triangles += 1;
            return Ok(());
        }
        let candidate_width = u64::try_from(max_x - min_x + 1).unwrap_or(u64::MAX);
        let candidate_height = u64::try_from(max_y - min_y + 1).unwrap_or(u64::MAX);
        let candidates = candidate_width.saturating_mul(candidate_height);
        if self.stats.tested_fragments.saturating_add(candidates) > self.limits.fragments {
            return Err(render_error(
                "render-budget",
                "fragment work budget exhausted",
            ));
        }
        self.stats.tested_fragments += candidates;

        let top_left = [
            is_top_left(screen[1], screen[2]),
            is_top_left(screen[2], screen[0]),
            is_top_left(screen[0], screen[1]),
        ];
        let texture_lod = [0, 1].map(|unit| {
            self.textures[unit]
                .as_ref()
                .map_or(0.0, |texture| triangle_texture_lod(&screen, unit, texture))
        });
        // Only exactly constant attributes may bypass interpolation.
        #[allow(clippy::float_cmp)]
        let constant_color = (triangle[0].color == triangle[1].color
            && triangle[0].color == triangle[2].color)
            .then(|| pack_color(triangle[0].color));
        // Positive reciprocal W and covered barycentric weights cannot produce
        // a zero denominator. A flat, untextured fragment without fog therefore
        // needs only its color and depth, with no perspective interpolation.
        let constant_fragment = constant_color.filter(|_| {
            self.textures.iter().all(Option::is_none)
                && self.fog.is_none()
                && screen.iter().all(|vertex| vertex.inverse_w > 0.0)
        });
        #[allow(clippy::float_cmp)]
        let constant_texture_z = [0, 1].map(|unit| {
            let value = triangle[0].texture[unit][2];
            (value.is_finite()
                && value == triangle[1].texture[unit][2]
                && value == triangle[2].texture[unit][2])
                .then_some(value)
        });
        let edge_step_x = [
            (screen[2].y - screen[1].y) * SUBPIXEL_SCALE,
            (screen[0].y - screen[2].y) * SUBPIXEL_SCALE,
            (screen[1].y - screen[0].y) * SUBPIXEL_SCALE,
        ];
        let first_sample_x = i64::from(min_x) * SUBPIXEL_SCALE + SUBPIXEL_SCALE / 2;
        let needs_depth = self.depth_test || self.depth_write;
        for y in min_y..=max_y {
            let sample_y = i64::from(y) * SUBPIXEL_SCALE + SUBPIXEL_SCALE / 2;
            let mut next_edges = [
                edge(screen[1], screen[2], first_sample_x, sample_y),
                edge(screen[2], screen[0], first_sample_x, sample_y),
                edge(screen[0], screen[1], first_sample_x, sample_y),
            ];
            let span = covered_span(next_edges, edge_step_x, top_left, max_x - min_x + 1);
            for index in 0..3 {
                next_edges[index] += i64::from(span.start) * edge_step_x[index];
            }
            for x in min_x + span.start..min_x + span.end {
                let edges = next_edges;
                for index in 0..3 {
                    next_edges[index] += edge_step_x[index];
                }
                self.stats.shaded_fragments += 1;
                let weights = edges.map(|value| value as f64 / area);
                let pixel_index = y as usize * self.width as usize + x as usize;
                let depth = if needs_depth {
                    let depth = weights[0].mul_add(
                        screen[0].depth,
                        weights[1].mul_add(screen[1].depth, weights[2] * screen[2].depth),
                    );
                    let depth = rounded_u32(
                        (depth + depth_offset).clamp(0.0, 1.0) * DEPTH_MAX,
                        DEPTH_MAX as u32,
                    );
                    if self.depth_rejects_fragment(pixel_index, depth) {
                        continue;
                    }
                    depth
                } else {
                    0
                };
                if let Some(source) = constant_fragment {
                    self.write_fragment(pixel_index, depth, source, 0.0);
                    continue;
                }
                let denominator = weights[0].mul_add(
                    screen[0].inverse_w,
                    weights[1].mul_add(screen[1].inverse_w, weights[2] * screen[2].inverse_w),
                );
                if denominator == 0.0 {
                    continue;
                }
                let mut source = constant_color.unwrap_or_else(|| {
                    let mut components = [0.0; 4];
                    for (component, value) in components.iter_mut().enumerate() {
                        *value = weights[0].mul_add(
                            screen[0].color_over_w[component],
                            weights[1].mul_add(
                                screen[1].color_over_w[component],
                                weights[2] * screen[2].color_over_w[component],
                            ),
                        ) / denominator;
                    }
                    pack_color(components)
                });
                for (unit, texture) in self.textures.iter().enumerate() {
                    if let Some(texture) = texture {
                        let coordinates = [0, 1, 2].map(|component| {
                            if component == 2
                                && let Some(value) = constant_texture_z[unit]
                            {
                                return value;
                            }
                            weights[0].mul_add(
                                screen[0].texture_over_w[unit][component],
                                weights[1].mul_add(
                                    screen[1].texture_over_w[unit][component],
                                    weights[2] * screen[2].texture_over_w[unit][component],
                                ),
                            ) / denominator
                        });
                        source =
                            texture.shade_coordinates_lod(coordinates, source, texture_lod[unit]);
                    }
                }
                let eye_z = if self.fog.is_some() {
                    weights[0].mul_add(
                        screen[0].eye_z_over_w,
                        weights[1]
                            .mul_add(screen[1].eye_z_over_w, weights[2] * screen[2].eye_z_over_w),
                    ) / denominator
                } else {
                    0.0
                };
                self.write_fragment(pixel_index, depth, source, eye_z);
            }
        }
        self.stats.rasterized_triangles += 1;
        Ok(())
    }
}

/// Intersects the three edge half-planes with this row. Integer division keeps
/// exactly the same top-left coverage as testing each pixel in the bounding box.
/// The full box still counts toward the work budget, independently of coverage.
fn covered_span(
    edges: [i64; 3],
    steps: [i64; 3],
    top_left: [bool; 3],
    width: i32,
) -> std::ops::Range<i32> {
    let mut first = 0_i64;
    let mut last = i64::from(width) - 1;
    for index in 0..3 {
        let edge = edges[index] - i64::from(!top_left[index]);
        let step = steps[index];
        if step > 0 {
            first = first.max(-edge.div_euclid(step));
        } else if step < 0 {
            last = last.min(edge.div_euclid(-step));
        } else if edge < 0 {
            return 0..0;
        }
        if first > last {
            return 0..0;
        }
    }
    first as i32..last as i32 + 1
}

fn triangle_texture_lod(screen: &[ScreenVertex; 3], unit: usize, texture: &Texture2DState) -> f64 {
    if !texture.uses_level_of_detail() {
        return 0.0;
    }
    let coordinates: [[f64; 2]; 3] = std::array::from_fn(|index| {
        let vertex = &screen[index];
        let denominator = vertex.inverse_w;
        [
            vertex.texture_over_w[unit][0] / denominator,
            vertex.texture_over_w[unit][1] / denominator,
        ]
    });
    let positions: [[f64; 2]; 3] = std::array::from_fn(|index| {
        let vertex = &screen[index];
        [
            vertex.x as f64 / SUBPIXEL_SCALE as f64,
            vertex.y as f64 / SUBPIXEL_SCALE as f64,
        ]
    });
    let dx1 = positions[1][0] - positions[0][0];
    let dy1 = positions[1][1] - positions[0][1];
    let dx2 = positions[2][0] - positions[0][0];
    let dy2 = positions[2][1] - positions[0][1];
    let determinant = dx1.mul_add(dy2, -(dx2 * dy1));
    if determinant.abs() <= f64::EPSILON {
        return 0.0;
    }
    let inverse = determinant.recip();
    let (width, height) = texture.dimensions();
    let derivative = |component: usize| {
        let first = coordinates[1][component] - coordinates[0][component];
        let second = coordinates[2][component] - coordinates[0][component];
        (
            first.mul_add(dy2, -(second * dy1)) * inverse,
            dx1.mul_add(second, -(dx2 * first)) * inverse,
        )
    };
    let (ds_dx, ds_dy) = derivative(0);
    let (dt_dx, dt_dy) = derivative(1);
    let rho_x = (ds_dx * f64::from(width)).hypot(dt_dx * f64::from(height));
    let rho_y = (ds_dy * f64::from(width)).hypot(dt_dy * f64::from(height));
    rho_x.max(rho_y).max(1.0).log2()
}

fn edge(first: ScreenVertex, second: ScreenVertex, x: i64, y: i64) -> i64 {
    (x - first.x) * (second.y - first.y) - (y - first.y) * (second.x - first.x)
}

fn is_top_left(first: ScreenVertex, second: ScreenVertex) -> bool {
    let dx = second.x - first.x;
    let dy = second.y - first.y;
    dy > 0 || dy == 0 && dx < 0
}

fn floor_pixel(value: i64) -> i32 {
    value.div_euclid(SUBPIXEL_SCALE) as i32
}

fn ceil_pixel(value: i64) -> i32 {
    value
        .saturating_add(SUBPIXEL_SCALE - 1)
        .div_euclid(SUBPIXEL_SCALE) as i32
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/renderer/triangles.rs"]
mod tests;
