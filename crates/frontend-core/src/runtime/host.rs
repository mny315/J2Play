use super::{
    Arc, AttemptId, AudioMailbox, BTreeMap, Category, Cell, CommandQueue, Duration, EmuError,
    EventMailbox, Frame, HostDecision, HostRequestKind, Instant, LatestFrameMailbox,
    LatestTelemetryMailbox, PlatformLifecycleSignal, Rc, RefCell, RequestBroker, RuntimeTelemetry,
    SessionEvent, SessionEventKind, SessionId, UrgentState, VibrationEffect, VibrationMailbox,
    bounded_detail, compose_unscaled_lcdui_frame, thread, update_text_input_state,
};
use natives::{MidletLifecycleEvent, MidletNotification};
use runtime::CanvasInputProfile;

pub(super) struct FrontendAudioSink {
    pub(super) session_id: SessionId,
    pub(super) attempt_id: AttemptId,
    pub(super) mailbox: AudioMailbox,
}

impl mmapi::AudioSink for FrontendAudioSink {
    fn write(&mut self, samples: &[i16]) -> Result<(), EmuError> {
        let _ = self
            .mailbox
            .publish(self.session_id, self.attempt_id, samples)?;
        Ok(())
    }
}

pub(super) struct FrontendNativeContext {
    pub(super) session_id: SessionId,
    pub(super) attempt_id: AttemptId,
    pub(super) started: Instant,
    pub(super) display_width: u32,
    pub(super) display_height: u32,
    pub(super) normal_canvas_width: u32,
    pub(super) normal_canvas_height: u32,
    pub(super) active_canvas_dimensions: Rc<Cell<(u32, u32)>>,
    pub(super) pointer: platform::PointerInputPolicy,
    pub(super) canvas_input: CanvasInputProfile,
    pub(super) display_colors: i32,
    pub(super) lcd_ui_font_heights: Option<device_profile::LcdUiFontHeights>,
    pub(super) properties: cldc::SystemProperties,
    pub(super) bluetooth_properties: bluetooth::Properties,
    pub(super) jar_resources: jar::ResourceArchive,
    pub(super) suite_properties: BTreeMap<String, String>,
    pub(super) ams: Rc<RefCell<midp::Ams>>,
    pub(super) events: EventMailbox,
    pub(super) frames: LatestFrameMailbox,
    pub(super) telemetry: LatestTelemetryMailbox,
    pub(super) commands: CommandQueue,
    pub(super) urgent: Arc<UrgentState>,
    pub(super) lifecycle: PlatformLifecycleSignal,
    pub(super) composed_frame: Vec<u32>,
    pub(super) frame_pacer: platform::FramePacer,
    pub(super) interpreter_instructions_per_second: Option<u64>,
    pub(super) rms_runtime: rms::Runtime,
    pub(super) mmapi_runtime: mmapi::Runtime<FrontendAudioSink>,
    pub(super) gcf_runtime: gcf::Runtime,
    pub(super) vibration: VibrationMailbox,
    pub(super) request_broker: RequestBroker,
    pub(super) allow_heap_recovery: bool,
    pub(super) realtime_pacing: Rc<Cell<bool>>,
    pub(super) text_input_active: bool,
    pub(super) checkpoints: super::checkpoint::ContextCheckpoints,
}

impl FrontendNativeContext {
    fn publish_diagnostic(&self, code: &str, detail: &str) -> Result<(), EmuError> {
        self.events.publish(SessionEvent {
            session_id: self.session_id,
            attempt_id: self.attempt_id,
            kind: SessionEventKind::Diagnostic {
                code: bounded_detail(code, 128),
                detail: bounded_detail(detail, 8 * 1024),
                repeated: 1,
            },
        })
    }

    pub(super) fn present_physical_frame(
        &mut self,
        pixels: &[u32],
        canvas_region: platform::LogicalRect,
    ) -> Result<(), EmuError> {
        let frame = Arc::new(Frame::with_canvas_region(
            self.session_id,
            self.attempt_id,
            self.display_width,
            self.display_height,
            canvas_region,
            Arc::from(pixels),
        )?);
        self.checkpoints.last_frame = Some(crate::library::checkpoint::SavedFrame {
            canvas_region: [
                canvas_region.x,
                canvas_region.y,
                canvas_region.width,
                canvas_region.height,
            ],
            pixels: Arc::clone(&frame.pixels),
        });
        if self.frames.publish(frame)? {
            self.events.publish(SessionEvent {
                session_id: self.session_id,
                attempt_id: self.attempt_id,
                kind: SessionEventKind::FrameAvailable,
            })?;
        }
        Ok(())
    }

    fn authorize(&mut self, kind: HostRequestKind) -> Result<HostDecision, EmuError> {
        self.request_broker.decide(kind)
    }

    fn authorize_file(&mut self, url: &str) -> Result<(), EmuError> {
        self.request_broker
            .require_access(HostRequestKind::FileConnection {
                url: url.to_owned(),
            })
    }
}

impl natives::HostServices for FrontendNativeContext {
    fn lcd_dimensions(&self) -> (u32, u32) {
        (self.display_width, self.display_height)
    }

    fn lcd_non_fullscreen_dimensions(&self) -> (u32, u32) {
        (self.normal_canvas_width, self.normal_canvas_height)
    }

    fn lcd_ui_font_height(&self, size: i32) -> Option<i32> {
        let heights = self.lcd_ui_font_heights?;
        i32::try_from(match size {
            8 => heights.small(),
            16 => heights.large(),
            _ => heights.medium(),
        })
        .ok()
    }

    fn canvas_game_action(&self, key_code: i32) -> Option<i32> {
        self.canvas_input.game_action(key_code)
    }

    fn canvas_key_code(&self, game_action: i32) -> Option<i32> {
        self.canvas_input.key_code(game_action)
    }

    fn canvas_key_name(&self, key_code: i32) -> Option<&str> {
        self.canvas_input.key_name(key_code)
    }

    fn canvas_pointer_events(&self) -> bool {
        self.pointer.device_events()
    }

    fn canvas_pointer_motion_events(&self) -> bool {
        self.pointer.device_motion_events()
    }

    fn set_text_input_active(&mut self, active: bool) {
        if !update_text_input_state(&mut self.text_input_active, active) {
            return;
        }
        let _ = self.events.publish(SessionEvent {
            session_id: self.session_id,
            attempt_id: self.attempt_id,
            kind: SessionEventKind::TextInputActive { active },
        });
    }

    fn request_vibration(&mut self, request: natives::VibrationRequest) -> bool {
        self.vibration
            .publish(VibrationEffect {
                session_id: self.session_id,
                attempt_id: self.attempt_id,
                request,
            })
            .unwrap_or(false)
    }

    fn monotonic_millis(&self) -> i64 {
        self.checkpoints
            .monotonic_base
            .saturating_add(i64::try_from(self.started.elapsed().as_millis()).unwrap_or(i64::MAX))
    }

    fn wall_clock_millis(&self) -> i64 {
        super::checkpoint::system_millis().saturating_add(self.checkpoints.wall_offset)
    }

    fn save_checkpoint(
        &mut self,
        vm: &[u8],
        driver: &[u8],
        clock: (i64, i64),
    ) -> Result<(), EmuError> {
        self.write_automatic_checkpoint(vm, driver, clock)
    }

    fn checkpoint_restored(&mut self) -> Result<(), EmuError> {
        self.finish_checkpoint_restore()
    }

    fn checkpoint_failed(&mut self, error: &EmuError) {
        self.report_checkpoint_failure(error);
    }

    fn realtime_pacing(&self) -> bool {
        self.realtime_pacing.get()
    }

    fn realtime_interpreter_instructions_per_second(&self) -> Option<u64> {
        self.interpreter_instructions_per_second
    }

    fn pace_lcdui_frame_request(&mut self) -> Result<(), EmuError> {
        let delay = self
            .frame_pacer
            .frame_request_delay_millis(self.realtime_pacing.get());
        self.pace_millis(delay)
    }

    fn pace_millis(&mut self, millis: u64) -> Result<(), EmuError> {
        if !self.realtime_pacing.get() {
            return Ok(());
        }
        let mut remaining = millis;
        while remaining != 0 {
            if self.lifecycle.suspended() {
                return Ok(());
            }
            if self.urgent.stop_requested(self.attempt_id) || self.urgent.shutdown_requested() {
                return Ok(());
            }
            if self.execution_cancelled() {
                return Err(EmuError::new(
                    Category::Vm,
                    "execution-cancelled",
                    "guest execution was cancelled by the host",
                ));
            }
            let step = remaining.min(10);
            thread::sleep(Duration::from_millis(step));
            remaining -= step;
        }
        Ok(())
    }

    fn execution_cancelled(&self) -> bool {
        self.urgent.force_cancelled() || self.urgent.stop_cancellation_due(self.attempt_id)
    }

    fn execution_suspended(&self) -> bool {
        self.lifecycle.suspended()
    }

    fn update_vm_telemetry(&mut self, telemetry: natives::VmTelemetry) {
        let _ = self.telemetry.publish(RuntimeTelemetry {
            session_id: self.session_id,
            attempt_id: self.attempt_id,
            stats: platform::VmDebugStats {
                instructions: telemetry.instructions,
                heap_bytes: telemetry.heap_bytes,
                runnable_threads: telemetry.runnable_threads,
                continuations: telemetry.continuations,
                native_calls: telemetry.native_calls,
                arraycopy_calls: telemetry.arraycopy_calls,
                arraycopy_elements: telemetry.arraycopy_elements,
                draw_region_calls: telemetry.draw_region_calls,
                draw_image_calls: telemetry.draw_image_calls,
                draw_rgb_calls: telemetry.draw_rgb_calls,
                fill_rect_calls: telemetry.fill_rect_calls,
                m3g_live_bytes: telemetry.m3g_live_bytes,
                m3g_peak_bytes: telemetry.m3g_peak_bytes,
                m3g_live_objects: telemetry.m3g_live_objects,
                m3g_peak_objects: telemetry.m3g_peak_objects,
                m3g_loaded_files: telemetry.m3g_loaded_files,
                m3g_loaded_objects: telemetry.m3g_loaded_objects,
                m3g_submitted_triangles: telemetry.m3g_submitted_triangles,
                m3g_rasterized_triangles: telemetry.m3g_rasterized_triangles,
                m3g_shaded_pixels: telemetry.m3g_shaded_pixels,
            },
        });
    }

    fn system_property(&self, name: &str) -> Option<&str> {
        self.properties.get(name)
    }

    fn bluetooth_property(&self, name: &str) -> Option<&str> {
        self.bluetooth_properties.get(name)
    }

    fn display_colors(&self) -> i32 {
        self.display_colors
    }

    fn repeated_managed_heap_limit(
        &mut self,
        _notice: natives::ManagedHeapLimitNotice,
    ) -> natives::ManagedHeapLimitDecision {
        if self.allow_heap_recovery {
            natives::ManagedHeapLimitDecision::RequestProfileChange
        } else {
            natives::ManagedHeapLimitDecision::Continue
        }
    }

    fn uncaught_thread_exception(
        &mut self,
        thread_id: u64,
        exception_class: &str,
        exception_message: Option<&str>,
        _stack_trace: &[String],
        _managed_heap_limit: bool,
    ) {
        let detail = exception_message.map_or_else(
            || format!("thread {thread_id}: {exception_class}"),
            |message| format!("thread {thread_id}: {exception_class}: {message}"),
        );
        let _ = self.publish_diagnostic("uncaught-thread-exception", &detail);
        self.ams.borrow_mut().request_close();
    }

    fn read_resource(&self, name: &str) -> Result<Option<Vec<u8>>, EmuError> {
        self.jar_resources.read(name)
    }

    fn gcf_http(
        &mut self,
        request: natives::GcfHttpRequest,
    ) -> Result<natives::GcfHttpResponse, EmuError> {
        self.request_broker
            .require_access(HostRequestKind::Network {
                url: request.url.clone(),
            })?;
        self.gcf_runtime.http(request)
    }

    fn gcf_file_metadata(&mut self, url: &str) -> Result<natives::GcfFileMetadata, EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_metadata(url)
    }

    fn gcf_file_read(&mut self, url: &str) -> Result<Vec<u8>, EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_read(url)
    }
    fn gcf_file_revision(&self) -> Result<u64, EmuError> {
        Ok(self.gcf_runtime.file_revision())
    }

    fn gcf_file_write(&mut self, url: &str, data: &[u8], append: bool) -> Result<(), EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_write(url, data, append)
    }
    fn gcf_file_output_offset(&mut self, url: &str, offset: u64) -> Result<u64, EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_output_offset(url, offset)
    }

    fn gcf_file_write_at(&mut self, url: &str, data: &[u8], offset: u64) -> Result<(), EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_write_at(url, data, offset)
    }

    fn gcf_file_create(&mut self, url: &str) -> Result<(), EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_create(url)
    }

    fn gcf_file_mkdir(&mut self, url: &str) -> Result<(), EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_mkdir(url)
    }

    fn gcf_file_delete(&mut self, url: &str) -> Result<(), EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_delete(url)
    }

    fn gcf_file_rename(&mut self, url: &str, name: &str) -> Result<String, EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_rename(url, name)
    }

    fn gcf_file_truncate(&mut self, url: &str, size: u64) -> Result<(), EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_truncate(url, size)
    }

    fn gcf_file_list(&mut self, url: &str) -> Result<Vec<String>, EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_list(url)
    }

    fn gcf_file_directory_size(&mut self, url: &str, recursive: bool) -> Result<u64, EmuError> {
        self.authorize_file(url)?;
        self.gcf_runtime.file_directory_size(url, recursive)
    }

    fn gcf_file_space(&mut self, selector: i32) -> Result<u64, EmuError> {
        let synthetic_url = "file:///";
        self.authorize_file(synthetic_url)?;
        self.gcf_runtime.file_space(selector)
    }

    fn rms_open(&mut self, name: &str, create: bool) -> Result<u64, EmuError> {
        let now = self.wall_clock_millis();
        self.rms_runtime.open(name, create, now)
    }

    fn rms_open_owned(&mut self, name: &str, vendor: &str, suite: &str) -> Result<u64, EmuError> {
        let now = self.wall_clock_millis();
        self.rms_runtime.open_owned(name, vendor, suite, now)
    }

    fn rms_close(&mut self, handle: u64) -> Result<(), EmuError> {
        self.rms_runtime.close(handle)
    }

    fn rms_delete_store(&mut self, name: &str) -> Result<(), EmuError> {
        self.rms_runtime.delete_store(name)
    }

    fn rms_list_stores(&mut self) -> Result<Vec<String>, EmuError> {
        self.rms_runtime.list_stores()
    }

    fn rms_metadata(
        &mut self,
        handle: u64,
        field: natives::RmsMetadataField,
    ) -> Result<i64, EmuError> {
        self.rms_runtime.metadata(handle, field)
    }

    fn rms_record_ids(&mut self, handle: u64) -> Result<Vec<i32>, EmuError> {
        self.rms_runtime.record_ids(handle)
    }

    fn rms_get(&mut self, handle: u64, record_id: i32) -> Result<Vec<u8>, EmuError> {
        self.rms_runtime.get(handle, record_id).map(<[u8]>::to_vec)
    }

    fn rms_record_size(&mut self, handle: u64, record_id: i32) -> Result<usize, EmuError> {
        self.rms_runtime.get(handle, record_id).map(<[u8]>::len)
    }

    fn rms_add(&mut self, handle: u64, data: &[u8]) -> Result<i32, EmuError> {
        let now = self.wall_clock_millis();
        self.rms_runtime.add(handle, data, now)
    }

    fn rms_set(&mut self, handle: u64, record_id: i32, data: &[u8]) -> Result<(), EmuError> {
        let now = self.wall_clock_millis();
        self.rms_runtime.set(handle, record_id, data, now)
    }

    fn rms_delete(&mut self, handle: u64, record_id: i32) -> Result<(), EmuError> {
        let now = self.wall_clock_millis();
        self.rms_runtime.delete(handle, record_id, now)
    }

    fn mmapi_create_bytes(&mut self, content_type: &str, data: &[u8]) -> Result<u64, EmuError> {
        self.mmapi_runtime.create_from_bytes(content_type, data)
    }

    fn mmapi_create_tone(&mut self) -> Result<u64, EmuError> {
        self.mmapi_runtime.create_tone_player()
    }

    fn mmapi_create_midi(&mut self) -> Result<u64, EmuError> {
        self.mmapi_runtime.create_midi_player()
    }

    fn mmapi_retain_handles(&mut self, handles: &[u64]) {
        self.mmapi_runtime.retain_handles(handles);
    }

    fn mmapi_state(&mut self, handle: u64, now_micros: i64) -> Result<i32, EmuError> {
        self.mmapi_runtime
            .state(handle, now_micros)
            .map(|state| state as i32)
    }

    fn mmapi_transition(
        &mut self,
        handle: u64,
        transition: i32,
        now_micros: i64,
    ) -> Result<(), EmuError> {
        self.mmapi_runtime
            .transition(handle, transition, now_micros)
    }

    fn mmapi_start_exclusive(&mut self, handle: u64, now_micros: i64) -> Result<(), EmuError> {
        self.mmapi_runtime.start_exclusive(handle, now_micros)
    }

    fn mmapi_content_type(&mut self, handle: u64) -> Result<String, EmuError> {
        self.mmapi_runtime.content_type(handle).map(str::to_owned)
    }

    fn mmapi_time(&mut self, handle: u64, selector: i32, now_micros: i64) -> Result<i64, EmuError> {
        self.mmapi_runtime.time(handle, selector, now_micros)
    }

    fn mmapi_set_media_time(
        &mut self,
        handle: u64,
        time: i64,
        now_micros: i64,
    ) -> Result<i64, EmuError> {
        self.mmapi_runtime.set_media_time(handle, time, now_micros)
    }

    fn mmapi_set_loop_count(&mut self, handle: u64, count: i32) -> Result<(), EmuError> {
        self.mmapi_runtime.set_loop_count(handle, count)
    }

    fn mmapi_volume(&mut self, handle: u64) -> Result<i32, EmuError> {
        self.mmapi_runtime.volume(handle)
    }

    fn mmapi_set_volume(&mut self, handle: u64, volume: i32) -> Result<i32, EmuError> {
        self.mmapi_runtime.set_volume(handle, volume)
    }

    fn mmapi_muted(&mut self, handle: u64) -> Result<bool, EmuError> {
        self.mmapi_runtime.muted(handle)
    }

    fn mmapi_set_muted(&mut self, handle: u64, muted: bool) -> Result<(), EmuError> {
        self.mmapi_runtime.set_muted(handle, muted)
    }

    fn mmapi_set_tone_sequence(&mut self, handle: u64, sequence: &[u8]) -> Result<(), EmuError> {
        self.mmapi_runtime.set_tone_sequence(handle, sequence)
    }

    fn mmapi_next_event(&mut self, handle: u64, now_micros: i64) -> Result<i32, EmuError> {
        self.mmapi_runtime
            .next_event(handle, now_micros)
            .map(|event| event.map_or(0, |event| event.kind.code()))
    }

    fn mmapi_play_tone(
        &mut self,
        note: i32,
        duration_millis: i32,
        volume: i32,
        now_micros: i64,
    ) -> Result<(), EmuError> {
        self.mmapi_runtime
            .play_tone(note, duration_millis, volume, now_micros)
    }

    fn mmapi_pump(&mut self, now_micros: i64) -> Result<(), EmuError> {
        self.mmapi_runtime.tick(now_micros)
    }

    fn midlet_property(&self, name: &str) -> Option<&str> {
        self.suite_properties
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    fn midlet_lifecycle_event(&mut self, event: MidletLifecycleEvent) -> Result<(), EmuError> {
        runtime::midlet_lifecycle_event(&mut self.ams.borrow_mut(), event)?;
        if event == MidletLifecycleEvent::Notification(MidletNotification::Destroyed)
            && !self.checkpoints.host_close.get()
            && !super::attempt::attempt_stop_requested(&self.urgent, self.attempt_id)
            && let Err(error) = self
                .checkpoints
                .repository
                .invalidate_checkpoint(&self.checkpoints.entry, self.checkpoints.epoch)
        {
            self.report_checkpoint_failure(&error);
        }
        Ok(())
    }

    fn platform_request(&mut self, url: &str) -> Result<bool, EmuError> {
        match self.authorize(HostRequestKind::PlatformRequest {
            url: url.to_owned(),
        })? {
            HostDecision::AllowOnce | HostDecision::AllowForSession => Ok(false),
            HostDecision::AllowAfterExit => Ok(true),
            HostDecision::Deny | HostDecision::ResumeGame { .. } => Err(EmuError::new(
                Category::Platform,
                "connection-not-found",
                "platform request denied by user or host policy",
            )),
        }
    }

    fn check_permission(&mut self, permission: &str) -> Result<i32, EmuError> {
        Ok(if self.gcf_runtime.permission_allowed(permission) {
            self.request_broker.permission_status(permission)
        } else {
            0
        })
    }

    fn write_console_error(&mut self, line: &str) -> Result<(), EmuError> {
        self.publish_diagnostic("guest-console-error", line)
    }

    fn write_console_output(&mut self, text: &str, newline: bool) -> Result<(), EmuError> {
        let detail = if newline {
            text.to_owned()
        } else {
            format!("{text} [partial]")
        };
        self.publish_diagnostic("guest-console-output", &detail)
    }

    fn lcdui_frame_delay_millis(&mut self, changed: bool) -> u64 {
        self.frame_pacer
            .frame_delay_millis(self.realtime_pacing.get(), changed)
    }

    fn present_lcdui_frame(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u32],
    ) -> Result<(), EmuError> {
        let expected = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| {
                EmuError::new(Category::Api, "framebuffer-size", "frame size overflow")
            })?;
        if pixels.len() != expected {
            return Err(EmuError::new(
                Category::Api,
                "framebuffer-size",
                "host frame does not match its declared dimensions",
            ));
        }
        let dimensions = (width, height);
        let full = (self.display_width, self.display_height);
        let normal = (self.normal_canvas_width, self.normal_canvas_height);
        if dimensions == full {
            self.active_canvas_dimensions.set(dimensions);
            return self.present_physical_frame(
                pixels,
                platform::LogicalRect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                },
            );
        }
        if dimensions != normal {
            return Err(EmuError::new(
                Category::Api,
                "framebuffer-size",
                "host frame matches neither Canvas mode in the active device profile",
            ));
        }
        self.active_canvas_dimensions.set(dimensions);
        let mut composed = std::mem::take(&mut self.composed_frame);
        let canvas_region = compose_unscaled_lcdui_frame(full, normal, pixels, &mut composed)?;
        let result = self.present_physical_frame(&composed, canvas_region);
        self.composed_frame = composed;
        result
    }
}
