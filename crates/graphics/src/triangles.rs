//! Inclusive integer triangle coverage, shared by the framebuffer and VM.

use std::ops::RangeInclusive;

use crate::{Rect, clipped_line_points};

/// Clipped rows with inclusive x intervals. Each covered pixel occurs once.
/// Empty intervals retain rows with no covered integer point, so callers can
/// poll for cancellation even for very thin triangles. Degenerate triangles
/// use the same Bresenham coverage as a solid line between their extreme points.
pub fn clipped_triangle_rows(
    points: [(i32, i32); 3],
    clip: Rect,
) -> impl Iterator<Item = (i32, RangeInclusive<i32>)> {
    let area = edge(points[0], points[1], points[2]);
    let line = (area == 0).then(|| {
        clipped_line_points(
            points[0].min(points[1]).min(points[2]),
            points[0].max(points[1]).max(points[2]),
            clip,
        )
        .map(|(x, y, _)| (y, x..=x))
    });
    let min_x = points[0].0.min(points[1].0).min(points[2].0).max(clip.x);
    let min_y = points[0].1.min(points[1].1).min(points[2].1).max(clip.y);
    let max_x = i64::from(points[0].0.max(points[1].0).max(points[2].0))
        .min(i64::from(clip.x) + i64::from(clip.width) - 1);
    let max_y = i64::from(points[0].1.max(points[1].1).max(points[2].1))
        .min(i64::from(clip.y) + i64::from(clip.height) - 1);
    let visible = area != 0
        && clip.width > 0
        && clip.height > 0
        && i64::from(min_x) <= max_x
        && i64::from(min_y) <= max_y;
    let rows = visible.then_some(min_y..=max_y as i32);
    let edges = [
        (points[0], points[1]),
        (points[1], points[2]),
        (points[2], points[0]),
    ];
    let direction = area.signum();
    let mut edges = visible.then(|| {
        edges.map(|(start, end)| {
            RowEdge::new(
                edge(start, end, (min_x, min_y)) * direction,
                (i64::from(end.1) - i64::from(start.1)) * direction as i64,
                (i64::from(start.0) - i64::from(end.0)) * direction as i64,
            )
        })
    });
    let filled = rows.into_iter().flatten().map(move |y| {
        let span = edges.as_mut().map_or_else(
            || RangeInclusive::new(1, 0),
            |edges| next_row_span(edges, min_x, max_x),
        );
        (y, span)
    });
    line.into_iter().flatten().chain(filled)
}

// Keep the three-edge calculation out of the shared iterator's line path.
#[inline(never)]
fn next_row_span(edges: &mut [RowEdge; 3], min_x: i32, max_x: i64) -> RangeInclusive<i32> {
    let mut first = 0;
    let mut last = i128::from(max_x) - i128::from(min_x);
    for edge in edges {
        edge.clip_and_advance(&mut first, &mut last);
    }
    if first <= last {
        (i128::from(min_x) + first) as i32..=(i128::from(min_x) + last) as i32
    } else {
        RangeInclusive::new(1, 0)
    }
}

// Each edge constrains x by floor(value / abs(x_step)). Keep its quotient and
// remainder as y advances, so division is needed only when preparing the edge.
// Horizontal edges use divisor 1 and retain their unscaled sign test.
struct RowEdge {
    direction: i64,
    quotient: i128,
    quotient_step: i64,
    remainder: i64,
    remainder_step: i64,
    divisor: i64,
}

impl RowEdge {
    fn new(value: i128, x_step: i64, y_step: i64) -> Self {
        let divisor = x_step.abs().max(1);
        Self {
            direction: x_step.signum(),
            quotient: value.div_euclid(i128::from(divisor)),
            quotient_step: y_step.div_euclid(divisor),
            remainder: value.rem_euclid(i128::from(divisor)) as i64,
            remainder_step: y_step.rem_euclid(divisor),
            divisor,
        }
    }

    fn clip_and_advance(&mut self, first: &mut i128, last: &mut i128) {
        if self.direction > 0 {
            *first = (*first).max(-self.quotient);
        } else if self.direction < 0 {
            *last = (*last).min(self.quotient);
        } else if self.quotient < 0 {
            *last = -1;
        }
        self.quotient += i128::from(self.quotient_step);
        self.remainder += self.remainder_step;
        if self.remainder >= self.divisor {
            self.remainder -= self.divisor;
            self.quotient += 1;
        }
    }
}

fn edge(start: (i32, i32), end: (i32, i32), point: (i32, i32)) -> i128 {
    // Java int differences span 32 unsigned bits; their cross product needs
    // more than a signed 64-bit accumulator.
    (i128::from(point.0) - i128::from(start.0)) * (i128::from(end.1) - i128::from(start.1))
        - (i128::from(point.1) - i128::from(start.1)) * (i128::from(end.0) - i128::from(start.0))
}

#[cfg(test)]
#[path = "../../../tests/unit/graphics/triangles.rs"]
mod tests;
