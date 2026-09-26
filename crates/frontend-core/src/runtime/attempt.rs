//! Launch preparation and the state shared by attempt input, lifecycle and checkpoints.

use super::checkpoint::{ContextCheckpoints, LaunchCheckpoint, choose_checkpoint, system_millis};
use super::{
    Arc, AttemptId, AudioMailbox, CanvasInputProfile, Category, Cell, CommandQueue, Duration,
    EmuError, EventMailbox, FrontendAudioSink, FrontendNativeContext, Instant, LatestFrameMailbox,
    LatestTelemetryMailbox, LibraryEntry, LibraryRepository, MIDLET_CLASS, PauseReason,
    PlatformLifecycleSignal, ProfileChoice, Rc, RefCell, RequestBroker, SessionCommand,
    SessionCommandKind, SessionEvent, SessionEventKind, SessionId, UrgentState, VecDeque,
    VibrationMailbox, WORKER_IDLE_POLL, apply_profile_limits, install_rust_bootstrap,
    install_suite_classes, profile_display_colors, profile_input_map, profile_lcd_ui_font_heights,
    register_profile_natives, runtime_error, suite_bundles_nokia_full_canvas_adapter,
    suite_requested_compatibility_jsrs,
};
use crate::library::checkpoint::CheckpointIdentity;
use sha2::{Digest, Sha256};

mod checkpoint;
mod driver;
mod input;
pub(super) use input::retain_input_release_steps;

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-core/runtime/attempt/mod.rs"]
mod tests;

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
pub(super) fn run_attempt(
    repository: &LibraryRepository,
    entry_id: &str,
    orientation: Option<launch::CanvasOrientation>,
    session_id: SessionId,
    attempt_id: AttemptId,
    commands: CommandQueue,
    events: EventMailbox,
    frames: LatestFrameMailbox,
    telemetry: LatestTelemetryMailbox,
    audio: AudioMailbox,
    vibration: VibrationMailbox,
    lifecycle: PlatformLifecycleSignal,
) -> Result<(), EmuError> {
    let urgent = Arc::clone(commands.urgent());
    if attempt_stop_requested(&urgent, attempt_id) {
        return Ok(());
    }
    let entry = repository.load_entry(entry_id)?;
    let app_settings = repository.load_app_settings().unwrap_or_else(|error| {
        eprintln!("j2play: app settings unavailable; using defaults: {error}");
        repository.default_app_settings()
    });
    let mut effective_settings = entry.settings().clone();
    app_settings.apply_control_defaults(&mut effective_settings);
    vibration.configure(session_id, attempt_id, effective_settings.vibration)?;
    if attempt_stop_requested(&urgent, attempt_id) {
        return Ok(());
    }
    let prepared = repository
        .prepare_launch_cancellable(&entry, || attempt_stop_requested(&urgent, attempt_id));
    if attempt_stop_requested(&urgent, attempt_id) {
        return Ok(());
    }
    let prepared = prepared?;
    let plan = prepared.launch_plan_with_orientation(entry.settings(), orientation)?;
    let profiles = launch::builtin_device_profiles()?;
    let resolved = plan
        .decision
        .resolve_profile(profiles, device_profile::ProfileOverrides::default())?;
    let persona = resolved.persona();
    let runtime_host = resolved.runtime_host();
    let (display_width, display_height) = resolved.canvas_dimensions();
    let (normal_canvas_width, normal_canvas_height) = resolved.non_fullscreen_canvas_dimensions();
    let class = prepared.midlet().class_name.replace('.', "/");
    let checkpoint_identity = CheckpointIdentity {
        jar_sha256: plan.jar_sha256.clone(),
        midlet_class: class.clone(),
        profile_catalog: plan.profile_catalog.clone(),
        persona: persona.profile_id().to_owned(),
        runtime_host: runtime_host.profile_id().to_owned(),
        canvas: (display_width, display_height),
        properties: Sha256::digest(save_state::encode(&prepared.suite().properties)?).into(),
    };
    let automatic_fps_limit = plan.automatic_fps_limit;
    events.publish(SessionEvent {
        session_id,
        attempt_id,
        kind: SessionEventKind::LaunchPrepared(Box::new(plan)),
    })?;
    let mut request_broker = RequestBroker::new(
        session_id,
        attempt_id,
        commands.clone(),
        events.clone(),
        lifecycle.clone(),
    );
    let saved = match choose_checkpoint(
        repository,
        &entry,
        &checkpoint_identity,
        &mut request_broker,
    )? {
        LaunchCheckpoint::Fresh => None,
        LaunchCheckpoint::Resume(saved) => Some(saved),
        LaunchCheckpoint::Cancel => return Ok(()),
    };
    if attempt_stop_requested(&urgent, attempt_id) {
        return Ok(());
    }
    let mut limits = vm::Limits {
        max_instructions: vm::INTERACTIVE_INSTRUCTION_LIMIT,
        ..vm::Limits::default()
    };
    apply_profile_limits(&mut limits, &resolved)?;
    let class_resources = jar::read_class_entries_bytes(prepared.jar_bytes())?;
    if attempt_stop_requested(&urgent, attempt_id) {
        return Ok(());
    }
    let Some(requested_compatibility_jsrs) =
        suite_requested_compatibility_jsrs(&class_resources, persona, || {
            attempt_stop_requested(&urgent, attempt_id)
        })?
    else {
        return Ok(());
    };
    let replace_bundled_nokia_full_canvas =
        suite_bundles_nokia_full_canvas_adapter(&class_resources)?;
    if attempt_stop_requested(&urgent, attempt_id) {
        return Ok(());
    }
    let mut program = vm::Program::new();
    install_rust_bootstrap(
        &mut program,
        &limits,
        &resolved,
        &requested_compatibility_jsrs,
        replace_bundled_nokia_full_canvas,
    )?;
    if !install_suite_classes(&mut program, &limits, class_resources, || {
        attempt_stop_requested(&urgent, attempt_id)
    })? {
        return Ok(());
    }
    #[cfg(feature = "aot")]
    {
        let mut numeric_compiler =
            native_code::Compiler::with_cache(repository.native_code_cache_root());
        program.prepare_integer_methods(&mut numeric_compiler, &|| {
            attempt_stop_requested(&urgent, attempt_id)
        });
        eprintln!(
            "J2Play AOT preparation: {:?}",
            numeric_compiler.preparation_stats()
        );
    }
    if attempt_stop_requested(&urgent, attempt_id) {
        return Ok(());
    }
    if !program.is_assignable_to(&class, MIDLET_CLASS) {
        return Err(EmuError::new(
            Category::Api,
            "midlet-class",
            format!("selected class {class} does not extend {MIDLET_CLASS}"),
        ));
    }
    if !program.is_publicly_instantiable(&class) {
        return Err(EmuError::new(
            Category::Api,
            "midlet-constructor",
            format!(
                "selected MIDlet class {class} must be public, concrete, and have a public no-argument constructor"
            ),
        ));
    }
    register_profile_natives(&mut program, persona)?;

    let frame_rate = natives::FrameRateControl::new(automatic_fps_limit)?;
    frame_rate
        .set_manual_limit(app_settings.effective_manual_fps_limit(entry.settings().fps_limit))?;
    let realtime_pacing = Rc::new(Cell::new(true));
    let ams = Rc::new(RefCell::new(midp::Ams::new([midp::HostEvent::Launch])?));
    let active_canvas_dimensions = Rc::new(Cell::new((normal_canvas_width, normal_canvas_height)));
    let pointer = platform::PointerInputPolicy::new(
        resolved.pointer_events(),
        resolved.pointer_motion_events(),
    );
    let gcf_limits = gcf::Limits {
        connect_timeout: Duration::from_secs(1),
        read_timeout: Duration::from_secs(1),
        ..gcf::Limits::default()
    };
    let connector_permissions = connector_permissions(persona);
    let storage = match &saved {
        Some(saved) => repository.prepare_restored_storage(&entry, &saved.files)?,
        None => repository.runtime_storage(&entry)?,
    };
    let epoch = if saved.is_some() {
        repository.checkpoint_epoch(&entry)?
    } else {
        repository.begin_checkpoint_session(&entry)?
    };
    let suite_id = rms_suite_id(&entry, prepared.suite())?;
    let rms_runtime = match &saved {
        Some(saved) => rms::Runtime::restore_checkpoint(
            &storage.rms,
            suite_id,
            rms::Limits::default(),
            &saved.rms,
        )?,
        None => rms::Runtime::new(&storage.rms, suite_id, rms::Limits::default()),
    };
    let suite_properties = prepared.suite().properties.clone();
    let gcf_runtime = gcf::Runtime::new(
        connector_permissions,
        Box::<gcf::UreqTransport>::default(),
        Box::<gcf::SystemResolver>::default(),
        &storage.files,
        gcf_limits,
        false,
    )?;
    let jar_resources = jar::ResourceArchive::from_owned_bytes(prepared.into_jar_bytes())?;
    let allow_heap_recovery = matches!(entry.settings().device_profile, ProfileChoice::Automatic);
    let decode_urgent = Arc::clone(&urgent);
    let mut context = FrontendNativeContext {
        session_id,
        attempt_id,
        started: Instant::now(),
        display_width,
        display_height,
        normal_canvas_width,
        normal_canvas_height,
        active_canvas_dimensions: Rc::clone(&active_canvas_dimensions),
        pointer,
        canvas_input: CanvasInputProfile::from_profile(persona),
        display_colors: profile_display_colors(persona)?,
        lcd_ui_font_heights: profile_lcd_ui_font_heights(persona),
        properties: cldc::SystemProperties::from_profile(persona),
        bluetooth_properties: bluetooth::Properties::from_profile(persona),
        jar_resources,
        suite_properties,
        ams: Rc::clone(&ams),
        events,
        frames,
        telemetry,
        commands,
        urgent: Arc::clone(&urgent),
        lifecycle: lifecycle.clone(),
        composed_frame: Vec::new(),
        frame_pacer: platform::FramePacer::new(frame_rate),
        interpreter_instructions_per_second: runtime_host
            .runtime()
            .interpreter_instructions_per_second(),
        rms_runtime,
        mmapi_runtime: mmapi::Runtime::new(
            FrontendAudioSink {
                session_id,
                attempt_id,
                mailbox: audio.clone(),
            },
            mmapi::Limits::default(),
        )
        .with_decode_cancellation(move || {
            decode_urgent.force_cancelled() || decode_urgent.stop_cancellation_due(attempt_id)
        }),
        gcf_runtime,
        vibration: vibration.clone(),
        request_broker,
        allow_heap_recovery,
        realtime_pacing: Rc::clone(&realtime_pacing),
        text_input_active: false,
        checkpoints: ContextCheckpoints {
            repository: repository.clone(),
            entry: entry.clone(),
            identity: checkpoint_identity,
            epoch,
            storage,
            host_close: Rc::default(),
            restored: saved.is_some(),
            monotonic_base: saved.as_ref().map_or(0, |saved| saved.monotonic_millis),
            wall_offset: saved.as_ref().map_or(0, |saved| {
                saved.wall_clock_millis.saturating_sub(system_millis())
            }),
            last_frame: saved.as_ref().and_then(|saved| saved.frame.clone()),
        },
    };
    if let Some(saved) = &saved {
        if saved.monotonic_millis < 0
            || ![
                (display_width, display_height),
                (normal_canvas_width, normal_canvas_height),
            ]
            .contains(&saved.active_canvas)
        {
            return Err(runtime_error(
                "checkpoint-canvas",
                "The automatic save has incompatible Canvas state",
            ));
        }
        if let Some(frame) = &saved.frame {
            let [x, y, width, height] = frame.canvas_region;
            crate::Frame::with_canvas_region(
                session_id,
                attempt_id,
                display_width,
                display_height,
                platform::LogicalRect {
                    x,
                    y,
                    width,
                    height,
                },
                Arc::clone(&frame.pixels),
            )?;
        }
        let restored_ams: midp::Ams = save_state::decode(&saved.ams)?;
        restored_ams.validate_checkpoint()?;
        *ams.borrow_mut() = restored_ams;
        active_canvas_dimensions.set(saved.active_canvas);
        context.mmapi_runtime.restore_checkpoint(&saved.mmapi)?;
    }
    let input_map = profile_input_map(persona);
    let mut driver = AttemptDriver::new(
        session_id,
        attempt_id,
        context.commands.clone(),
        Rc::clone(&ams),
        input_map,
        pointer,
        Rc::clone(&active_canvas_dimensions),
        audio,
        vibration,
        realtime_pacing,
        lifecycle,
    );
    driver.checkpoints.enabled = true;
    driver.checkpoints.host_close = Rc::clone(&context.checkpoints.host_close);
    if let Some(saved) = &saved {
        driver.restore_driver_checkpoint(&saved.driver)?;
    }
    context.events.publish(SessionEvent {
        session_id,
        attempt_id,
        kind: SessionEventKind::Started,
    })?;
    // Host components have been restored. Drop their encoded copies now and
    // transfer the remaining VM bytes so it can release them before its loop.
    let vm_checkpoint = saved.map(|saved| saved.vm);
    let execution = match program.execute_instance_checkpoint_driver_with_context(
        &class,
        |turn| driver.next_step(turn),
        limits,
        false,
        &mut context,
        vm_checkpoint,
    ) {
        Ok(execution) => execution,
        Err(error)
            if error.code() == "execution-cancelled"
                && (urgent.stop_requested(attempt_id) || urgent.shutdown_requested()) =>
        {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if let Some(error) =
        runtime::thread_failure_error(&execution.thread_failures, execution.thread_failure_count)
    {
        return Err(error);
    }
    Ok(())
}

fn connector_permissions(persona: &device_profile::DeviceProfile) -> gcf::PermissionPolicy {
    let mut permissions = vec![
        gcf::HTTP_PERMISSION.to_owned(),
        gcf::HTTPS_PERMISSION.to_owned(),
    ];
    if persona.java().supports_jsr("75") {
        permissions.extend([
            gcf::FILE_READ_PERMISSION.to_owned(),
            gcf::FILE_WRITE_PERMISSION.to_owned(),
        ]);
    }
    gcf::PermissionPolicy::new(permissions)
}

pub(super) fn attempt_stop_requested(urgent: &UrgentState, attempt_id: AttemptId) -> bool {
    urgent.stop_requested(attempt_id) || urgent.shutdown_requested() || urgent.force_cancelled()
}

fn rms_suite_id(
    entry: &LibraryEntry,
    suite: &midp::SuiteDescriptor,
) -> Result<rms::SuiteId, EmuError> {
    let vendor = suite
        .properties
        .get("midlet-vendor")
        .cloned()
        .unwrap_or_else(|| "J2Play local suite".to_owned());
    let name = suite
        .properties
        .get("midlet-name")
        .cloned()
        .unwrap_or_else(|| entry.title().to_owned());
    rms::SuiteId::new(vendor, name)
}

pub(super) fn recovery_candidates(
    repository: &LibraryRepository,
    entry_id: &str,
    orientation: Option<launch::CanvasOrientation>,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<String>, EmuError> {
    let entry = repository.load_entry(entry_id)?;
    if !matches!(entry.settings().device_profile, ProfileChoice::Automatic) {
        return Ok(Vec::new());
    }
    let prepared = repository.prepare_launch_cancellable(&entry, cancelled)?;
    let profiles = launch::builtin_device_profiles()?;
    let decision = prepared
        .launch_plan_with_orientation(entry.settings(), orientation)?
        .decision;
    let fallback = u64::try_from(vm::Limits::default().max_heap_bytes).unwrap_or(u64::MAX);
    let mut candidates = launch::managed_heap_recovery_candidates(
        profiles,
        &prepared.suite().properties,
        prepared.evidence(),
        &decision,
        fallback,
        false,
    )?
    .into_iter()
    .map(|candidate| candidate.selection().profile_id().to_owned())
    .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

#[derive(Default)]
struct PauseSources {
    user: bool,
    lifecycle: bool,
    platform: bool,
}

pub(super) struct AttemptDriver {
    session_id: SessionId,
    attempt_id: AttemptId,
    commands: CommandQueue,
    urgent: Arc<UrgentState>,
    ams: Rc<RefCell<midp::Ams>>,
    input: platform::InputState,
    pointer: platform::PointerInputPolicy,
    active_canvas_dimensions: Rc<Cell<(u32, u32)>>,
    audio: AudioMailbox,
    vibration: VibrationMailbox,
    pending: VecDeque<vm::DriverStep>,
    pending_text: VecDeque<vm::DriverStep>,
    composing: bool,
    paused: PauseSources,
    close_queued: bool,
    captured_pointer: Option<u64>,
    pointer_position: (i32, i32),
    last_idle: Instant,
    realtime_pacing: Rc<Cell<bool>>,
    lifecycle: PlatformLifecycleSignal,
    checkpoints: checkpoint::DriverCheckpoints,
}

pub(super) fn update_text_input_state(current: &mut bool, requested: bool) -> bool {
    if *current == requested {
        return false;
    }
    *current = requested;
    true
}
