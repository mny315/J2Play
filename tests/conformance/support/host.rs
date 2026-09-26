use super::gcf_transport::{FixedResolver, MockTransport};
use super::{Fixture, MAX_CAPTURE_BYTES, push_capture};
use diagnostics::{Category, EmuError};
use natives::{MidletLifecycleEvent, MidletNotification};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

pub(super) struct FixtureHost {
    display_width: u32,
    display_height: u32,
    normal_canvas_width: u32,
    normal_canvas_height: u32,
    pointer: platform::PointerInputPolicy,
    canvas_input: runtime::CanvasInputProfile,
    display_colors: i32,
    lcd_ui_font_heights: Option<device_profile::LcdUiFontHeights>,
    properties: cldc::SystemProperties,
    bluetooth_properties: bluetooth::Properties,
    jar_resources: jar::ResourceArchive,
    suite_properties: BTreeMap<String, String>,
    rms_runtime: rms::Runtime,
    pub(super) mmapi_runtime: mmapi::Runtime<mmapi::NullAudioSink>,
    gcf_runtime: gcf::Runtime,
    permissions: HashSet<String>,
    unknown_permissions: HashSet<String>,
    allow_platform: bool,
    wall_clock: i64,
    deadline: Instant,
    started: Instant,
    trace: bool,
    pub(super) ams: Rc<RefCell<midp::Ams>>,
    pub(super) console: String,
    pub(super) flight_events: Vec<String>,
    pub(super) frame: Option<(u32, u32, Vec<u32>)>,
    pub(super) frames: u64,
    pub(super) lifecycle: Vec<MidletNotification>,
    pub(super) platform_requests: usize,
}

impl FixtureHost {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        config: &Fixture,
        scratch: &Path,
        profile: &device_profile::DeviceProfile,
        bytes: Vec<u8>,
        properties: BTreeMap<String, String>,
        hash: &str,
        deadline: Instant,
    ) -> Result<Self, EmuError> {
        let (display_width, display_height) = runtime::profile_canvas_dimensions(profile)?;
        let (normal_canvas_width, normal_canvas_height) =
            runtime::profile_non_fullscreen_canvas_dimensions(profile)?;
        let resolved = profile.resolve(device_profile::ProfileOverrides::default())?;
        let vendor = properties
            .get("midlet-vendor")
            .map_or("J2Play local suite", String::as_str);
        let name = properties.get("midlet-name").map_or(hash, String::as_str);
        let rms_runtime = rms::Runtime::new(
            config
                .rms_root
                .clone()
                .unwrap_or_else(|| scratch.join("rms")),
            rms::SuiteId::new(vendor, name)?,
            rms::Limits::default(),
        );
        let gcf_runtime = gcf::Runtime::new(
            gcf::PermissionPolicy::new(config.permissions.iter().cloned()),
            Box::new(MockTransport::new([])),
            Box::new(FixedResolver { addresses: vec![] }),
            &config
                .file_root
                .clone()
                .unwrap_or_else(|| scratch.join("files")),
            gcf::Limits::default(),
            true,
        )?;
        Ok(Self {
            display_width,
            display_height,
            normal_canvas_width,
            normal_canvas_height,
            pointer: platform::PointerInputPolicy::new(
                resolved.pointer_events(),
                resolved.pointer_motion_events(),
            ),
            canvas_input: runtime::CanvasInputProfile::from_profile(profile),
            display_colors: runtime::profile_display_colors(profile)?,
            lcd_ui_font_heights: runtime::profile_lcd_ui_font_heights(profile),
            properties: cldc::SystemProperties::from_profile(profile),
            bluetooth_properties: bluetooth::Properties::from_profile(profile),
            jar_resources: jar::ResourceArchive::from_owned_bytes(bytes)?,
            suite_properties: properties,
            rms_runtime,
            gcf_runtime,
            mmapi_runtime: mmapi::Runtime::new(
                mmapi::NullAudioSink::default(),
                mmapi::Limits::default(),
            )
            .with_decode_cancellation(move || Instant::now() >= deadline),
            permissions: config.permissions.clone(),
            unknown_permissions: config.unknown_permissions.clone(),
            allow_platform: config.allow_platform,
            wall_clock: config.wall_clock,
            deadline,
            started: Instant::now(),
            trace: config.trace,
            ams: Rc::new(RefCell::new(midp::Ams::new(
                config.host_events.iter().copied(),
            )?)),
            console: String::new(),
            flight_events: Vec::new(),
            frame: None,
            frames: 0,
            lifecycle: Vec::new(),
            platform_requests: 0,
        })
    }
}

impl natives::HostServices for FixtureHost {
    fn monotonic_millis(&self) -> i64 {
        i64::try_from(self.started.elapsed().as_millis()).unwrap_or(i64::MAX)
    }
    fn wall_clock_millis(&self) -> i64 {
        self.wall_clock.saturating_add(self.monotonic_millis())
    }
    fn execution_cancelled(&self) -> bool {
        Instant::now() >= self.deadline
    }
    fn vm_flight_recorder_enabled(&self) -> bool {
        self.trace
    }
    fn record_vm_flight(&mut self, event: String) {
        if self.flight_events.len() < 4096 && event.len() <= 4096 {
            self.flight_events.push(event);
        }
    }
    fn write_console_error(&mut self, line: &str) -> Result<(), EmuError> {
        push_capture(&mut self.console, line);
        push_capture(&mut self.console, "\n");
        Ok(())
    }
    fn write_console_output(&mut self, text: &str, newline: bool) -> Result<(), EmuError> {
        push_capture(&mut self.console, text);
        if newline {
            push_capture(&mut self.console, "\n");
        }
        Ok(())
    }
    fn midlet_property(&self, name: &str) -> Option<&str> {
        self.suite_properties
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
    fn midlet_lifecycle_event(&mut self, event: MidletLifecycleEvent) -> Result<(), EmuError> {
        runtime::midlet_lifecycle_event(&mut self.ams.borrow_mut(), event)?;
        if let MidletLifecycleEvent::Notification(notification) = event
            && self.lifecycle.len() < 1024
        {
            self.lifecycle.push(notification);
        }
        Ok(())
    }
    fn platform_request(&mut self, url: &str) -> Result<bool, EmuError> {
        if !self.allow_platform {
            return Err(EmuError::new(
                Category::Platform,
                "connection-not-found",
                "fixture policy denied platform request",
            ));
        }
        assert!(url.len() < MAX_CAPTURE_BYTES);
        self.platform_requests = self.platform_requests.saturating_add(1);
        Ok(false)
    }
    fn check_permission(&mut self, name: &str) -> Result<i32, EmuError> {
        Ok(if self.permissions.contains(name) {
            1
        } else if self.unknown_permissions.contains(name) {
            -1
        } else {
            0
        })
    }
    fn uncaught_thread_exception(
        &mut self,
        _: u64,
        _: &str,
        _: Option<&str>,
        _: &[String],
        _: bool,
    ) {
        self.ams.borrow_mut().request_close();
    }
    fn present_lcdui_frame(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u32],
    ) -> Result<(), EmuError> {
        let full = (self.display_width, self.display_height);
        let normal = (self.normal_canvas_width, self.normal_canvas_height);
        if (width, height) != full && (width, height) != normal {
            return Err(EmuError::new(
                Category::Api,
                "framebuffer-size",
                "invalid fixture Canvas size",
            ));
        }
        let mut composed = Vec::new();
        runtime::compose_unscaled_lcdui_frame(full, (width, height), pixels, &mut composed)?;
        self.frames = self.frames.saturating_add(1);
        self.frame = Some((full.0, full.1, composed));
        Ok(())
    }
    fn lcd_dimensions(&self) -> (u32, u32) {
        (self.display_width, self.display_height)
    }

    fn lcd_non_fullscreen_dimensions(&self) -> (u32, u32) {
        (self.normal_canvas_width, self.normal_canvas_height)
    }

    fn lcd_ui_font_height(&self, size: i32) -> Option<i32> {
        let heights = self.lcd_ui_font_heights?;
        let height = match size {
            8 => heights.small(),
            16 => heights.large(),
            _ => heights.medium(),
        };
        i32::try_from(height).ok()
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

    fn system_property(&self, name: &str) -> Option<&str> {
        self.properties.get(name)
    }

    fn bluetooth_property(&self, name: &str) -> Option<&str> {
        self.bluetooth_properties.get(name)
    }

    fn display_colors(&self) -> i32 {
        self.display_colors
    }

    fn read_resource(&self, name: &str) -> Result<Option<Vec<u8>>, EmuError> {
        self.jar_resources.read(name)
    }

    fn gcf_http(
        &mut self,
        request: natives::GcfHttpRequest,
    ) -> Result<natives::GcfHttpResponse, EmuError> {
        self.gcf_runtime.http(request)
    }

    fn gcf_file_metadata(&mut self, url: &str) -> Result<natives::GcfFileMetadata, EmuError> {
        self.gcf_runtime.file_metadata(url)
    }

    fn gcf_file_read(&mut self, url: &str) -> Result<Vec<u8>, EmuError> {
        self.gcf_runtime.file_read(url)
    }

    fn gcf_file_revision(&self) -> Result<u64, EmuError> {
        Ok(self.gcf_runtime.file_revision())
    }

    fn gcf_file_write(&mut self, url: &str, data: &[u8], append: bool) -> Result<(), EmuError> {
        self.gcf_runtime.file_write(url, data, append)
    }

    fn gcf_file_output_offset(&mut self, url: &str, offset: u64) -> Result<u64, EmuError> {
        self.gcf_runtime.file_output_offset(url, offset)
    }

    fn gcf_file_write_at(&mut self, url: &str, data: &[u8], offset: u64) -> Result<(), EmuError> {
        self.gcf_runtime.file_write_at(url, data, offset)
    }

    fn gcf_file_create(&mut self, url: &str) -> Result<(), EmuError> {
        self.gcf_runtime.file_create(url)
    }

    fn gcf_file_mkdir(&mut self, url: &str) -> Result<(), EmuError> {
        self.gcf_runtime.file_mkdir(url)
    }

    fn gcf_file_delete(&mut self, url: &str) -> Result<(), EmuError> {
        self.gcf_runtime.file_delete(url)
    }

    fn gcf_file_rename(&mut self, url: &str, name: &str) -> Result<String, EmuError> {
        self.gcf_runtime.file_rename(url, name)
    }

    fn gcf_file_truncate(&mut self, url: &str, size: u64) -> Result<(), EmuError> {
        self.gcf_runtime.file_truncate(url, size)
    }

    fn gcf_file_list(&mut self, url: &str) -> Result<Vec<String>, EmuError> {
        self.gcf_runtime.file_list(url)
    }

    fn gcf_file_directory_size(&mut self, url: &str, recursive: bool) -> Result<u64, EmuError> {
        self.gcf_runtime.file_directory_size(url, recursive)
    }

    fn gcf_file_space(&mut self, selector: i32) -> Result<u64, EmuError> {
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
}
