use super::*;

#[test]
fn frame_rate_control_supports_live_manual_override_and_clear() {
    let control = FrameRateControl::new(60).unwrap();
    assert_eq!(control.effective_limit(), 60);
    assert_eq!(control.manual_limit(), None);
    control.set_manual_limit(Some(20)).unwrap();
    assert_eq!(control.effective_limit(), 20);
    control.set_automatic_limit(30).unwrap();
    assert_eq!(control.effective_limit(), 20);
    control.set_manual_limit(None).unwrap();
    assert_eq!(control.effective_limit(), 30);
    assert_eq!(
        control.set_manual_limit(Some(0)).unwrap_err().code(),
        "invalid-max-fps"
    );
}
