use super::*;

#[test]
fn floating_point_strings_keep_java_notation_and_special_values() {
    for (value, expected) in [
        (f64::NAN, "NaN"),
        (f64::INFINITY, "Infinity"),
        (f64::NEG_INFINITY, "-Infinity"),
        (0.0, "0.0"),
        (-0.0, "-0.0"),
        (1.0, "1.0"),
        (-12.5, "-12.5"),
        (0.001, "0.001"),
        (0.0001, "1.0E-4"),
        (9_999_999.0, "9999999.0"),
        (10_000_000.0, "1.0E7"),
        (1.25e20, "1.25E20"),
        (-1.25e-20, "-1.25E-20"),
    ] {
        assert_eq!(format_java_float(value), expected);
        assert_eq!(format_java_float(value as f32), expected);
    }
}

#[test]
fn floating_point_strings_round_trip_across_exponents() {
    for exponent in 0..=254_u32 {
        for fraction in [0, 1, 0x0012_3456, 0x0040_0000, 0x007f_fffe, 0x007f_ffff] {
            for sign in [0, 1 << 31] {
                let bits = sign | exponent << 23 | fraction;
                let text = format_java_float(f32::from_bits(bits));
                assert_eq!(parse_java_f32(&text).unwrap().to_bits(), bits, "{text}");
                assert!(text.contains('.'), "{text}");
                assert!(text.len() <= 16, "{text}");
            }
        }
    }
    for exponent in 0..=2046_u64 {
        for fraction in [
            0,
            1,
            0x0001_2345_6789_abcd,
            1 << 51,
            (1 << 52) - 2,
            (1 << 52) - 1,
        ] {
            for sign in [0, 1 << 63] {
                let bits = sign | exponent << 52 | fraction;
                let text = format_java_float(f64::from_bits(bits));
                assert_eq!(parse_java_f64(&text).unwrap().to_bits(), bits, "{text}");
                assert!(text.contains('.'), "{text}");
                assert!(text.len() <= 25, "{text}");
            }
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn floating_point_format_throughput() {
    use std::{hint::black_box, time::Instant};
    for (name, values) in [
        ("ordinary", [0.0, -0.0, 1.25, -1234.5, 9_999_999.0, 0.001]),
        (
            "scientific",
            [
                1.0e-100,
                -1.25e-200,
                f64::from_bits(1),
                1.0e100,
                1.25e200,
                f64::MAX,
            ],
        ),
    ] {
        let started = Instant::now();
        let mut checksum = 0_u64;
        for _ in 0..100_000 {
            for value in values {
                for byte in black_box(format_java_float(black_box(value))).bytes() {
                    checksum = checksum.wrapping_mul(31).wrapping_add(u64::from(byte));
                }
            }
        }
        eprintln!("{name}: {:?}; checksum={checksum}", started.elapsed());
    }
}
