//! Typed fixture hosts and process deadlines. This module is never a product dependency.

use diagnostics::{Category, EmuError};
use gcf::{HttpTransport, Resolver, TransportRequest, TransportResponse};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[path = "../../support/gcf_transport.rs"]
pub(crate) mod gcf_transport;
pub(crate) mod guest;
mod host;
mod isolation;
#[path = "../../support/process.rs"]
mod process;
#[path = "../../support/storage.rs"]
mod storage;
use host::FixtureHost;
pub use isolation::{isolate, process_phase};
use process::MAX_CAPTURE_BYTES;
pub(crate) use storage::Scratch;

const VM_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct Fixture {
    jar: PathBuf,
    profile: OsString,
    jad: Option<PathBuf>,
    midlet: Option<u32>,
    rms_root: Option<PathBuf>,
    file_root: Option<PathBuf>,
    permissions: HashSet<String>,
    unknown_permissions: HashSet<String>,
    allow_platform: bool,
    host_events: Vec<midp::HostEvent>,
    wall_clock: i64,
    instructions: u64,
    trace: bool,
}

impl Fixture {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            jar: path.as_ref().to_owned(),
            profile: OsString::from("se-featurephone"),
            jad: None,
            midlet: None,
            rms_root: None,
            file_root: None,
            permissions: HashSet::new(),
            unknown_permissions: HashSet::new(),
            allow_platform: false,
            host_events: vec![
                midp::HostEvent::Launch,
                midp::HostEvent::Pause,
                midp::HostEvent::Resume,
                midp::HostEvent::Close,
            ],
            wall_clock: 1_650_000_000_000,
            instructions: vm::Limits::default().max_instructions,
            trace: false,
        }
    }

    pub fn profile(mut self, value: impl AsRef<OsStr>) -> Self {
        value.as_ref().clone_into(&mut self.profile);
        self
    }
    pub fn jad(mut self, value: impl AsRef<Path>) -> Self {
        self.jad = Some(value.as_ref().to_owned());
        self
    }
    pub const fn midlet(mut self, value: u32) -> Self {
        self.midlet = Some(value);
        self
    }
    pub fn rms_root(mut self, value: impl AsRef<Path>) -> Self {
        self.rms_root = Some(value.as_ref().to_owned());
        self
    }
    pub fn file_root(mut self, value: impl AsRef<Path>) -> Self {
        self.file_root = Some(value.as_ref().to_owned());
        self
    }
    pub fn permissions(mut self, values: &[&str]) -> Self {
        self.permissions = values.iter().map(|value| (*value).to_owned()).collect();
        self
    }
    pub fn unknown_permissions(mut self, values: &[&str]) -> Self {
        self.unknown_permissions = values.iter().map(|value| (*value).to_owned()).collect();
        self
    }
    pub const fn allow_platform(mut self, value: bool) -> Self {
        self.allow_platform = value;
        self
    }
    pub fn host_events(mut self, events: &[midp::HostEvent]) -> Self {
        self.host_events = events.to_vec();
        self
    }
    pub const fn wall_clock(mut self, value: i64) -> Self {
        self.wall_clock = value;
        self
    }
    pub fn instruction_limit(mut self, value: u64) -> Self {
        assert!((1..=100_000_000).contains(&value));
        self.instructions = value;
        self
    }
    pub const fn trace(mut self) -> Self {
        self.trace = true;
        self
    }

    pub fn run_static(self, class: &str, method: &str, descriptor: &str) -> Probe {
        self.run(Some((
            class.to_owned(),
            method.to_owned(),
            descriptor.to_owned(),
        )))
    }

    pub fn run_midlet(self) -> Probe {
        self.run(None)
    }

    fn run(self, entry: Option<(String, String, String)>) -> Probe {
        assert!(
            isolation::is_current_test_child(),
            "guest fixtures must run inside isolate() or process_phase()"
        );
        // A joined VM worker has the same stack contract as the product. The
        // containing test runs under isolate(), whose parent kills AND reaps a
        // stuck child if a VM cancellation regression prevents this join.
        std::thread::Builder::new()
            .name("conformance-vm".to_owned())
            .stack_size(frontend_core::VM_HOST_STACK_BYTES)
            .spawn(move || self.execute(entry))
            .unwrap()
            .join()
            .unwrap()
    }

    #[allow(clippy::too_many_lines)]
    fn execute(self, entry: Option<(String, String, String)>) -> Probe {
        let mut probe = Probe::default();
        let result = (|| {
            let scratch = Scratch::new();
            let bytes = read_bounded(&self.jar, 64 * 1024 * 1024);
            let info = jar::inspect_bytes(&bytes)?;
            let jad = self
                .jad
                .as_ref()
                .map(|path| jar::parse_jad(&read_bounded(path, 1024 * 1024)))
                .transpose()?;
            let suite = if entry.is_none() {
                Some(midp::describe_suite(&info, jad.as_ref(), self.midlet)?)
            } else {
                None
            };
            let properties = suite.as_ref().map_or_else(
                || {
                    info.manifest
                        .iter()
                        .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
                        .collect()
                },
                |suite| suite.properties.clone(),
            );
            let profile = launch::builtin_device_profile(&self.profile)?.map_or_else(
                || device_profile::DeviceProfile::from_path(&self.profile),
                Ok,
            )?;
            let resolved = profile.resolve(device_profile::ProfileOverrides::default())?;
            let mut limits = vm::Limits {
                max_instructions: self.instructions,
                ..vm::Limits::default()
            };
            runtime::apply_profile_limits(&mut limits, &resolved)?;
            let mut program = vm::Program::new();
            let deadline = Instant::now() + VM_DEADLINE;
            let cancelled = || Instant::now() >= deadline;
            let classes = jar::read_class_entries_bytes(&bytes)?;
            let requested =
                runtime::suite_requested_compatibility_jsrs(&classes, &profile, cancelled)?
                    .ok_or_else(cancelled_error)?;
            let replace = runtime::suite_bundles_nokia_full_canvas_adapter(&classes)?;
            runtime::install_rust_bootstrap(&mut program, &limits, &resolved, &requested, replace)?;
            if !runtime::install_suite_classes(&mut program, &limits, classes, cancelled)? {
                return Err(cancelled_error());
            }
            runtime::register_profile_natives(&mut program, &profile)?;
            let mut host = FixtureHost::new(
                &self,
                &scratch.0,
                &profile,
                bytes,
                properties,
                &info.sha256,
                deadline,
            )?;
            let execution = if let Some((class, method, descriptor)) = entry {
                program.execute_with_context(
                    &class,
                    &method,
                    &descriptor,
                    limits,
                    self.trace,
                    &mut host,
                )
            } else {
                let suite = suite.unwrap();
                let class = suite.midlet.class_name.replace('.', "/");
                probe.midlet = Some(suite.midlet);
                if !program.is_assignable_to(&class, "javax/microedition/midlet/MIDlet") {
                    return Err(EmuError::new(
                        Category::Api,
                        "midlet-class",
                        "selected class does not extend MIDlet",
                    ));
                }
                if !program.is_publicly_instantiable(&class) {
                    return Err(EmuError::new(
                        Category::Api,
                        "midlet-constructor",
                        "MIDlet requires a public no-argument constructor",
                    ));
                }
                let ams = host.ams.clone();
                let mut pending_idle = false;
                program.execute_instance_step_driver_with_context(
                    &class,
                    |turn| {
                        if cancelled() {
                            return Err(cancelled_error());
                        }
                        if pending_idle {
                            pending_idle = false;
                            return Ok(vm::DriverStep::Call(runtime::idle_call()));
                        }
                        let action = ams.borrow_mut().next_action();
                        if let Some(action) = action {
                            if probe.callbacks.len() >= 1024 {
                                return Err(cancelled_error());
                            }
                            probe.callbacks.push(action);
                            pending_idle = matches!(
                                action,
                                midp::LifecycleAction::Start | midp::LifecycleAction::Resume
                            );
                            let call = midp::lifecycle_call(action);
                            return Ok(
                                if matches!(action, midp::LifecycleAction::Destroy { .. }) {
                                    vm::DriverStep::CallAfterApplicationShutdown(call)
                                } else {
                                    vm::DriverStep::Call(call)
                                },
                            );
                        }
                        if ams.borrow().state() == midp::LifecycleState::Destroyed {
                            return Ok(vm::DriverStep::Stop);
                        }
                        Ok(if turn == vm::DriverTurn::HostPoll {
                            vm::DriverStep::Tick
                        } else {
                            vm::DriverStep::Call(runtime::idle_call())
                        })
                    },
                    limits,
                    self.trace,
                    &mut host,
                )
            };
            probe.diagnostics = std::mem::take(&mut host.console);
            probe.flight_events = std::mem::take(&mut host.flight_events);
            probe.frame = host.frame.take();
            probe.frames = host.frames;
            probe.audio_frames = host.mmapi_runtime.sink().frames_written();
            probe.lifecycle = std::mem::take(&mut host.lifecycle);
            probe.platform_requests = host.platform_requests;
            probe.ams_state = Some(host.ams.borrow().state());
            execution
        })();
        match result {
            Ok(execution) => {
                for line in &execution.trace {
                    push_capture(&mut probe.diagnostics, line);
                    push_capture(&mut probe.diagnostics, "\n");
                }
                if let Some(error) = runtime::thread_failure_error(
                    &execution.thread_failures,
                    execution.thread_failure_count,
                ) {
                    push_capture(&mut probe.diagnostics, &format!("{error}\n"));
                    if probe.midlet.is_some() {
                        probe.error = Some(error);
                    }
                }
                probe.execution = Some(execution);
            }
            Err(error) => {
                push_capture(&mut probe.diagnostics, &format!("{error}\n"));
                probe.error = Some(error);
            }
        }
        probe
    }
}

#[derive(Default)]
pub struct Probe {
    pub execution: Option<vm::Execution>,
    pub error: Option<EmuError>,
    pub diagnostics: String,
    pub flight_events: Vec<String>,
    pub frame: Option<(u32, u32, Vec<u32>)>,
    pub frames: u64,
    pub audio_frames: u64,
    pub ams_state: Option<midp::LifecycleState>,
    pub callbacks: Vec<midp::LifecycleAction>,
    pub lifecycle: Vec<natives::MidletNotification>,
    pub platform_requests: usize,
    pub midlet: Option<jar::MidletInfo>,
}

impl Probe {
    pub fn int_value(&self) -> Option<i32> {
        match self.execution.as_ref()?.value {
            Some(vm::Value::Int(value)) => Some(value),
            _ => None,
        }
    }
    pub fn success(&self) -> bool {
        self.error.is_none()
            && self
                .execution
                .as_ref()
                .is_some_and(|e| self.midlet.is_none() || e.thread_failure_count == 0)
    }
}

fn cancelled_error() -> EmuError {
    EmuError::new(
        Category::Vm,
        "execution-cancelled",
        "fixture execution deadline exceeded",
    )
}

fn push_capture(target: &mut String, value: &str) {
    let remaining = MAX_CAPTURE_BYTES.saturating_sub(target.len());
    let mut end = value.len().min(remaining);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    target.push_str(&value[..end]);
}

fn read_bounded(path: &Path, maximum: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= maximum, "oversized fixture input");
    bytes
}
