use super::*;

#[test]
fn cooperative_deadlines_round_up_without_overflow() {
    for (delay, expected) in [
        (Duration::ZERO, 0),
        (Duration::from_nanos(1), 1),
        (Duration::from_nanos(999_999), 1),
        (Duration::from_millis(1), 1),
        (Duration::from_nanos(1_000_001), 2),
        (Duration::MAX, u64::MAX),
    ] {
        assert_eq!(delay_millis(delay), expected);
    }
}

#[test]
fn frame_pacer_avoids_double_wait_and_paces_direct_presentations() {
    let start = Instant::now();
    let control = natives::FrameRateControl::new(50).unwrap();
    let mut pacer = FramePacer::new(control.clone());

    assert_eq!(pacer.request_delay(start, true), Duration::ZERO);
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(5), true, true),
        Duration::ZERO
    );
    assert_eq!(pacer.next_frame_at, Some(start + Duration::from_millis(20)));

    // A direct presentation, such as GameCanvas.flushGraphics, has no
    // repaint credit and therefore waits for the same frame deadline.
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(5), true, true),
        Duration::from_millis(15)
    );
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(100), true, true),
        Duration::ZERO
    );
    assert_eq!(pacer.next_frame_at, Some(start + Duration::from_millis(60)));
    for _ in 0..3 {
        assert_eq!(
            pacer.presentation_delay(start + Duration::from_millis(100), true, true),
            Duration::ZERO
        );
    }
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(100), true, true),
        Duration::from_millis(20)
    );

    control.set_manual_limit(Some(20)).unwrap();
    assert_eq!(
        pacer.request_delay(start + Duration::from_millis(105), true),
        Duration::ZERO
    );
    assert_eq!(
        pacer.next_frame_at,
        Some(start + Duration::from_millis(155))
    );
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(110), true, true),
        Duration::ZERO
    );
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_secs(10), false, true),
        Duration::ZERO
    );
    assert_eq!(pacer.next_frame_at, None);
    assert!(!pacer.presentation_pre_paced);
}

#[test]
fn unchanged_frames_do_not_reserve_deadlines_or_leave_repaint_credit() {
    let start = Instant::now();
    let control = natives::FrameRateControl::new(50).unwrap();
    let mut pacer = FramePacer::new(control.clone());
    assert_eq!(pacer.presentation_delay(start, true, true), Duration::ZERO);
    for millis in 1..20 {
        assert_eq!(
            pacer.presentation_delay(start + Duration::from_millis(millis), true, false),
            Duration::ZERO
        );
    }
    assert_eq!(pacer.next_frame_at, Some(start + Duration::from_millis(20)));
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(20), true, true),
        Duration::ZERO
    );
    assert_eq!(pacer.next_frame_at, Some(start + Duration::from_millis(40)));

    assert_eq!(
        pacer.request_delay(start + Duration::from_millis(30), true),
        Duration::from_millis(10)
    );
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(40), true, false),
        Duration::ZERO
    );
    assert!(!pacer.presentation_pre_paced);
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(40), true, true),
        Duration::from_millis(20)
    );

    control.set_manual_limit(Some(25)).unwrap();
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(50), true, false),
        Duration::ZERO
    );
    assert_eq!(pacer.next_frame_at, None);
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(50), true, true),
        Duration::ZERO
    );
    assert_eq!(pacer.next_frame_at, Some(start + Duration::from_millis(90)));
    assert_eq!(
        pacer.presentation_delay(start + Duration::from_millis(60), false, false),
        Duration::ZERO
    );
    assert_eq!(pacer.next_frame_at, None);
}

#[test]
fn frame_pacer_bounds_missed_deadline_catch_up_to_one_second() {
    let start = Instant::now();
    let control = natives::FrameRateControl::new(10).unwrap();
    let mut pacer = FramePacer::new(control);
    assert_eq!(pacer.presentation_delay(start, true, true), Duration::ZERO);

    let after_long_pause = start + Duration::from_secs(10);
    for _ in 0..10 {
        assert_eq!(
            pacer.presentation_delay(after_long_pause, true, true),
            Duration::ZERO
        );
    }
    assert_eq!(
        pacer.presentation_delay(after_long_pause, true, true),
        Duration::from_millis(100)
    );
}
