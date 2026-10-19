use super::{
    InterpolationDivisor, bounded_area_weight, covered_span, interpolation_division_is_bounded,
};

#[test]
fn shared_interpolation_division_preserves_signed_zero_and_extreme_normal_rounding() {
    let mut seed = 0x0018_4220_2654_u64;
    for _ in 0..100_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let exponent = -130 + (seed % 281) as i32;
        let denominator =
            f64::from_bits(((1023 + exponent) as u64) << 52 | (seed & ((1_u64 << 52) - 1)));
        let exponent = -475 + (seed.rotate_left(19) % 778) as i32;
        let numerator = f64::from_bits(
            (seed & (1_u64 << 63))
                | ((1023 + exponent) as u64) << 52
                | (seed.rotate_left(31) & ((1_u64 << 52) - 1)),
        );
        let divisor = InterpolationDivisor::new(denominator, true);
        for value in [numerator, -numerator, 0.0, -0.0] {
            assert_eq!(
                divisor.divide(value).to_bits(),
                (value / denominator).to_bits(),
                "{value}/{denominator}"
            );
        }
    }
}

#[test]
fn shared_interpolation_rejects_attributes_outside_the_normal_arithmetic_bounds() {
    let vertex = super::ScreenVertex {
        inverse_w: 1.0,
        color_over_w: [0.25, -0.5, 1.0, -0.0],
        ..super::ScreenVertex::default()
    };
    assert!(interpolation_division_is_bounded(&[vertex; 3]));
    for value in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MIN_POSITIVE,
        f64::from_bits((1023_u64 - 257) << 52),
        f64::from_bits((1023_u64 + 301) << 52),
    ] {
        let mut screen = [vertex; 3];
        screen[1].texture_over_w[1][2] = value;
        assert!(!interpolation_division_is_bounded(&screen));
    }
    for value in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        let mut screen = [vertex; 3];
        screen[0].inverse_w = value;
        assert!(!interpolation_division_is_bounded(&screen));
    }
}

#[test]
fn large_projected_areas_retain_division_and_the_target_work_budget() {
    let limits = super::super::RenderLimits {
        max_viewport_width: 1 << 20,
        max_viewport_height: 1 << 20,
        ..super::super::RenderLimits::default()
    };
    let mut renderer = super::SoftwareRenderer::new(1, 1, limits).unwrap();
    renderer.set_viewport(0, 0, 1 << 20, 1 << 20).unwrap();
    renderer
        .draw_triangle([
            super::Vertex::new(crate::Vec4::new(-1.0, 1.0, 0.0, 1.0), 0xffd0_4020),
            super::Vertex::new(crate::Vec4::new(1.0, 1.0, 0.0, 1.0), 0xff20_d040),
            super::Vertex::new(crate::Vec4::new(-1.0, -1.0, 0.0, 1.0), 0xff40_20d0),
        ])
        .unwrap();
    assert_eq!(renderer.pixels(), [0xffd0_4020]);
    assert_eq!(renderer.stats().tested_fragments, 1);
    assert_eq!(renderer.stats().shaded_fragments, 1);
}

#[test]
fn bounded_area_weights_preserve_exact_division_rounding() {
    let check = |numerator: u64, area: u64| {
        let expected = numerator as f64 / area as f64;
        let actual = bounded_area_weight(numerator as i64, area as f64, (area as f64).recip());
        assert_eq!(actual.to_bits(), expected.to_bits(), "{numerator}/{area}");
    };
    for area in 1..=256 {
        for numerator in 0..=area {
            check(numerator, area);
        }
    }
    let mut seed = 0x0018_4220_2653_u64;
    for _ in 0..100_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let area = (seed & ((1_u64 << 40) - 1)).max(1);
        for numerator in [
            0,
            1,
            seed.rotate_left(21) % (area + 1),
            area / 2,
            area - 1,
            area,
        ] {
            check(numerator, area);
        }
    }
}

#[test]
fn bounded_area_weights_preserve_ratios_nearest_to_binary64_midpoints() {
    let power_mod = |mut base: u64, mut power: u32, modulus: u64| {
        let mut result = 1_u64;
        while power != 0 {
            if power & 1 != 0 {
                result = ((u128::from(result) * u128::from(base)) % u128::from(modulus)) as u64;
            }
            base = ((u128::from(base) * u128::from(base)) % u128::from(modulus)) as u64;
            power >>= 1;
        }
        result
    };
    for offset in 1..=8192 {
        let area = (1_u64 << 40) - offset * 2 - 1;
        let inverse = power_mod(area.div_ceil(2), 54, area);
        let numerator = inverse.max(area - inverse);
        let expected = numerator as f64 / area as f64;
        let actual = bounded_area_weight(numerator as i64, area as f64, (area as f64).recip());
        assert_eq!(actual.to_bits(), expected.to_bits(), "{numerator}/{area}");
    }
}

#[test]
fn row_spans_match_individual_top_left_edge_tests() {
    let mut random = 0x1843_2026_u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random
    };
    // Include horizontal edges, exact edge hits, either winding, empty rows,
    // and coefficients near the rasterizer's coordinate guard range.
    for sample in 0..20_000 {
        let scale = if sample % 2 == 0 { 1 } else { 1_i64 << 40 };
        let edges = std::array::from_fn(|_| (i64::from(next() % 1025) - 512) * scale);
        let steps = std::array::from_fn(|_| (i64::from(next() % 65) - 32) * scale);
        let top_left = std::array::from_fn(|_| next() & 1 != 0);
        let width = (next() % 65) as i32;
        let expected: Vec<_> = (0..width)
            .filter(|x| {
                (0..3).all(|index| {
                    let edge = edges[index] + i64::from(*x) * steps[index];
                    edge > 0 || edge == 0 && top_left[index]
                })
            })
            .collect();
        assert_eq!(
            covered_span(edges, steps, top_left, width).collect::<Vec<_>>(),
            expected,
            "edges={edges:?} steps={steps:?} top_left={top_left:?} width={width}",
        );
    }
}
