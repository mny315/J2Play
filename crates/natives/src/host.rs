//! Frontend and operating-system capabilities exposed to the VM.

use super::{MidletLifecycleEvent, native_error};
use diagnostics::EmuError;

mod types;

pub use types::{
    GcfFileMetadata, GcfHttpRequest, GcfHttpResponse, ManagedHeapLimitDecision,
    ManagedHeapLimitNotice, RmsMetadataField, VibrationRequest, VmTelemetry,
};

/// Frontend and operating-system services exposed to the VM.
///
/// Fallible methods return a categorized diagnostic when a service is absent,
/// a handle is stale, input violates a boundary, or the host operation fails.
#[allow(clippy::missing_errors_doc)]
pub trait HostServices {
    fn monotonic_millis(&self) -> i64;
    fn wall_clock_millis(&self) -> i64;
    fn system_property(&self, name: &str) -> Option<&str>;

    /// Returns one case-sensitive JSR-82 property selected by the active
    /// device profile. Unknown properties return `None` as required by JSR-82.
    fn bluetooth_property(&self, _name: &str) -> Option<&str> {
        None
    }

    /// Number of physical colors exposed by the active device profile.
    fn display_colors(&self) -> i32 {
        262_144
    }

    /// Whether MIDP Canvas pointer press/release callbacks are available.
    fn canvas_pointer_events(&self) -> bool {
        false
    }

    /// Whether MIDP Canvas pointer drag callbacks are available.
    fn canvas_pointer_motion_events(&self) -> bool {
        false
    }

    /// Shows or hides the host IME for the current MIDP displayable.
    fn set_text_input_active(&mut self, _active: bool) {}

    /// Replaces the vibration effect; returns whether an actuator is available.
    fn request_vibration(&mut self, _request: VibrationRequest) -> bool {
        false
    }

    /// Uses host monotonic time for pacing. Otherwise sleeps advance VM virtual time.
    fn realtime_pacing(&self) -> bool {
        false
    }

    /// Sustainable interpreted guest-bytecode rate for the selected device.
    /// `None` keeps execution unthrottled. The VM applies this only while
    /// [`Self::realtime_pacing`] is enabled.
    fn realtime_interpreter_instructions_per_second(&self) -> Option<u64> {
        None
    }

    /// Paces `Canvas.repaint` on the calling guest thread before coalescing.
    /// Presentation of the same request must not wait a second frame interval.
    fn pace_lcdui_frame_request(&mut self) -> Result<(), EmuError> {
        Ok(())
    }

    /// Waits in host time when [`Self::realtime_pacing`] is enabled.
    fn pace_millis(&mut self, _millis: u64) -> Result<(), EmuError> {
        Ok(())
    }

    /// Applies a suite-scoped compatibility policy to a real-time guest
    /// `Thread.sleep` request. Returning zero preserves the cooperative
    /// scheduling yield while leaving frame timing to the host pacer.
    fn realtime_thread_sleep_millis(&self, requested: u64) -> u64 {
        requested
    }

    /// Overrides the internal 48-bit state used by default-constructed
    /// `java.util.Random` instances. Hosts use this only for deterministic
    /// replay fixtures; normal execution remains wall-clock seeded.
    fn random_seed_override(&self) -> Option<i64> {
        None
    }

    /// Cooperative cancellation checked by the interpreter at bounded
    /// instruction intervals. Product frontends use this only after graceful
    /// lifecycle shutdown misses its measured deadline.
    fn execution_cancelled(&self) -> bool {
        false
    }

    /// Cooperative lifecycle suspension checked beside cancellation. Unlike
    /// cancellation, this preserves the current Java stack so a frontend can
    /// resume the same attempt after its platform surface returns.
    fn execution_suspended(&self) -> bool {
        false
    }

    /// Atomically persists a VM checkpoint together with the driver's state
    /// and the active host runtimes. Called only at a cooperative VM boundary.
    fn save_checkpoint(
        &mut self,
        _vm_state: &[u8],
        _driver_state: &[u8],
        _clock: (i64, i64),
    ) -> Result<(), EmuError> {
        Err(native_error(
            "checkpoint-unavailable",
            "Game checkpoints are unavailable in this context",
        ))
    }

    /// Reports an unsuccessful automatic save without terminating the game.
    fn checkpoint_failed(&mut self, error: &EmuError) {
        eprintln!("J2Play checkpoint: {error}");
    }

    /// Publishes restored host state after all VM validation has succeeded.
    fn checkpoint_restored(&mut self) -> Result<(), EmuError> {
        Ok(())
    }

    fn update_vm_telemetry(&mut self, _telemetry: VmTelemetry) {}

    /// Lets the host continue guest OOM handling or stop for a profile chooser.
    fn repeated_managed_heap_limit(
        &mut self,
        _notice: ManagedHeapLimitNotice,
    ) -> ManagedHeapLimitDecision {
        ManagedHeapLimitDecision::Continue
    }

    /// Whether compact VM flight-recorder events should be produced.
    fn vm_flight_recorder_enabled(&self) -> bool {
        false
    }

    /// Records one flight-recorder event in the host's bounded diagnostic buffer.
    fn record_vm_flight(&mut self, _event: String) {}

    /// Reports an uncaught guest worker exception before the `MIDlet` exits.
    /// The heap flag identifies VM allocation failure independently of its message.
    fn uncaught_thread_exception(
        &mut self,
        _thread_id: u64,
        _exception_class: &str,
        _exception_message: Option<&str>,
        _stack_trace: &[String],
        _managed_heap_limit: bool,
    ) {
    }

    /// Reads a suite resource.
    fn read_resource(&self, name: &str) -> Result<Option<Vec<u8>>, EmuError>;

    fn gcf_http(&mut self, _request: GcfHttpRequest) -> Result<GcfHttpResponse, EmuError> {
        Err(native_error(
            "gcf-unavailable",
            "GCF is unavailable in this context",
        ))
    }
    fn gcf_file_metadata(&mut self, _url: &str) -> Result<GcfFileMetadata, EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_read(&mut self, _url: &str) -> Result<Vec<u8>, EmuError> {
        Err(file_connection_unavailable())
    }
    /// Returns the suite file change token without reading file contents.
    fn gcf_file_revision(&self) -> Result<u64, EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_output_offset(&mut self, _url: &str, _offset: u64) -> Result<u64, EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_write(&mut self, _url: &str, _data: &[u8], _append: bool) -> Result<(), EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_write_at(
        &mut self,
        _url: &str,
        _data: &[u8],
        _offset: u64,
    ) -> Result<(), EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_create(&mut self, _url: &str) -> Result<(), EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_mkdir(&mut self, _url: &str) -> Result<(), EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_delete(&mut self, _url: &str) -> Result<(), EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_rename(&mut self, _url: &str, _name: &str) -> Result<String, EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_truncate(&mut self, _url: &str, _size: u64) -> Result<(), EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_list(&mut self, _url: &str) -> Result<Vec<String>, EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_directory_size(&mut self, _url: &str, _recursive: bool) -> Result<u64, EmuError> {
        Err(file_connection_unavailable())
    }
    fn gcf_file_space(&mut self, _selector: i32) -> Result<u64, EmuError> {
        Err(file_connection_unavailable())
    }

    /// Opens an RMS store in the active suite and returns an opaque host handle.
    fn rms_open(&mut self, _name: &str, _create: bool) -> Result<u64, EmuError> {
        Err(rms_unavailable())
    }

    /// Opens an existing store through the MIDP 2.0 owner-qualified overload.
    fn rms_open_owned(
        &mut self,
        _name: &str,
        _vendor: &str,
        _suite: &str,
    ) -> Result<u64, EmuError> {
        Err(rms_unavailable())
    }

    /// Balances one successful RMS open operation.
    ///
    /// # Errors
    /// Returns `store-not-open` for an unknown or stale handle.
    fn rms_close(&mut self, _handle: u64) -> Result<(), EmuError> {
        Err(rms_unavailable())
    }

    /// Deletes a closed RMS store owned by the active suite.
    fn rms_delete_store(&mut self, _name: &str) -> Result<(), EmuError> {
        Err(rms_unavailable())
    }

    /// Lists committed RMS store names owned by the active suite.
    fn rms_list_stores(&mut self) -> Result<Vec<String>, EmuError> {
        Err(rms_unavailable())
    }

    /// Returns one metadata value for an open RMS handle, without computing other fields.
    /// All fields except `LastModified` are bounded to the Java int range.
    fn rms_metadata(&mut self, _handle: u64, _field: RmsMetadataField) -> Result<i64, EmuError> {
        Err(rms_unavailable())
    }

    /// Returns the record IDs currently present in an open RMS store.
    ///
    /// # Errors
    /// Returns `store-not-open` for an unknown or stale handle.
    fn rms_record_ids(&mut self, _handle: u64) -> Result<Vec<i32>, EmuError> {
        Err(rms_unavailable())
    }

    /// Returns the byte length of one RMS record without copying its payload.
    fn rms_record_size(&mut self, _handle: u64, _record_id: i32) -> Result<usize, EmuError> {
        Err(rms_unavailable())
    }

    /// Reads one RMS record into host-owned memory.
    fn rms_get(&mut self, _handle: u64, _record_id: i32) -> Result<Vec<u8>, EmuError> {
        Err(rms_unavailable())
    }

    /// Atomically adds one RMS record after durable commit.
    fn rms_add(&mut self, _handle: u64, _data: &[u8]) -> Result<i32, EmuError> {
        Err(rms_unavailable())
    }

    /// Atomically replaces one RMS record after durable commit.
    fn rms_set(&mut self, _handle: u64, _record_id: i32, _data: &[u8]) -> Result<(), EmuError> {
        Err(rms_unavailable())
    }

    /// Atomically deletes one RMS record after durable commit.
    fn rms_delete(&mut self, _handle: u64, _record_id: i32) -> Result<(), EmuError> {
        Err(rms_unavailable())
    }

    /// Creates an MMAPI player over encoded media bytes.
    fn mmapi_create_bytes(&mut self, _content_type: &str, _data: &[u8]) -> Result<u64, EmuError> {
        Err(mmapi_unavailable())
    }

    /// Creates the standard `device://tone` player.
    fn mmapi_create_tone(&mut self) -> Result<u64, EmuError> {
        Err(mmapi_unavailable())
    }

    /// Creates the standard `device://midi` player.
    fn mmapi_create_midi(&mut self) -> Result<u64, EmuError> {
        Err(mmapi_unavailable())
    }

    /// Releases players no longer owned by Java wrappers after a heap collection.
    fn mmapi_retain_handles(&mut self, _handles: &[u64]) {}

    /// Returns the numeric state of one MMAPI player.
    fn mmapi_state(&mut self, _handle: u64, _now_micros: i64) -> Result<i32, EmuError> {
        Err(mmapi_unavailable())
    }

    /// Applies a lifecycle transition selected by the MMAPI native adapter.
    fn mmapi_transition(
        &mut self,
        _handle: u64,
        _transition: i32,
        _now_micros: i64,
    ) -> Result<(), EmuError> {
        Err(mmapi_unavailable())
    }

    /// Starts a player, replacing the previous clip in its compatibility group
    /// (for example Samsung `AudioClip`). Defaults to an ordinary start.
    fn mmapi_start_exclusive(&mut self, handle: u64, now_micros: i64) -> Result<(), EmuError> {
        self.mmapi_transition(handle, 2, now_micros)
    }

    /// Returns the canonical content type of a realized player.
    fn mmapi_content_type(&mut self, _handle: u64) -> Result<String, EmuError> {
        Err(mmapi_unavailable())
    }

    /// Returns duration, media time, or last event data by selector.
    fn mmapi_time(
        &mut self,
        _handle: u64,
        _selector: i32,
        _now_micros: i64,
    ) -> Result<i64, EmuError> {
        Err(mmapi_unavailable())
    }

    fn mmapi_set_media_time(
        &mut self,
        _handle: u64,
        _time: i64,
        _now_micros: i64,
    ) -> Result<i64, EmuError> {
        Err(mmapi_unavailable())
    }

    fn mmapi_set_loop_count(&mut self, _handle: u64, _count: i32) -> Result<(), EmuError> {
        Err(mmapi_unavailable())
    }

    fn mmapi_volume(&mut self, _handle: u64) -> Result<i32, EmuError> {
        Err(mmapi_unavailable())
    }

    fn mmapi_set_volume(&mut self, _handle: u64, _volume: i32) -> Result<i32, EmuError> {
        Err(mmapi_unavailable())
    }

    fn mmapi_muted(&mut self, _handle: u64) -> Result<bool, EmuError> {
        Err(mmapi_unavailable())
    }

    fn mmapi_set_muted(&mut self, _handle: u64, _muted: bool) -> Result<(), EmuError> {
        Err(mmapi_unavailable())
    }

    fn mmapi_set_tone_sequence(&mut self, _handle: u64, _sequence: &[u8]) -> Result<(), EmuError> {
        Err(mmapi_unavailable())
    }

    /// Pops one host event code after advancing playback to current time.
    fn mmapi_next_event(&mut self, _handle: u64, _now_micros: i64) -> Result<i32, EmuError> {
        Err(mmapi_unavailable())
    }

    /// Starts a nonblocking one-shot tone on the suite mixer.
    fn mmapi_play_tone(
        &mut self,
        _note: i32,
        _duration_millis: i32,
        _volume: i32,
        _now_micros: i64,
    ) -> Result<(), EmuError> {
        Err(mmapi_unavailable())
    }

    /// Advances the shared mixer to the current host time.
    ///
    /// Contexts without a media backend treat scheduler maintenance as a
    /// no-op. Media entry points still report `mmapi-unavailable` themselves.
    fn mmapi_pump(&mut self, _now_micros: i64) -> Result<(), EmuError> {
        Ok(())
    }

    /// Returns one application property from the active `MIDlet` suite.
    fn midlet_property(&self, _name: &str) -> Option<&str> {
        None
    }

    /// Records a lifecycle notification emitted by the active `MIDlet`.
    fn midlet_lifecycle_event(&mut self, _event: MidletLifecycleEvent) -> Result<(), EmuError> {
        Err(native_error(
            "native-context",
            "MIDlet lifecycle is unavailable in this context",
        ))
    }

    /// Applies the host policy to a platform request and reports whether the
    /// suite must exit before the request can be completed.
    /// An empty URL cancels pending requests; the default host has none.
    fn platform_request(&mut self, url: &str) -> Result<bool, EmuError> {
        if url.is_empty() {
            return Ok(false);
        }
        Err(native_error(
            "native-context",
            "platform requests are unavailable in this context",
        ))
    }

    /// Returns the MIDP permission status: 1 allowed, 0 denied, -1 unknown.
    /// Queries do not prompt; return -1 when an operation would need user approval.
    fn check_permission(&mut self, _permission: &str) -> Result<i32, EmuError> {
        Ok(0)
    }

    /// Reserves a presentation slot; the VM parks only the presenting Java worker.
    /// Do not repeat the wait in `present_lcdui_frame` or reserve another slot
    /// for an unchanged framebuffer, which loading loops may repeatedly flush.
    fn lcdui_frame_delay_millis(&mut self, _changed: bool) -> u64 {
        0
    }

    /// Publishes one complete ARGB8888 LCDUI frame to the host backend.
    ///
    /// # Errors
    /// Returns a controlled diagnostic when the frame shape is invalid or the
    /// active host does not provide an LCDUI sink.
    fn present_lcdui_frame(
        &mut self,
        _width: u32,
        _height: u32,
        _pixels: &[u32],
    ) -> Result<(), EmuError> {
        Err(native_error(
            "native-context",
            "LCDUI framebuffer presentation is unavailable in this context",
        ))
    }

    /// Full-screen Canvas dimensions of the active device profile.
    #[must_use]
    fn lcd_dimensions(&self) -> (u32, u32) {
        (240, 320)
    }

    /// Drawable dimensions exposed by a Canvas before full-screen mode is
    /// requested. Hosts without separate system chrome use the LCD size.
    #[must_use]
    fn lcd_non_fullscreen_dimensions(&self) -> (u32, u32) {
        self.lcd_dimensions()
    }

    /// Optional profile-selected LCDUI line height for a MIDP font-size
    /// constant. Frontends without measured device metrics keep the bootstrap
    /// defaults by returning `None`.
    #[must_use]
    fn lcd_ui_font_height(&self, _size: i32) -> Option<i32> {
        None
    }

    /// Resolves a device key code to its MIDP game action.
    #[must_use]
    fn canvas_game_action(&self, _key_code: i32) -> Option<i32> {
        None
    }

    /// Resolves a MIDP game action to the profile's preferred device key code.
    #[must_use]
    fn canvas_key_code(&self, _game_action: i32) -> Option<i32> {
        None
    }

    /// Returns the profile's stable name for a device key code.
    #[must_use]
    fn canvas_key_name(&self, _key_code: i32) -> Option<&str> {
        None
    }

    /// Writes one Java console diagnostic line through the host bridge.
    fn write_console_error(&mut self, _line: &str) -> Result<(), EmuError> {
        Err(native_error(
            "native-context",
            "console error output is unavailable in this context",
        ))
    }

    /// Writes Java `System.out` text through the host diagnostic bridge.
    fn write_console_output(&mut self, _text: &str, _newline: bool) -> Result<(), EmuError> {
        Err(native_error(
            "native-context",
            "console output is unavailable in this context",
        ))
    }
}

fn file_connection_unavailable() -> EmuError {
    native_error(
        "gcf-unavailable",
        "FileConnection is unavailable in this context",
    )
}

fn rms_unavailable() -> EmuError {
    native_error("rms-unavailable", "RMS is unavailable in this context")
}

fn mmapi_unavailable() -> EmuError {
    native_error("mmapi-unavailable", "MMAPI is unavailable in this context")
}
