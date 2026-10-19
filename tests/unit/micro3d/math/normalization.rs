use super::*;

#[test]
fn normal_length_matches_integer_sqrt_at_square_boundaries_and_full_width_inputs() {
    for squared in 0..65536 {
        assert_eq!(normal_length(squared), squared.isqrt());
    }
    for root in [
        0_u64,
        1,
        2,
        3,
        4095,
        4096,
        (1 << 30) - 1,
        1 << 30,
        (1 << 31) - 1,
        1 << 31,
        (1 << 32) - 2,
        (1 << 32) - 1,
    ] {
        for offset in -4_i64..=4 {
            if let Some(squared) = (root * root).checked_add_signed(offset) {
                assert_eq!(normal_length(squared), squared.isqrt(), "{squared}");
            }
        }
    }
    let mut seed = 0xa1f5_e384_20c7_d69b_u64;
    for _ in 0..100_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        assert_eq!(normal_length(seed), seed.isqrt(), "{seed}");
    }
    assert_eq!(normal_length(u64::MAX), u64::MAX.isqrt());
}

// Integer-only oracle for the established fixed-point normalization contract.
fn integer_normalize(input: [i128; 3]) -> Vector3D {
    let magnitudes = input.map(i128::unsigned_abs);
    let Some(exponent) = magnitudes.into_iter().max().unwrap().checked_ilog2() else {
        return Vector3D::default();
    };
    let scaled = magnitudes.map(|magnitude| {
        if exponent > 30 {
            (magnitude >> (exponent - 30)) as u64
        } else {
            (magnitude << (30 - exponent)) as u64
        }
    });
    let length = (scaled[0] * scaled[0] + scaled[1] * scaled[1] + scaled[2] * scaled[2]).isqrt();
    let component = |index: usize| {
        let value = ((scaled[index] * 4096 + length / 2) / length) as i32;
        if input[index] < 0 { -value } else { value }
    };
    Vector3D::new(component(0), component(1), component(2))
}

#[test]
fn normalization_keeps_integer_rounding_for_wide_and_small_vectors() {
    let coordinates = [
        i128::MIN,
        i128::MAX,
        -(1 << 95),
        1 << 95,
        -4096,
        -1,
        0,
        1,
        4096,
    ];
    for x in coordinates {
        for y in coordinates {
            for z in coordinates {
                let input = [x, y, z];
                assert_eq!(normalize_i128(input), integer_normalize(input), "{input:?}");
            }
        }
    }
    let mut seed = 0xa1f5_e384_20c7_d69b_u64;
    for index in 0..100_000 {
        let input = std::array::from_fn(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            i128::from(seed.cast_signed()) << (index % 64)
        });
        assert_eq!(normalize_i128(input), integer_normalize(input), "{input:?}");
    }
}

#[test]
fn normalization_preserves_component_rounding_across_dense_fixed_point_steps() {
    for component in 0..65536 {
        for input in [
            [component, 32768, 12345],
            [32768, -component, 12345],
            [12345, 32768, -component],
        ] {
            assert_eq!(normalize_i128(input), integer_normalize(input), "{input:?}");
        }
    }
}

#[test]
#[ignore = "manual fixed-point vector normalization throughput measurement"]
fn normalization_throughput() {
    let mut seed = 0xa1f5_e384_20c7_d69b_u64;
    let inputs: Vec<[i128; 3]> = (0..4096)
        .map(|index| {
            std::array::from_fn(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                i128::from(seed.cast_signed()) << (index % 32)
            })
        })
        .collect();
    let started = std::time::Instant::now();
    let mut checksum = 0_u64;
    for _ in 0..64 {
        for input in &inputs {
            let output = normalize_i128(std::hint::black_box(*input));
            for value in [output.x, output.y, output.z] {
                checksum = checksum
                    .wrapping_mul(31)
                    .wrapping_add(u64::from(value.cast_unsigned()));
            }
        }
    }
    eprintln!(
        "normalization elapsed={:?} checksum={checksum:016x}",
        started.elapsed()
    );
}

fn wide_normal_oracle(transform: AffineTrans, normal: Vector3D) -> Vector3D {
    let matrix: [[i128; 3]; 3] = std::array::from_fn(|row| {
        std::array::from_fn(|column| i128::from(transform.values[row * 4 + column]))
    });
    let cofactors: [[i128; 3]; 3] = std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            let rows: Vec<_> = (0..3).filter(|candidate| *candidate != row).collect();
            let columns: Vec<_> = (0..3).filter(|candidate| *candidate != column).collect();
            let minor = matrix[rows[0]][columns[0]] * matrix[rows[1]][columns[1]]
                - matrix[rows[0]][columns[1]] * matrix[rows[1]][columns[0]];
            if (row + column).is_multiple_of(2) {
                minor
            } else {
                -minor
            }
        })
    });
    let determinant: i128 = (0..3)
        .map(|column| matrix[0][column] * cofactors[0][column])
        .sum();
    let input = [normal.x, normal.y, normal.z].map(i128::from);
    integer_normalize(std::array::from_fn(|row| {
        determinant.signum()
            * (0..3)
                .map(|column| cofactors[row][column] * input[column])
                .sum::<i128>()
    }))
}

#[test]
fn prepared_normals_preserve_full_width_cofactors_and_determinants() {
    let edges = [i32::MIN, i32::MIN + 1, -4096, -1, 0, 1, 4096, i32::MAX];
    let mut seed = 0x8f31_cab4_1d62_059e_u64;
    for index in 0..20_000 {
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            if index < 4096 {
                edges[(seed as usize) % edges.len()]
            } else {
                (seed as u32).cast_signed()
            }
        };
        let transform = AffineTrans::new(std::array::from_fn(|_| next()));
        let normal = Vector3D::new(next(), next(), next());
        let prepared = transform.normal_transform();
        for input in [
            normal,
            Vector3D::new(i32::MIN, i32::MAX, 1),
            Vector3D::default(),
        ] {
            let expected = wide_normal_oracle(transform, input);
            assert_eq!(prepared(input), expected, "{transform:?} {input:?}");
            assert_eq!(transform.transform_normal(input), expected);
        }
    }
}

#[test]
#[ignore = "manual full-width inverse transpose normal throughput measurement"]
fn prepared_normal_transform_throughput() {
    let transforms = [
        ("identity", AffineTrans::IDENTITY),
        (
            "translation",
            AffineTrans::new([4096, 0, 0, 97, 0, 4096, 0, -34, 0, 0, 4096, 18]),
        ),
        (
            "scaled",
            AffineTrans::new([8192, 0, 0, 0, 0, 4096, 0, 0, 0, 0, 2048, 0]),
        ),
        (
            "shear",
            AffineTrans::new([4096, 512, -64, 23, 128, 4096, 32, -21, -512, 128, 4096, 12]),
        ),
        (
            "mirrored",
            AffineTrans::new([-8192, 0, 0, 0, 0, 4096, 0, 0, 0, 0, 2048, 0]),
        ),
        (
            "full-width",
            AffineTrans::new([
                i32::MIN,
                i32::MAX,
                -1,
                0,
                i32::MAX,
                i32::MIN,
                1,
                0,
                1,
                -1,
                i32::MAX,
                0,
            ]),
        ),
        ("singular", AffineTrans::new([i32::MAX; 12])),
    ];
    let mut seed = 0xf831_045d_1c82_6a9b_u64;
    let normals: Vec<_> = (0..4096)
        .map(|_| {
            let mut next = || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed as u32).cast_signed()
            };
            Vector3D::new(next(), next(), next())
        })
        .collect();
    for (name, transform) in transforms {
        let prepared = std::hint::black_box(transform).normal_transform();
        let started = std::time::Instant::now();
        let mut checksum = 0_u64;
        for _ in 0..64 {
            for normal in &normals {
                let output = prepared(std::hint::black_box(*normal));
                for value in [output.x, output.y, output.z] {
                    checksum = checksum
                        .wrapping_mul(31)
                        .wrapping_add(u64::from(value.cast_unsigned()));
                }
            }
        }
        eprintln!(
            "prepared-normal matrix={name} elapsed={:?} checksum={checksum:016x}",
            started.elapsed()
        );
    }
}
