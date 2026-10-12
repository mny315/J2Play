//! Host driver commands and execution results.

use super::Value;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstanceCall {
    pub target: CallTarget,
    pub name: String,
    pub descriptor: String,
    pub arguments: Vec<Value>,
}

/// Why the VM is asking the host driver for its next step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriverTurn {
    /// No guest callback is currently waiting for a frontend poll.
    Regular,
    /// A guest callback yielded after a completed frame or real-time sleep.
    /// The host may poll and dispatch external events, but should not start
    /// routine guest maintenance which can wait for another frame itself.
    HostPoll,
}

/// One host-driver step between guest callbacks. `Tick` advances a runnable
/// Java worker thread without manufacturing a synthetic Java method call.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DriverStep {
    /// Captures semantic VM state at a cooperative boundary together with
    /// the host driver's opaque state. The host owns atomic persistence.
    SaveCheckpoint {
        driver_state: Vec<u8>,
    },
    Call(InstanceCall),
    /// Stops existing application workers and timers before invoking a
    /// host-selected callback, then omits the normal post-callback worker
    /// turn. Lifecycle managers use this for `destroyApp`, so teardown cannot
    /// release guest resources concurrently with an old application thread.
    CallAfterApplicationShutdown(InstanceCall),
    Tick,
    /// Returns control to the host boundary without advancing any guest state.
    /// If a long-running driver callback yielded for a host poll, it remains
    /// suspended until a later non-idle step.
    Idle,
    Stop,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum CallTarget {
    Instance,
    Static { class: String },
}

#[derive(Debug)]
pub struct Execution {
    pub value: Option<Value>,
    pub instructions: u64,
    /// Deterministic VM monotonic time elapsed during the execution.
    pub virtual_millis: i64,
    /// Live heap size at shutdown.
    pub heap_bytes: usize,
    /// Live heap object count at shutdown.
    pub heap_objects: usize,
    /// Maximum live heap size observed by the allocator.
    pub peak_heap_bytes: usize,
    /// Maximum live heap object count observed by the allocator.
    pub peak_heap_objects: usize,
    /// Suite-scoped native M3G counters at shutdown.
    pub m3g: M3gExecutionMetrics,
    /// Suite-scoped native `MascotCapsule Micro3D` counters at shutdown.
    pub micro3d: micro3d::Micro3dMetrics,
    /// Bounded, sampled `Micro3D` projection state for structured host logs.
    pub micro3d_diagnostics: Vec<String>,
    /// Aggregated Java exceptions handled by guest `catch` blocks.
    pub caught_exception_diagnostics: Vec<String>,
    pub trace: Vec<String>,
    /// Bounded recent details for application threads whose exception escaped
    /// `Thread.run`. The VM still isolates each failure to that Java thread.
    pub thread_failures: Vec<ThreadFailure>,
    /// Total uncaught worker failures, including entries evicted from the
    /// bounded retained detail list.
    pub thread_failure_count: u64,
}

/// Native M3G resource and work counters included in run metrics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct M3gExecutionMetrics {
    pub live_bytes: usize,
    pub peak_bytes: usize,
    pub live_objects: usize,
    pub peak_objects: usize,
    pub loaded_files: u64,
    pub loaded_sections: u64,
    pub loaded_objects: u64,
    pub decompressed_bytes: u64,
    pub scene_nodes: u64,
    pub active_lights: usize,
    pub texture_binds: u64,
    pub animation_samples: u64,
    pub render_calls: u64,
    pub render_time_nanos: u64,
    pub max_render_time_nanos: u64,
    pub submitted_triangles: u64,
    pub clipped_triangles: u64,
    pub culled_triangles: u64,
    pub rasterized_triangles: u64,
    pub shaded_pixels: u64,
    pub depth_rejected_pixels: u64,
    pub blended_pixels: u64,
}

/// A Java worker-thread failure retained independently of optional VM tracing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadFailure {
    pub thread_id: u64,
    pub exception_class: String,
    pub exception_message: Option<String>,
    pub stack_trace: Vec<String>,
    /// True only when the VM recorded a managed heap allocation failure.
    pub managed_heap_limit: bool,
}

impl ThreadFailure {
    /// Returns whether this failure reports the bounded managed-heap limit.
    #[must_use]
    pub const fn is_managed_heap_limit(&self) -> bool {
        self.managed_heap_limit
    }
}
