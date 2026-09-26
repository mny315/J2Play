//! Shared VM preparation and guest host contracts, independent of any frontend.
#![allow(clippy::missing_errors_doc)]

use diagnostics::{Category, EmuError};

mod presentation;
mod suite;

pub use presentation::{LCDUI_SYSTEM_CHROME, compose_unscaled_lcdui_frame};
pub use suite::*;

mod dispatch;
pub use dispatch::*;

#[cfg(test)]
#[path = "../../../tests/unit/runtime/contracts.rs"]
mod tests;

mod failures;
pub use failures::{thread_failure_error, thread_failure_message};
