use super::*;

#[test]
fn delayed_acknowledgement_preserves_the_request_then_reports_compositor_exit_once() {
    let now = Instant::now();
    let mut state = Fullscreen::default();
    assert!(!state.poll(Some(false), now).unwrap().unwrap());
    state.request(true, now);
    for elapsed in [Duration::ZERO, TIMEOUT / 2] {
        assert!(state.poll(Some(false), now + elapsed).is_none());
        assert!(state.pending());
    }
    assert!(state.poll(Some(true), now + TIMEOUT).unwrap().unwrap());
    assert!(!state.pending());
    assert!(state.poll(Some(true), now + TIMEOUT).is_none());
    assert!(!state.poll(Some(false), now + TIMEOUT).unwrap().unwrap());
    assert!(state.poll(Some(false), now + TIMEOUT).is_none());
}

#[test]
fn rejected_and_superseded_requests_finish_without_reverting_a_newer_toggle() {
    let now = Instant::now();
    let mut state = Fullscreen::default();
    state.request(true, now);
    let error = state.poll(Some(false), now + TIMEOUT).unwrap().unwrap_err();
    assert_eq!(error.code(), "linux-fullscreen-timeout");
    assert!(!state.pending());
    assert!(!state.poll(Some(false), now + TIMEOUT).unwrap().unwrap());
    assert!(state.poll(Some(false), now + TIMEOUT).is_none());
    state.request(true, now);
    state.request(false, now);
    assert!(state.poll(Some(true), now).is_none());
    assert!(!state.poll(Some(false), now).unwrap().unwrap());
    state.request(true, now);
    assert!(state.poll(None, now).is_none());
    assert!(state.poll(None, now + TIMEOUT).unwrap().is_err());
    assert!(!state.pending());
}
