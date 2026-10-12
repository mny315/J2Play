use super::{Graphics, Rect, StrokeStyle, clamp_i64};

impl Graphics<'_> {
    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let area = self.clip.intersect(Rect {
            x: x.saturating_add(self.tx),
            y: y.saturating_add(self.ty),
            width: width.max(0),
            height: height.max(0),
        });
        if area.width == 0 || area.height == 0 {
            return;
        }
        for py in area.y..area.y + area.height {
            let start = (py as u32 * self.target.width + area.x as u32) as usize;
            self.target.pixels[start..start + area.width as usize].fill(self.color);
        }
    }
    pub fn draw_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        if width < 0 || height < 0 {
            return;
        }
        let right = x.saturating_add(width);
        let bottom = y.saturating_add(height);
        self.draw_line(x, y, right, y);
        self.draw_line(x, y, x, bottom);
        self.draw_line(right, y, right, bottom);
        self.draw_line(x, bottom, right, bottom);
    }
    pub fn draw_round_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        arc_width: i32,
        arc_height: i32,
    ) {
        if width < 0 || height < 0 {
            return;
        }
        let rx = (arc_width.max(0) / 2).min(width / 2);
        let ry = (arc_height.max(0) / 2).min(height / 2);
        let right = x.saturating_add(width);
        let bottom = y.saturating_add(height);
        let diameter_x = rx.saturating_mul(2);
        let diameter_y = ry.saturating_mul(2);
        self.draw_line(x.saturating_add(rx), y, right.saturating_sub(rx), y);
        self.draw_line(
            x.saturating_add(rx),
            bottom,
            right.saturating_sub(rx),
            bottom,
        );
        self.draw_line(x, y.saturating_add(ry), x, bottom.saturating_sub(ry));
        self.draw_line(
            right,
            y.saturating_add(ry),
            right,
            bottom.saturating_sub(ry),
        );
        self.draw_arc(x, y, diameter_x, diameter_y, 90, 90);
        self.draw_arc(
            right.saturating_sub(diameter_x),
            y,
            diameter_x,
            diameter_y,
            0,
            90,
        );
        self.draw_arc(
            x,
            bottom.saturating_sub(diameter_y),
            diameter_x,
            diameter_y,
            180,
            90,
        );
        self.draw_arc(
            right.saturating_sub(diameter_x),
            bottom.saturating_sub(diameter_y),
            diameter_x,
            diameter_y,
            270,
            90,
        );
    }
    pub fn fill_round_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        arc_width: i32,
        arc_height: i32,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }
        let rx = (arc_width.max(0) / 2).min(width / 2);
        let ry = (arc_height.max(0) / 2).min(height / 2);
        let right = x.saturating_add(width);
        let bottom = y.saturating_add(height);
        let diameter_x = rx.saturating_mul(2);
        let diameter_y = ry.saturating_mul(2);
        self.fill_rect(
            x.saturating_add(rx),
            y,
            width.saturating_sub(diameter_x),
            height,
        );
        self.fill_rect(
            x,
            y.saturating_add(ry),
            rx,
            height.saturating_sub(diameter_y),
        );
        self.fill_rect(
            right.saturating_sub(rx),
            y.saturating_add(ry),
            rx,
            height.saturating_sub(diameter_y),
        );
        self.fill_arc(x, y, diameter_x, diameter_y, 90, 90);
        self.fill_arc(
            right.saturating_sub(diameter_x),
            y,
            diameter_x,
            diameter_y,
            0,
            90,
        );
        self.fill_arc(
            x,
            bottom.saturating_sub(diameter_y),
            diameter_x,
            diameter_y,
            180,
            90,
        );
        self.fill_arc(
            right.saturating_sub(diameter_x),
            bottom.saturating_sub(diameter_y),
            diameter_x,
            diameter_y,
            270,
            90,
        );
    }
    pub fn draw_arc(&mut self, x: i32, y: i32, width: i32, height: i32, start: i32, angle: i32) {
        self.arc(x, y, width, height, start, angle, false);
    }
    pub fn fill_arc(&mut self, x: i32, y: i32, width: i32, height: i32, start: i32, angle: i32) {
        self.arc(x, y, width, height, start, angle, true);
    }
    fn arc(&mut self, x: i32, y: i32, width: i32, height: i32, start: i32, angle: i32, fill: bool) {
        if width < 0
            || height < 0
            || angle == 0
            || (fill && (width == 0 || height == 0))
            || self.clip.width == 0
            || self.clip.height == 0
        {
            return;
        }
        let cx = clamp_i64(i64::from(x) + i64::from(self.tx) + i64::from(width) / 2);
        let cy = clamp_i64(i64::from(y) + i64::from(self.ty) + i64::from(height) / 2);
        let max_steps = i64::from(self.target.width + self.target.height) * 8;
        let mut previous = (cx, cy);
        for (index, (point, repeated)) in
            ellipse_arc_samples((cx, cy), (width, height), start, angle, max_steps).enumerate()
        {
            if fill {
                // Fill between adjacent radii. Isolated radial lines leave
                // holes when neighboring boundary samples are several pixels apart.
                self.fill_absolute_triangle([(cx, cy), previous, point]);
                self.draw_absolute_line(
                    previous.0,
                    previous.1,
                    point.0,
                    point.1,
                    StrokeStyle::Solid,
                );
            } else if index > 0 {
                self.draw_absolute_line(previous.0, previous.1, point.0, point.1, self.stroke);
            }
            if !fill && repeated {
                // Repeated samples used to draw a zero-length line. Its first
                // pixel is set even when the preceding dotted segment skips it.
                self.put(point.0, point.1, self.color);
            }
            previous = point;
        }
        if fill {
            self.draw_absolute_line(cx, cy, previous.0, previous.1, StrokeStyle::Solid);
        }
    }
    pub fn fill_triangle(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32) {
        let points = [
            (x1.saturating_add(self.tx), y1.saturating_add(self.ty)),
            (x2.saturating_add(self.tx), y2.saturating_add(self.ty)),
            (x3.saturating_add(self.tx), y3.saturating_add(self.ty)),
        ];
        self.fill_absolute_triangle(points);
    }
    fn fill_absolute_triangle(&mut self, points: [(i32, i32); 3]) {
        for (y, xs) in crate::clipped_triangle_rows(points, self.clip) {
            if xs.is_empty() {
                continue;
            }
            let row = y as usize * self.target.width as usize;
            self.target.pixels[row + *xs.start() as usize..=row + *xs.end() as usize]
                .fill(self.color);
        }
    }
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.draw_absolute_line(
            x0.saturating_add(self.tx),
            y0.saturating_add(self.ty),
            x1.saturating_add(self.tx),
            y1.saturating_add(self.ty),
            self.stroke,
        );
    }
    fn draw_absolute_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, stroke: StrokeStyle) {
        for (x, y, step) in clipped_line_points((x0, y0), (x1, y1), self.clip) {
            if stroke == StrokeStyle::Solid || step.is_multiple_of(2) {
                self.put(x, y, self.color);
            }
        }
    }
}

/// Distinct angular samples of the deterministic LCDUI ellipse, including both
/// endpoints. The flag records omitted repetitions, whose zero-length outline
/// segments still set a pixel in dotted mode. At most 361 samples are produced.
pub fn ellipse_arc_samples(
    center: (i32, i32),
    size: (i32, i32),
    start: i32,
    angle: i32,
    max_steps: i64,
) -> impl Iterator<Item = ((i32, i32), bool)> {
    let sweep = angle.clamp(-360, 360);
    let span = i64::from(sweep.unsigned_abs());
    let steps = (i64::from(size.0.max(size.1).max(0)) * span / 45).clamp(1, max_steps.max(1));
    let mut next_sample = 0;
    std::iter::from_fn(move || {
        if span == 0 || next_sample > steps {
            return None;
        }
        let sample = next_sample;
        let offset = span * sample / steps;
        next_sample = ((steps * (offset + 1) + span - 1) / span).min(steps + 1);
        let point = ellipse_point(
            center,
            (size.0 / 2, size.1 / 2),
            i64::from(start) + i64::from(sweep.signum()) * offset,
        );
        Some((point, next_sample > sample + 1))
    })
}

const TRIG_SCALE: i64 = 65_536;
fn rounded_scale(radius: i64, factor: i64) -> i64 {
    let value = radius * factor;
    if value >= 0 {
        (value + TRIG_SCALE / 2) / TRIG_SCALE
    } else {
        (value - TRIG_SCALE / 2) / TRIG_SCALE
    }
}
/// One point on the deterministic integer ellipse used by LCDUI arcs.
/// Zero degrees points right; positive angles run counterclockwise on screen.
/// The Bhaskara-I approximation avoids host libm differences, and coordinates
/// saturate at the Java int limits.
#[must_use]
pub fn ellipse_point(center: (i32, i32), radii: (i32, i32), degrees: i64) -> (i32, i32) {
    let degrees = degrees.rem_euclid(360);
    (
        clamp_i64(i64::from(center.0) + rounded_scale(i64::from(radii.0), fixed_sin(degrees + 90))),
        clamp_i64(i64::from(center.1) - rounded_scale(i64::from(radii.1), fixed_sin(degrees))),
    )
}
fn fixed_sin(degrees: i64) -> i64 {
    // Arcs sample whole degrees. Evaluate the same integer approximation at
    // compile time, preserving its rounding without dividing at every point.
    const SINES: [i32; 360] = {
        let mut values = [0; 360];
        let mut angle = 0;
        while angle < values.len() {
            let (x, sign) = if angle <= 180 {
                (angle as i64, 1)
            } else {
                (angle as i64 - 180, -1)
            };
            let product = x * (180 - x);
            values[angle] = (sign * 4 * product * TRIG_SCALE / (40_500 - product)) as i32;
            angle += 1;
        }
        values
    };
    i64::from(SINES[degrees.rem_euclid(360) as usize])
}
/// Visible Bresenham pixels and their step index from the original first point.
/// Skips hidden steps arithmetically, preserving both rounding and dotted phase.
/// Work is bounded by the clip's extent on the line's major axis.
pub fn clipped_line_points(
    start: (i32, i32),
    end: (i32, i32),
    clip: Rect,
) -> impl Iterator<Item = (i32, i32, u64)> {
    let dx = (i64::from(end.0) - i64::from(start.0)).unsigned_abs();
    let dy = (i64::from(end.1) - i64::from(start.1)).unsigned_abs();
    let sx = if start.0 < end.0 { 1_i64 } else { -1 };
    let sy = if start.1 < end.1 { 1_i64 } else { -1 };
    let x_major = dx >= dy;
    let (major, minor, origin, direction, low, length) = if x_major {
        (dx, dy, start.0, sx, clip.x, clip.width)
    } else {
        (dy, dx, start.1, sy, clip.y, clip.height)
    };
    let origin = i64::from(origin);
    let low = i64::from(low);
    let high = low + i64::from(length) - 1;
    let (first, last) = if clip.width <= 0 || clip.height <= 0 {
        (1, 0)
    } else if direction > 0 {
        ((low - origin).max(0), (high - origin).min(major as i64))
    } else {
        ((origin - high).max(0), (origin - low).min(major as i64))
    };

    // Java int endpoint differences fit u32; their product plus half a
    // difference fits u64. Initialize the original recurrence at `first`,
    // instead of rounding clipped endpoints and starting a different line.
    let advance = first as u64 * minor + major / 2;
    let (minor_steps, remainder) = advance
        .checked_div(major)
        .map_or((0, 0), |steps| (steps as i64, (advance % major) as i64));
    let mut x = i64::from(start.0) + sx * if x_major { first } else { minor_steps };
    let mut y = i64::from(start.1) + sy * if x_major { minor_steps } else { first };
    let mut error = dx as i64 - dy as i64
        + if x_major {
            (major / 2) as i64 - remainder
        } else {
            remainder - (major / 2) as i64
        };
    (first..=last)
        .map(move |step| {
            let point = (x as i32, y as i32, step as u64);
            let twice = error * 2;
            if twice >= -(dy as i64) {
                error -= dy as i64;
                x += sx;
            }
            if twice <= dx as i64 {
                error += dx as i64;
                y += sy;
            }
            point
        })
        .filter(move |&(x, y, _)| {
            x >= clip.x
                && y >= clip.y
                && i64::from(x) < i64::from(clip.x) + i64::from(clip.width)
                && i64::from(y) < i64::from(clip.y) + i64::from(clip.height)
        })
}

#[cfg(test)]
#[path = "../../../tests/unit/graphics/primitives.rs"]
mod tests;
