//! Executed against the prepared winit patch by test_winit_patches.py.

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
enum ControlFlow {
    Poll,
    Wait,
    WaitUntil(Instant),
}

struct Receiver(bool);
impl Receiver {
    fn has_incoming(&mut self) -> bool {
        self.0
    }
}

struct EventLoop {
    running: bool,
    pending_redraw: bool,
    user_events_receiver: Receiver,
    control_flow: ControlFlow,
}

// The unchanged winit helper treats None as an unbounded wait.
fn min_timeout(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    a.map_or(b, |a_timeout| {
        b.map_or(Some(a_timeout), |b_timeout| Some(a_timeout.min(b_timeout)))
    })
}

impl EventLoop {
    fn new(control_flow: ControlFlow) -> Self {
        Self {
            running: true,
            pending_redraw: false,
            user_events_receiver: Receiver(false),
            control_flow,
        }
    }
    fn control_flow(&self) -> ControlFlow {
        self.control_flow
    }
    fn timeout(&mut self, start: Instant, mut timeout: Option<Duration>) -> Option<Duration> {
        // PRODUCTION_TIMEOUT_SELECTION
        timeout
    }
}

#[test]
fn paused_activity_waits_even_with_stale_poll_or_expired_repaint() {
    let now = Instant::now();
    for control_flow in [
        ControlFlow::Poll,
        ControlFlow::Wait,
        ControlFlow::WaitUntil(now - Duration::from_secs(1)),
        ControlFlow::WaitUntil(now + Duration::from_secs(1)),
    ] {
        for redraw in [false, true] {
            for incoming in [false, true] {
                let mut event_loop = EventLoop::new(control_flow);
                event_loop.running = false;
                event_loop.pending_redraw = redraw;
                event_loop.user_events_receiver.0 = incoming;
                assert_eq!(event_loop.timeout(now, None), None, "{control_flow:?}");
                // External pump callers must retain their own blocking budget.
                for timeout in [Duration::ZERO, Duration::from_millis(7)] {
                    assert_eq!(event_loop.timeout(now, Some(timeout)), Some(timeout));
                }
            }
        }
    }
}

#[test]
fn resume_preserves_the_queued_redraw() {
    let now = Instant::now();
    let mut event_loop = EventLoop::new(ControlFlow::Wait);
    event_loop.pending_redraw = true;
    assert_eq!(event_loop.timeout(now, None), Some(Duration::ZERO));
    event_loop.running = false;
    assert_eq!(event_loop.timeout(now, None), None);
    assert!(event_loop.pending_redraw);
    event_loop.running = true;
    assert_eq!(event_loop.timeout(now, None), Some(Duration::ZERO));
}

#[test]
fn foreground_work_does_not_wait_for_a_timer() {
    let now = Instant::now();
    let mut event_loop = EventLoop::new(ControlFlow::WaitUntil(now + Duration::from_secs(1)));
    event_loop.user_events_receiver.0 = true;
    assert_eq!(event_loop.timeout(now, None), Some(Duration::ZERO));
}

#[test]
fn foreground_control_flow_and_pump_deadlines_are_preserved() {
    let now = Instant::now();
    let delay = Duration::from_millis(20);
    for (control_flow, expected) in [
        (ControlFlow::Poll, Some(Duration::ZERO)),
        (ControlFlow::Wait, None),
        (ControlFlow::WaitUntil(now - delay), Some(Duration::ZERO)),
        (ControlFlow::WaitUntil(now + delay), Some(delay)),
    ] {
        let mut event_loop = EventLoop::new(control_flow);
        assert_eq!(event_loop.timeout(now, None), expected);
        for pump in [
            Duration::ZERO,
            Duration::from_millis(7),
            Duration::from_secs(1),
        ] {
            assert_eq!(
                event_loop.timeout(now, Some(pump)),
                Some(expected.unwrap_or(pump).min(pump))
            );
        }
    }
}
