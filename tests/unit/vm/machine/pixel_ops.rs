use super::*;

#[test]
fn shape_spans_match_individual_points_with_overlaps_and_transparency() {
    for alpha in [0, 1, 128, 254, 255] {
        let color = ((alpha << 24) | 0x12_3456_u32).cast_signed();
        let initial = vec![HeapValue::Int(0x8076_5432_u32.cast_signed()); 13 * 11];
        let mut expected = initial.clone();
        let mut actual = initial;
        {
            let mut reference =
                ShapePainter::new(&mut expected, 13, (2, 1, 11, 10), color).unwrap();
            let mut painter = ShapePainter::new(&mut actual, 13, (2, 1, 11, 10), color).unwrap();
            for (y, left, right) in [
                (2, 2, 10),
                (2, 5, 8),
                (3, 4, 4),
                (3, -100, 2),
                (1, 4, 7),
                (4, -100, 100),
                (0, -100, 100),
                (10, -100, 100),
                (1, 9, 8),
            ] {
                for x in left..=right {
                    reference.point(x, y).unwrap();
                }
                painter.span(y, left..=right, |_| Ok(())).unwrap();
            }
        }
        assert_eq!(actual, expected, "alpha={alpha}");
    }
}

#[test]
fn shape_spans_preserve_partial_writes_on_short_backing_and_cancellation() {
    for alpha in [128, 255] {
        let color = ((alpha << 24) | 0x12_3456_u32).cast_signed();
        let mut expected = vec![HeapValue::Int(-1); 4_096];
        let mut actual = expected.clone();
        let cancelled = |work: usize| {
            if work >= 1_024 {
                Err(vm_error("execution-cancelled", "test cancellation"))
            } else {
                Ok(())
            }
        };
        let mut reference =
            ShapePainter::new(&mut expected, 4_096, (0, 0, 4_096, 1), color).unwrap();
        let mut painter = ShapePainter::new(&mut actual, 4_096, (0, 0, 4_096, 1), color).unwrap();
        let expected_error = (0..4_096)
            .try_for_each(|x| {
                cancelled(x as usize)?;
                reference.point(x, 0)
            })
            .unwrap_err();
        assert_eq!(
            painter.span(0, 0..=4_095, cancelled).unwrap_err().code(),
            expected_error.code()
        );
        assert_eq!(painter.pixels, reference.pixels);
        for x in 0..4_096 {
            reference.point(x, 0).unwrap();
        }
        painter.span(0, 0..=4_095, |_| Ok(())).unwrap();
        assert_eq!(painter.pixels, reference.pixels);
    }

    let mut expected = vec![HeapValue::Int(0); 10];
    let mut actual = expected.clone();
    let mut reference = ShapePainter::new(&mut expected, 20, (0, 0, 20, 1), -1).unwrap();
    let mut painter = ShapePainter::new(&mut actual, 20, (0, 0, 20, 1), -1).unwrap();
    let expected_error = (0..20).try_for_each(|x| reference.point(x, 0)).unwrap_err();
    assert_eq!(
        painter.span(0, 0..=19, |_| Ok(())).unwrap_err().code(),
        expected_error.code()
    );
    assert_eq!(painter.pixels, reference.pixels);
}
