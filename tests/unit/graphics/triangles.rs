use std::collections::BTreeSet;

use super::clipped_triangle_rows;
use crate::Rect;

fn signed_area(a: (i32, i32), b: (i32, i32), p: (i32, i32)) -> i128 {
    (i128::from(b.0) - i128::from(a.0)) * (i128::from(p.1) - i128::from(a.1))
        - (i128::from(b.1) - i128::from(a.1)) * (i128::from(p.0) - i128::from(a.0))
}

fn pixels(points: [(i32, i32); 3], clip: Rect) -> BTreeSet<(i32, i32)> {
    let mut pixels = BTreeSet::new();
    for (y, xs) in clipped_triangle_rows(points, clip) {
        for x in xs {
            assert!(x >= clip.x && i64::from(x) < i64::from(clip.x) + i64::from(clip.width));
            assert!(y >= clip.y && i64::from(y) < i64::from(clip.y) + i64::from(clip.height));
            assert!(pixels.insert((x, y)), "pixel blended more than once");
        }
    }
    pixels
}

// Independent reference: test each clipped pixel against all three edges.
// Degenerate cases use the original full Bresenham walk (small coordinates).
fn reference(points: [(i32, i32); 3], clip: Rect) -> BTreeSet<(i32, i32)> {
    if signed_area(points[0], points[1], points[2]) != 0 {
        let mut expected = BTreeSet::new();
        for y in clip.y..clip.y + clip.height {
            for x in clip.x..clip.x + clip.width {
                let signs = [
                    signed_area(points[0], points[1], (x, y)),
                    signed_area(points[1], points[2], (x, y)),
                    signed_area(points[2], points[0], (x, y)),
                ];
                if signs.iter().all(|&v| v >= 0) || signs.iter().all(|&v| v <= 0) {
                    expected.insert((x, y));
                }
            }
        }
        return expected;
    }
    let (mut x, mut y) = *points.iter().min().unwrap();
    let (end_x, end_y) = *points.iter().max().unwrap();
    let dx = (end_x - x).abs();
    let dy = -(end_y - y).abs();
    let sx = if x < end_x { 1 } else { -1 };
    let sy = if y < end_y { 1 } else { -1 };
    let mut error = dx + dy;
    let mut expected = BTreeSet::new();
    loop {
        if x >= clip.x && x < clip.x + clip.width && y >= clip.y && y < clip.y + clip.height {
            expected.insert((x, y));
        }
        if (x, y) == (end_x, end_y) {
            return expected;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}

#[test]
fn row_spans_match_per_pixel_coverage_for_all_vertex_orders() {
    let mut state = 0x37da_1925_u32;
    for _ in 0..2_048 {
        let mut coordinate = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state % 33) as i32 - 16
        };
        let points = std::array::from_fn::<_, 3, _>(|_| (coordinate(), coordinate()));
        let clip = Rect {
            x: -5,
            y: -4,
            width: 11,
            height: 9,
        };
        let expected = reference(points, clip);
        for [a, b, c] in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            assert_eq!(
                pixels([points[a], points[b], points[c]], clip),
                expected,
                "{points:?}"
            );
        }
    }
}

#[test]
fn row_spans_handle_empty_clips_and_extreme_coordinates() {
    let clip = Rect {
        x: -2,
        y: -2,
        width: 5,
        height: 5,
    };
    let cases = [
        [(i32::MIN, i32::MIN), (i32::MAX, i32::MIN), (0, i32::MAX)],
        [(i32::MIN, i32::MAX), (i32::MAX, i32::MAX), (0, i32::MIN)],
        [(i32::MIN, 0), (i32::MAX, 1), (i32::MAX, 2)],
        [(0, i32::MIN), (1, i32::MAX), (2, i32::MAX)],
    ];
    for points in cases {
        assert_eq!(pixels(points, clip), reference(points, clip));
        for (width, height) in [(0, 5), (5, 0), (-1, 5), (5, -1)] {
            assert_eq!(
                clipped_triangle_rows(
                    points,
                    Rect {
                        width,
                        height,
                        ..clip
                    }
                )
                .count(),
                0
            );
        }
    }
    for origin in [i32::MIN, i32::MAX - 2] {
        let clip = Rect {
            x: origin,
            y: origin,
            width: 3,
            height: 3,
        };
        let points = [
            (origin, origin),
            (origin + 2, origin),
            (origin + 2, origin + 2),
        ];
        let expected = [(0, 0), (1, 0), (2, 0), (1, 1), (2, 1), (2, 2)]
            .map(|(x, y)| (origin + x, origin + y))
            .into_iter()
            .collect();
        assert_eq!(pixels(points, clip), expected);
    }
    let points = [(i32::MIN, i32::MIN), (0, 0), (i32::MAX, i32::MAX)];
    assert_eq!(pixels(points, clip), (-2..=2).map(|v| (v, v)).collect());
}

#[test]
fn thin_triangles_retain_empty_rows_for_cancellation() {
    let points = [(0, -1), (1, 999), (1, 1000)];
    let rows = clipped_triangle_rows(
        points,
        Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 999,
        },
    )
    .collect::<Vec<_>>();
    assert_eq!(rows.len(), 999);
    assert!(rows.iter().all(|(_, xs)| xs.is_empty()));
}

#[test]
fn row_spans_match_coverage_with_large_slopes_and_long_empty_runs() {
    let mut state = 0xc4ab_3719_u32;
    for index in 0..1_024 {
        let mut coordinate = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as i32
        };
        let mut points = std::array::from_fn::<_, 3, _>(|_| (coordinate(), coordinate()));
        if index % 2 == 0 {
            points[1].1 = points[0].1.saturating_add(1);
        }
        let (x, y) = [
            (-3, -256),
            (i32::MIN, -256),
            (-3, i32::MIN),
            (-3, i32::MAX - 512),
            (i32::MAX - 7, -256),
        ][index % 5];
        let clip = Rect {
            x,
            y,
            width: 7,
            height: 512,
        };
        assert_eq!(pixels(points, clip), reference(points, clip), "{points:?}");
    }
}

#[test]
#[ignore = "synthetic release throughput measurement"]
fn triangle_rows_throughput() {
    use std::{hint::black_box, time::Instant};

    let cases = [
        ("small", [(1, 1), (7, 2), (3, 7)]),
        ("ordinary", [(2, 7), (237, 49), (67, 319)]),
        ("thin", [(0, -1), (1, 999), (1, 1000)]),
        ("extreme", [(i32::MIN, 0), (i32::MAX, 1), (3, 319)]),
        ("flat", [(0, 100), (239, 100), (0, 319)]),
        ("line", [(0, 0), (239, 239), (119, 119)]),
        ("hidden", [(1, -3), (7, -2), (3, -7)]),
    ];
    let clip = Rect {
        x: 0,
        y: 0,
        width: 240,
        height: 320,
    };
    for (name, points) in cases {
        let mut checksum = 0_u64;
        let started = Instant::now();
        for _ in 0..16_384 {
            for (y, xs) in clipped_triangle_rows(black_box(points), black_box(clip)) {
                checksum = checksum
                    .wrapping_mul(31)
                    .wrapping_add(y as u64)
                    .wrapping_add((*xs.start() as u64).wrapping_mul(7))
                    .wrapping_add((*xs.end() as u64).wrapping_mul(13));
            }
        }
        println!(
            "triangle_rows {name} ns={} checksum={checksum}",
            started.elapsed().as_nanos()
        );
        black_box(checksum);

        let mut frame = crate::Framebuffer::new(240, 320).unwrap();
        let started = Instant::now();
        for color in 0..1_024 {
            let mut graphics = frame.graphics();
            graphics.set_color(black_box(color));
            let [(x1, y1), (x2, y2), (x3, y3)] = black_box(points);
            graphics.fill_triangle(x1, y1, x2, y2, x3, y3);
            black_box(&frame);
        }
        let elapsed = started.elapsed();
        let checksum = frame
            .pixels()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, &pixel| {
                hash.wrapping_mul(0x100_0000_01b3) ^ u64::from(pixel)
            });
        println!(
            "triangle_fill {name} ns={} checksum={checksum}",
            elapsed.as_nanos()
        );
    }
}
