use super::*;

#[test]
fn clock_roll_wraps_only_the_selected_field_and_handles_large_amounts() {
    let base = compose_millis(2024, 12, 31, 23, 59, 59, 999).unwrap();
    for (field, clock) in [
        (9, (11, 59, 59, 999)),
        (10, (12, 59, 59, 999)),
        (11, (0, 59, 59, 999)),
        (12, (23, 0, 59, 999)),
        (13, (23, 59, 0, 999)),
        (14, (23, 59, 59, 0)),
    ] {
        assert_eq!(
            calendar_roll_field(base, field, 1).unwrap(),
            compose_millis(2024, 12, 31, clock.0, clock.1, clock.2, clock.3).unwrap(),
            "field {field}"
        );
        for amount in [-i32::MAX, -1000, -1, 0, 1, 1000, i32::MAX] {
            let rolled = calendar_roll_field(base, field, amount).unwrap();
            assert_eq!(
                calendar_roll_field(rolled, field, -amount).unwrap(),
                base,
                "field {field}, amount {amount}"
            );
        }
        let rolled = calendar_roll_field(base, field, i32::MIN).unwrap();
        assert_eq!(
            calendar_roll_field(rolled, field, i32::MAX).unwrap(),
            calendar_roll_field(base, field, -1).unwrap()
        );
    }
}

#[test]
fn month_roll_preserves_the_year_and_clamps_to_the_target_month() {
    for (year, last_day) in [(2023, 28), (2024, 29), (0, 29), (-1, 28)] {
        let january = compose_millis(year, 1, 31, 12, 34, 56, 789).unwrap();
        assert_eq!(
            calendar_roll_field(january, 2, 1).unwrap(),
            compose_millis(year, 2, last_day, 12, 34, 56, 789).unwrap()
        );
        assert_eq!(
            calendar_roll_field(january, 2, -1).unwrap(),
            compose_millis(year, 12, 31, 12, 34, 56, 789).unwrap()
        );
    }
}

#[test]
fn civil_dates_round_trip_across_positive_and_negative_era_boundaries() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    for year in [
        -801, -800, -799, -401, -400, -399, -1, 0, 1, 399, 400, 401, 2000,
    ] {
        for month in 1..=12 {
            for day in [1, days_in_month(year, month)] {
                assert_eq!(
                    civil_from_days(days_from_civil(year, month, day)),
                    (year, month, day)
                );
            }
        }
    }
}

#[test]
fn calendar_extreme_values_return_controlled_results() {
    let year_zero = compose_millis(0, 1, 1, 0, 0, 0, 0).unwrap();
    assert_eq!(
        calendar_replace_date_time(year_zero, i32::MIN, 0, 1, 0, 0, 0)
            .unwrap_err()
            .code(),
        "illegal-argument"
    );
    assert_eq!(
        calendar_add_field(year_zero, 1, i32::MIN)
            .unwrap_err()
            .code(),
        "calendar-overflow"
    );
    for millis in [i64::MIN, year_zero, -1, 0, i64::MAX] {
        for field in 0..=15 {
            let _ = calendar_field(millis, field);
            for value in [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX] {
                let _ = calendar_replace_field(millis, field, value);
                let _ = calendar_add_field(millis, field, value);
                let _ = calendar_roll_field(millis, field, value);
            }
        }
    }
}

#[test]
fn calendar_replace_supports_the_complete_field_surface() {
    let base = compose_millis(2024, 3, 14, 12, 30, 0, 0).unwrap();
    let cases = [(0, 0), (3, 10), (4, 2), (6, 60), (7, 1), (8, 1)];
    for (field, value) in cases {
        let replaced = calendar_replace_field(base, field, value).unwrap();
        assert_eq!(calendar_field(replaced, field).unwrap(), value);
    }
    assert!(calendar_replace_field(base, 6, 367).is_err());
    assert!(calendar_replace_field(base, 8, 6).is_err());
    let bc = calendar_replace_field(base, 0, 0).unwrap();
    let bc_year = calendar_replace_field(bc, 1, 7).unwrap();
    assert_eq!(calendar_field(bc_year, 0).unwrap(), 0);
    assert_eq!(calendar_field(bc_year, 1).unwrap(), 7);
    let leap = calendar_replace_date_time(base, 2024, 1, 29, 13, 14, 15).unwrap();
    assert_eq!(calendar_field(leap, 2).unwrap(), 1);
    assert_eq!(calendar_field(leap, 5).unwrap(), 29);
    assert!(calendar_replace_date_time(base, 2023, 1, 29, 0, 0, 0).is_err());
    let bc_date = calendar_replace_date_time(bc, 7, 0, 2, 3, 4, 5).unwrap();
    assert_eq!(calendar_field(bc_date, 0).unwrap(), 0);
    assert_eq!(calendar_field(bc_date, 1).unwrap(), 7);
}

#[test]
fn calendar_add_treats_week_fields_as_seven_days() {
    const DAY_MILLIS: i64 = 86_400_000;
    let base = compose_millis(2024, 3, 14, 12, 30, 45, 678).unwrap();
    for field in [4, 8] {
        let added = calendar_add_field(base, field, 1).unwrap();
        assert_eq!(added - base, DAY_MILLIS * 7, "field {field}");
        assert_eq!(
            civil_from_days(added.div_euclid(DAY_MILLIS)),
            (2024, 3, 21),
            "field {field}"
        );
    }
}

#[test]
fn calendar_add_distinguishes_half_day_and_hour_fields() {
    let base = compose_millis(2024, 3, 14, 5, 30, 0, 0).unwrap();

    assert_eq!(
        calendar_field(calendar_add_field(base, 9, 1).unwrap(), 11).unwrap(),
        17
    );
    for field in [10, 11] {
        assert_eq!(
            calendar_field(calendar_add_field(base, field, 1).unwrap(), 11).unwrap(),
            6
        );
    }
}

#[test]
fn calendar_roll_wraps_week_and_day_fields_without_larger_carry() {
    const DAY_MILLIS: i64 = 86_400_000;
    let cases = [
        (3, (2024, 12, 31), (2024, 1, 2)),
        (4, (2024, 3, 31), (2024, 3, 3)),
        (6, (2024, 12, 31), (2024, 1, 1)),
        (7, (2024, 3, 16), (2024, 3, 10)),
        (8, (2024, 3, 31), (2024, 3, 3)),
    ];
    for (field, source, expected) in cases {
        let base = compose_millis(source.0, source.1, source.2, 23, 59, 58, 321).unwrap();
        let rolled = calendar_roll_field(base, field, 1).unwrap();
        assert_eq!(
            civil_from_days(rolled.div_euclid(DAY_MILLIS)),
            expected,
            "field {field}"
        );
        assert_eq!(
            rolled.rem_euclid(DAY_MILLIS),
            base.rem_euclid(DAY_MILLIS),
            "field {field}"
        );
        assert_eq!(calendar_field(rolled, field).unwrap(), 1, "field {field}");
        assert_eq!(
            calendar_roll_field(rolled, field, -1).unwrap(),
            base,
            "field {field}"
        );
    }
}
