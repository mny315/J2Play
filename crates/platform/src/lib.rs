//! Portable input, frame geometry, pacing and telemetry contracts.

mod frame_pacing;
mod input;
mod telemetry;

pub use frame_pacing::*;
pub use input::*;
pub use telemetry::VmDebugStats;

/// Rectangle in unscaled logical-frame coordinates.
///
/// Identifies guest regions inside the framebuffer independently of host scale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
