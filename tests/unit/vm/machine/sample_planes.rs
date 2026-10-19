use super::*;

#[test]
fn sample_comparison_detects_changes_in_each_phase_and_row() {
    for width in [1_u32, 3, 63, 64, 65, 127, 128, 129, 240] {
        for height in [1_u32, 3] {
            let source = vec![HeapValue::Int(0); (width * height) as usize];
            let mut expected: Vec<_> = (0..width * height * 4)
                .map(|index| index.wrapping_mul(0x6134_47af))
                .collect();
            for promoted in [false, true] {
                let mut planes = SamplePlanes::new(&source).unwrap().unwrap();
                if promoted {
                    planes.prepare_guest_planes(|_| Ok(())).unwrap();
                }
                planes.store(
                    width as usize,
                    &expected,
                    [0, 0, width, height],
                    false,
                    None,
                );
                assert!(planes.matches(width, &expected));
                for row in 0..height {
                    for phase in 0..4 {
                        for column in [0, width / 2, width - 1] {
                            let index = ((row * 2 + phase / 2) * width * 2 + column * 2 + phase % 2)
                                as usize;
                            expected[index] ^= 1;
                            assert!(!planes.matches(width, &expected));
                            expected[index] ^= 1;
                        }
                    }
                }
                assert!(planes.matches(width, &expected));
            }
        }
    }
}

#[test]
fn sample_comparison_rejects_non_integer_samples_with_matching_payload_bits() {
    let mut planes = SamplePlanes::new(&[HeapValue::Int(0); 65])
        .unwrap()
        .unwrap();
    planes.prepare_guest_planes(|_| Ok(())).unwrap();
    let expected = [0; 65 * 4];
    for phase in 0..4 {
        for index in [0, 63, 64] {
            planes.guest_plane_mut(phase).unwrap()[index] = HeapValue::Float(0.0);
            assert!(!planes.matches(65, &expected));
            planes.guest_plane_mut(phase).unwrap()[index] = HeapValue::Int(0);
        }
    }
    assert!(planes.matches(65, &expected));
}

#[test]
fn sample_promotion_keeps_clipped_samples_and_rounded_guest_pixels() {
    let initial: Vec<_> = (0..6).map(|i| HeapValue::Int(0x1020_3040 + i)).collect();
    let samples: Vec<_> = (1_u32..=24).map(|i| i.wrapping_mul(0x1020_3040)).collect();
    for clip in [[0, 0, 3, 2], [1, 0, 2, 2], [0, 1, 2, 1], [0, 0, 0, 2]] {
        for opaque in [false, true] {
            for promote_before in [false, true] {
                let mut planes = SamplePlanes::new(&initial).unwrap().unwrap();
                if promote_before {
                    planes.prepare_guest_planes(|_| Ok(())).unwrap();
                }
                let mut expected = vec![0; 24];
                let [x, y, width, height] = clip;
                for row in 0..4 {
                    for column in 0..6 {
                        let index = row * 6 + column;
                        expected[index] = if (x * 2..(x + width) * 2).contains(&(column as u32))
                            && (y * 2..(y + height) * 2).contains(&(row as u32))
                        {
                            samples[index] | if opaque { 0xff00_0000 } else { 0 }
                        } else {
                            0x1020_3040 + (row / 2 * 3 + column / 2) as u32
                        };
                    }
                }
                let mut destination = initial.clone();
                planes.store(3, &samples, clip, opaque, Some(&mut destination));
                assert_eq!(planes.interleave(3, 2).unwrap(), expected);
                for row in 0..2 {
                    for column in 0..3 {
                        let i = row * 12 + column * 2;
                        let values = [
                            expected[i],
                            expected[i + 1],
                            expected[i + 6],
                            expected[i + 7],
                        ];
                        let mut pixel = 0;
                        for shift in [0, 8, 16, 24] {
                            let sum: u32 = values.iter().map(|v| (v >> shift) & 255).sum();
                            pixel |= ((sum + 2) / 4) << shift;
                        }
                        assert!(
                            matches!(destination[row * 3 + column], HeapValue::Int(value)
                            if value.cast_unsigned() == pixel)
                        );
                    }
                }
                planes.prepare_guest_planes(|_| Ok(())).unwrap();
                assert_eq!(planes.interleave(3, 2).unwrap(), expected);
                assert!(planes.matches(3, &expected));
                expected[0] ^= 1;
                assert!(!planes.matches(3, &expected));
            }
        }
    }
}

#[test]
fn sample_storage_rejects_non_integer_sources_and_checked_pixel_bounds() {
    assert!(SamplePlanes::new(&[HeapValue::Float(1.0)]).is_err());
    let mut planes = SamplePlanes::new(&[HeapValue::Int(7); 6]).unwrap().unwrap();
    for promote in [false, true] {
        if promote {
            planes.prepare_guest_planes(|_| Ok(())).unwrap();
        }
        let before = planes.interleave(3, 2).unwrap();
        assert!(planes.composite_pixel(6, -1).is_err());
        assert!(planes.fill(0, 7, -1).is_err());
        assert_eq!(planes.interleave(3, 2).unwrap(), before);
    }
    assert!(planes.guest_plane_mut(4).is_err());
}

#[test]
fn cancelling_sample_promotion_preserves_every_sample() {
    let initial: Vec<_> = (0..4096).map(HeapValue::Int).collect();
    let mut planes = SamplePlanes::new(&initial).unwrap().unwrap();
    let before = planes.interleave(64, 64).unwrap();
    let error = planes
        .prepare_guest_planes(|work| {
            if work >= 2048 {
                Err(vm_error("execution-cancelled", "test cancellation"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.code(), "execution-cancelled");
    assert_eq!(planes.interleave(64, 64).unwrap(), before);
    planes.prepare_guest_planes(|_| Ok(())).unwrap();
    assert_eq!(planes.interleave(64, 64).unwrap(), before);
}

#[test]
#[ignore = "manual host sample comparison throughput measurement"]
fn sample_match_throughput() {
    for (width, height) in [(240_u32, 320_u32), (480, 640)] {
        let count = (width * height) as usize;
        let source: Vec<_> = (0..count)
            .map(|i| HeapValue::Int((i as i32).wrapping_mul(0x1218_4361)))
            .collect();
        for promoted in [false, true] {
            let mut planes = SamplePlanes::new(&source).unwrap().unwrap();
            if promoted {
                planes.prepare_guest_planes(|_| Ok(())).unwrap();
            }
            let expected = planes.interleave(width, height).unwrap();
            let repeats = (16_777_216 / count).clamp(32, 256);
            for position in [
                None,
                Some(0),
                Some(expected.len() / 2),
                Some(expected.len() - 1),
            ] {
                let mut pixels = expected.clone();
                if let Some(position) = position {
                    pixels[position] ^= 1;
                }
                let start = std::time::Instant::now();
                for _ in 0..repeats {
                    assert_eq!(
                        std::hint::black_box(planes.matches(width, std::hint::black_box(&pixels))),
                        position.is_none()
                    );
                }
                eprintln!(
                    "sample_match {width}x{height} promoted={promoted} changed={position:?} repeats={repeats} elapsed={:?}",
                    start.elapsed()
                );
            }
        }
    }
}
#[test]
fn sample_rows_match_scalar_compositing_with_late_transparency() {
    for count in [0, 1, 2, 15, 16, 17, 63, 64, 65, 1023, 1024, 1025] {
        let width = count + 4;
        let initial = vec![HeapValue::Int(0); width];
        let backgrounds: Vec<u32> = (0..width * 4)
            .map(|index| {
                [0, 0x4000_0000, 0x8000_0000, 0xff00_0000][index % 4]
                    | (index as u32).wrapping_mul(0x0005_374b) & 0x00ff_ffff
            })
            .collect();
        for promoted in [false, true] {
            for alpha in [0_u32, 128, 255] {
                for position in [None, Some(0), Some(count / 2), count.checked_sub(1)] {
                    let mut planes = SamplePlanes::new(&initial).unwrap().unwrap();
                    if promoted {
                        planes.prepare_guest_planes(|_| Ok(())).unwrap();
                    }
                    planes.store(width, &backgrounds, [0, 0, width as u32, 1], false, None);
                    let mut colors: Vec<i32> = (0..count)
                        .map(|index| {
                            (0xff00_0000 | (index as u32).wrapping_mul(0x0007_452b) & 0x00ff_ffff)
                                .cast_signed()
                        })
                        .collect();
                    if let Some(color) = position.and_then(|index| colors.get_mut(index)) {
                        *color = (color.cast_unsigned() & 0x00ff_ffff | alpha << 24).cast_signed();
                    }
                    let mut expected = backgrounds.clone();
                    for (offset, color) in colors.iter().enumerate() {
                        for phase in 0..4 {
                            let index = (phase / 2) * width * 2 + (offset + 2) * 2 + phase % 2;
                            expected[index] = crate::machine::pixel_ops::blend_argb(
                                *color,
                                expected[index].cast_signed(),
                            )
                            .cast_unsigned();
                        }
                    }
                    planes.composite_row(2, &colors).unwrap();
                    assert_eq!(
                        planes.interleave(width as u32, 1).unwrap(),
                        expected,
                        "count={count} promoted={promoted} alpha={alpha} position={position:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn opaque_sample_rows_replace_non_integer_destinations() {
    for operation in [SamplePlanes::composite_row, SamplePlanes::copy_row] {
        let mut planes = SamplePlanes::new(&[HeapValue::Int(0); 17])
            .unwrap()
            .unwrap();
        planes.prepare_guest_planes(|_| Ok(())).unwrap();
        for phase in 0..4 {
            planes
                .guest_plane_mut(phase)
                .unwrap()
                .fill(HeapValue::Float(1.0));
        }
        let colors: Vec<i32> = (0_u32..17)
            .map(|index| (0xff12_3400 | index).cast_signed())
            .collect();
        operation(&mut planes, 0, &colors).unwrap();
        for phase in 0..4 {
            assert!(
                planes
                    .guest_plane_mut(phase)
                    .unwrap()
                    .iter()
                    .zip(&colors)
                    .all(|(pixel, color)| matches!(pixel, HeapValue::Int(value) if value == color))
            );
        }
    }
}

#[test]
fn sample_row_bounds_errors_preserve_the_completed_plane_prefix() {
    for operation in [SamplePlanes::composite_row, SamplePlanes::copy_row] {
        for broken in 0..4 {
            let mut planes = SamplePlanes::new(&[HeapValue::Int(0); 17])
                .unwrap()
                .unwrap();
            planes.prepare_guest_planes(|_| Ok(())).unwrap();
            planes.guest_plane_mut(broken).unwrap().truncate(15);
            assert!(operation(&mut planes, 2, &[-1; 14]).is_err());
            for phase in 0..4 {
                let plane = planes.guest_plane_mut(phase).unwrap();
                for (index, pixel) in plane.iter().enumerate() {
                    let expected = if phase < broken && (2..16).contains(&index) {
                        -1
                    } else {
                        0
                    };
                    assert!(matches!(pixel,HeapValue::Int(value) if *value == expected));
                }
            }
        }
    }
}

#[test]
#[ignore = "manual four-plane row compositing throughput measurement"]
fn sample_row_throughput() {
    for count in [1, 16, 256, 1024] {
        for promoted in [false, true] {
            for kind in [0, 1, 2] {
                let initial: Vec<_> = (0..count)
                    .map(|index| HeapValue::Int((0x8040_2000_u32 | index as u32).cast_signed()))
                    .collect();
                let mut planes = SamplePlanes::new(&initial).unwrap().unwrap();
                if promoted {
                    planes.prepare_guest_planes(|_| Ok(())).unwrap();
                }
                let mut colors: Vec<i32> = (0..count)
                    .map(|index| {
                        ((if kind == 1 {
                            0x8000_0000_u32
                        } else {
                            0xff00_0000
                        }) | (index as u32).wrapping_mul(0x0007_452b) & 0x00ff_ffff)
                            .cast_signed()
                    })
                    .collect();
                if kind == 2 {
                    colors[count - 1] &= 0x00ff_ffff;
                }
                planes.composite_row(0, &colors).unwrap();
                let repeats = 1_048_576 / count;
                let started = std::time::Instant::now();
                for _ in 0..repeats {
                    std::hint::black_box(&mut planes)
                        .composite_row(0, std::hint::black_box(&colors))
                        .unwrap();
                }
                let elapsed = started.elapsed();
                let checksum = planes
                    .interleave(count as u32, 1)
                    .unwrap()
                    .iter()
                    .fold(0_u64, |sum, pixel| {
                        sum.wrapping_mul(31).wrapping_add(u64::from(*pixel))
                    });
                eprintln!(
                    "sample-row count={count} promoted={promoted} kind={kind} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}
