//! `MIDlet` lifecycle, AMS selection and permission boundary.

mod lifecycle;
mod native_api;
mod suite;

pub use lifecycle::{Ams, HostEvent, LifecycleAction, LifecycleState, lifecycle_call};
pub use native_api::register_natives;
pub use natives::{LifecycleCallback, LifecycleOutcome, MidletLifecycleEvent, MidletNotification};
pub use suite::{SuiteDescriptor, describe_suite, suite_midlets};
