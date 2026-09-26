//! `MIDlet` notifications and callback results crossing the VM/host boundary.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidletNotification {
    Destroyed,
    Paused,
    ResumeRequested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleCallback {
    Start,
    Pause,
    Destroy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleOutcome {
    Completed,
    StateChangeRejected,
    RuntimeFailure,
}

/// Guest-originated lifecycle state, decoded once by the MIDP native binding.
/// The host's AMS validates callback ordering and applies the state transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidletLifecycleEvent {
    Notification(MidletNotification),
    Callback {
        callback: LifecycleCallback,
        outcome: LifecycleOutcome,
    },
}
