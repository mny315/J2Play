use crate::transport::{CommandQueue, EventMailbox, UrgentState};
use crate::{
    AttemptId, AudioMailbox, Frame, HostDecision, HostRequestKind, LatestFrameMailbox,
    LatestTelemetryMailbox, LibraryEntry, LibraryRepository, PauseReason, ProfileChoice,
    RuntimeTelemetry, SessionCommand, SessionCommandKind, SessionEvent, SessionEventKind,
    SessionId, VibrationEffect, VibrationMailbox,
};
use ::runtime::CanvasInputProfile;
use diagnostics::{Category, EmuError, bounded_text as bounded_detail};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const VM_HOST_STACK_BYTES: usize = 16 * 1024 * 1024;
pub const AUDIO_SAMPLE_RATE: u32 = mmapi::OUTPUT_SAMPLE_RATE;
const WORKER_IDLE_POLL: Duration = Duration::from_millis(100);
const GRACEFUL_SHUTDOWN_DEADLINE: Duration = Duration::from_secs(2);
const FORCED_SHUTDOWN_DEADLINE: Duration = Duration::from_secs(1);
const MAX_TEXT_COMMIT_BYTES: usize = 4 * 1024;
const MIDLET_CLASS: &str = "javax/microedition/midlet/MIDlet";

mod attempt;
mod checkpoint;
mod host;
mod requests;
mod worker;

use ::runtime::{
    apply_profile_limits, compose_unscaled_lcdui_frame, install_rust_bootstrap,
    install_suite_classes, profile_display_colors, profile_input_map, profile_lcd_ui_font_heights,
    register_profile_natives, suite_bundles_nokia_full_canvas_adapter,
    suite_requested_compatibility_jsrs,
};
use attempt::{recovery_candidates, run_attempt, update_text_input_state};
use host::{FrontendAudioSink, FrontendNativeContext};
use requests::RequestBroker;
use worker::runtime_error;
pub use worker::{PlatformLifecycleSignal, RuntimeWorker};

#[cfg(test)]
#[path = "../../../tests/unit/frontend-core/runtime/mod.rs"]
mod tests;
