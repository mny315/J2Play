//! Host service forwarding with virtual clocks and media pressure recovery.

use super::super::{
    EmuError, HostServices, MachineNativeContext, ManagedHeapLimitDecision, ManagedHeapLimitNotice,
    VibrationRequest, VmTelemetry,
};
use natives::MidletLifecycleEvent;

impl HostServices for MachineNativeContext<'_> {
    fn monotonic_millis(&self) -> i64 {
        self.host
            .monotonic_millis()
            .saturating_add(self.scheduler.virtual_monotonic_millis)
    }
    fn wall_clock_millis(&self) -> i64 {
        self.host
            .wall_clock_millis()
            .saturating_add(self.scheduler.virtual_wall_millis)
    }
    fn system_property(&self, name: &str) -> Option<&str> {
        self.host.system_property(name)
    }
    fn bluetooth_property(&self, name: &str) -> Option<&str> {
        self.host.bluetooth_property(name)
    }
    fn display_colors(&self) -> i32 {
        self.host.display_colors()
    }
    fn canvas_pointer_events(&self) -> bool {
        self.host.canvas_pointer_events()
    }
    fn canvas_pointer_motion_events(&self) -> bool {
        self.host.canvas_pointer_motion_events()
    }
    fn set_text_input_active(&mut self, active: bool) {
        self.host.set_text_input_active(active);
    }
    fn request_vibration(&mut self, request: VibrationRequest) -> bool {
        self.host.request_vibration(request)
    }
    fn vm_flight_recorder_enabled(&self) -> bool {
        self.host.vm_flight_recorder_enabled()
    }
    fn record_vm_flight(&mut self, event: String) {
        self.host.record_vm_flight(event);
    }
    fn uncaught_thread_exception(
        &mut self,
        thread_id: u64,
        exception_class: &str,
        exception_message: Option<&str>,
        stack_trace: &[String],
        managed_heap_limit: bool,
    ) {
        self.host.uncaught_thread_exception(
            thread_id,
            exception_class,
            exception_message,
            stack_trace,
            managed_heap_limit,
        );
    }
    fn repeated_managed_heap_limit(
        &mut self,
        notice: ManagedHeapLimitNotice,
    ) -> ManagedHeapLimitDecision {
        self.host.repeated_managed_heap_limit(notice)
    }
    fn realtime_pacing(&self) -> bool {
        self.host.realtime_pacing()
    }
    fn realtime_interpreter_instructions_per_second(&self) -> Option<u64> {
        self.host.realtime_interpreter_instructions_per_second()
    }
    fn pace_lcdui_frame_request(&mut self) -> Result<(), EmuError> {
        self.host.pace_lcdui_frame_request()
    }
    fn pace_millis(&mut self, millis: u64) -> Result<(), EmuError> {
        self.host.pace_millis(millis)
    }
    fn realtime_thread_sleep_millis(&self, requested: u64) -> u64 {
        self.host.realtime_thread_sleep_millis(requested)
    }
    fn random_seed_override(&self) -> Option<i64> {
        self.host.random_seed_override()
    }
    fn execution_cancelled(&self) -> bool {
        self.host.execution_cancelled()
    }
    fn execution_suspended(&self) -> bool {
        self.host.execution_suspended()
    }
    fn update_vm_telemetry(&mut self, telemetry: VmTelemetry) {
        self.host.update_vm_telemetry(telemetry);
    }
    fn read_resource(&self, name: &str) -> Result<Option<Vec<u8>>, EmuError> {
        self.host.read_resource(name)
    }
    fn gcf_http(
        &mut self,
        request: natives::GcfHttpRequest,
    ) -> Result<natives::GcfHttpResponse, EmuError> {
        self.host.gcf_http(request)
    }
    fn gcf_file_metadata(&mut self, url: &str) -> Result<natives::GcfFileMetadata, EmuError> {
        self.host.gcf_file_metadata(url)
    }
    fn gcf_file_read(&mut self, url: &str) -> Result<Vec<u8>, EmuError> {
        self.host.gcf_file_read(url)
    }
    fn gcf_file_revision(&self) -> Result<u64, EmuError> {
        self.host.gcf_file_revision()
    }
    fn gcf_file_write(&mut self, url: &str, data: &[u8], append: bool) -> Result<(), EmuError> {
        self.host.gcf_file_write(url, data, append)
    }
    fn gcf_file_output_offset(&mut self, url: &str, offset: u64) -> Result<u64, EmuError> {
        self.host.gcf_file_output_offset(url, offset)
    }
    fn gcf_file_write_at(&mut self, url: &str, data: &[u8], offset: u64) -> Result<(), EmuError> {
        self.host.gcf_file_write_at(url, data, offset)
    }
    fn gcf_file_create(&mut self, url: &str) -> Result<(), EmuError> {
        self.host.gcf_file_create(url)
    }
    fn gcf_file_mkdir(&mut self, url: &str) -> Result<(), EmuError> {
        self.host.gcf_file_mkdir(url)
    }
    fn gcf_file_delete(&mut self, url: &str) -> Result<(), EmuError> {
        self.host.gcf_file_delete(url)
    }
    fn gcf_file_rename(&mut self, url: &str, name: &str) -> Result<String, EmuError> {
        self.host.gcf_file_rename(url, name)
    }
    fn gcf_file_truncate(&mut self, url: &str, size: u64) -> Result<(), EmuError> {
        self.host.gcf_file_truncate(url, size)
    }
    fn gcf_file_list(&mut self, url: &str) -> Result<Vec<String>, EmuError> {
        self.host.gcf_file_list(url)
    }
    fn gcf_file_directory_size(&mut self, url: &str, recursive: bool) -> Result<u64, EmuError> {
        self.host.gcf_file_directory_size(url, recursive)
    }
    fn gcf_file_space(&mut self, selector: i32) -> Result<u64, EmuError> {
        self.host.gcf_file_space(selector)
    }
    fn rms_open(&mut self, name: &str, create: bool) -> Result<u64, EmuError> {
        self.host.rms_open(name, create)
    }
    fn rms_open_owned(&mut self, name: &str, vendor: &str, suite: &str) -> Result<u64, EmuError> {
        self.host.rms_open_owned(name, vendor, suite)
    }
    fn rms_close(&mut self, handle: u64) -> Result<(), EmuError> {
        self.host.rms_close(handle)
    }
    fn rms_delete_store(&mut self, name: &str) -> Result<(), EmuError> {
        self.host.rms_delete_store(name)
    }
    fn rms_list_stores(&mut self) -> Result<Vec<String>, EmuError> {
        self.host.rms_list_stores()
    }
    fn rms_metadata(
        &mut self,
        handle: u64,
        field: natives::RmsMetadataField,
    ) -> Result<i64, EmuError> {
        self.host.rms_metadata(handle, field)
    }
    fn rms_record_ids(&mut self, handle: u64) -> Result<Vec<i32>, EmuError> {
        self.host.rms_record_ids(handle)
    }
    fn rms_get(&mut self, handle: u64, record_id: i32) -> Result<Vec<u8>, EmuError> {
        self.host.rms_get(handle, record_id)
    }
    fn rms_record_size(&mut self, handle: u64, record_id: i32) -> Result<usize, EmuError> {
        self.host.rms_record_size(handle, record_id)
    }
    fn rms_add(&mut self, handle: u64, data: &[u8]) -> Result<i32, EmuError> {
        self.host.rms_add(handle, data)
    }
    fn rms_set(&mut self, handle: u64, record_id: i32, data: &[u8]) -> Result<(), EmuError> {
        self.host.rms_set(handle, record_id, data)
    }
    fn rms_delete(&mut self, handle: u64, record_id: i32) -> Result<(), EmuError> {
        self.host.rms_delete(handle, record_id)
    }
    fn mmapi_create_bytes(&mut self, content_type: &str, data: &[u8]) -> Result<u64, EmuError> {
        self.retry_mmapi_resource(|host| host.mmapi_create_bytes(content_type, data))
    }
    fn mmapi_create_tone(&mut self) -> Result<u64, EmuError> {
        self.retry_mmapi_resource(|host| host.mmapi_create_tone())
    }
    fn mmapi_create_midi(&mut self) -> Result<u64, EmuError> {
        self.retry_mmapi_resource(|host| host.mmapi_create_midi())
    }
    fn mmapi_retain_handles(&mut self, handles: &[u64]) {
        self.host.mmapi_retain_handles(handles);
    }
    fn mmapi_state(&mut self, handle: u64, now_micros: i64) -> Result<i32, EmuError> {
        self.host.mmapi_state(handle, now_micros)
    }
    fn mmapi_transition(
        &mut self,
        handle: u64,
        transition: i32,
        now_micros: i64,
    ) -> Result<(), EmuError> {
        self.retry_mmapi_resource(|host| host.mmapi_transition(handle, transition, now_micros))
    }
    fn mmapi_start_exclusive(&mut self, handle: u64, now_micros: i64) -> Result<(), EmuError> {
        self.retry_mmapi_resource(|host| host.mmapi_start_exclusive(handle, now_micros))
    }
    fn mmapi_content_type(&mut self, handle: u64) -> Result<String, EmuError> {
        self.host.mmapi_content_type(handle)
    }
    fn mmapi_time(&mut self, handle: u64, selector: i32, now_micros: i64) -> Result<i64, EmuError> {
        self.host.mmapi_time(handle, selector, now_micros)
    }
    fn mmapi_set_media_time(
        &mut self,
        handle: u64,
        time: i64,
        now_micros: i64,
    ) -> Result<i64, EmuError> {
        self.host.mmapi_set_media_time(handle, time, now_micros)
    }
    fn mmapi_set_loop_count(&mut self, handle: u64, count: i32) -> Result<(), EmuError> {
        self.host.mmapi_set_loop_count(handle, count)
    }
    fn mmapi_volume(&mut self, handle: u64) -> Result<i32, EmuError> {
        self.host.mmapi_volume(handle)
    }
    fn mmapi_set_volume(&mut self, handle: u64, volume: i32) -> Result<i32, EmuError> {
        self.host.mmapi_set_volume(handle, volume)
    }
    fn mmapi_muted(&mut self, handle: u64) -> Result<bool, EmuError> {
        self.host.mmapi_muted(handle)
    }
    fn mmapi_set_muted(&mut self, handle: u64, muted: bool) -> Result<(), EmuError> {
        self.host.mmapi_set_muted(handle, muted)
    }
    fn mmapi_set_tone_sequence(&mut self, handle: u64, sequence: &[u8]) -> Result<(), EmuError> {
        self.retry_mmapi_resource(|host| host.mmapi_set_tone_sequence(handle, sequence))
    }
    fn mmapi_next_event(&mut self, handle: u64, now_micros: i64) -> Result<i32, EmuError> {
        self.host.mmapi_next_event(handle, now_micros)
    }
    fn mmapi_play_tone(
        &mut self,
        note: i32,
        duration_millis: i32,
        volume: i32,
        now_micros: i64,
    ) -> Result<(), EmuError> {
        self.retry_mmapi_resource(|host| {
            host.mmapi_play_tone(note, duration_millis, volume, now_micros)
        })
    }
    fn mmapi_pump(&mut self, now_micros: i64) -> Result<(), EmuError> {
        self.host.mmapi_pump(now_micros)
    }
    fn midlet_property(&self, name: &str) -> Option<&str> {
        self.host.midlet_property(name)
    }
    fn midlet_lifecycle_event(&mut self, event: MidletLifecycleEvent) -> Result<(), EmuError> {
        self.host.midlet_lifecycle_event(event)
    }
    fn platform_request(&mut self, url: &str) -> Result<bool, EmuError> {
        self.host.platform_request(url)
    }
    fn check_permission(&mut self, permission: &str) -> Result<i32, EmuError> {
        self.host.check_permission(permission)
    }
    fn lcdui_frame_delay_millis(&mut self, changed: bool) -> u64 {
        self.host.lcdui_frame_delay_millis(changed)
    }

    fn present_lcdui_frame(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u32],
    ) -> Result<(), EmuError> {
        self.host.present_lcdui_frame(width, height, pixels)
    }
    fn lcd_dimensions(&self) -> (u32, u32) {
        self.host.lcd_dimensions()
    }
    fn lcd_non_fullscreen_dimensions(&self) -> (u32, u32) {
        self.host.lcd_non_fullscreen_dimensions()
    }
    fn lcd_ui_font_height(&self, size: i32) -> Option<i32> {
        self.host.lcd_ui_font_height(size)
    }
    fn canvas_game_action(&self, key_code: i32) -> Option<i32> {
        self.host.canvas_game_action(key_code)
    }
    fn canvas_key_code(&self, game_action: i32) -> Option<i32> {
        self.host.canvas_key_code(game_action)
    }
    fn canvas_key_name(&self, key_code: i32) -> Option<&str> {
        self.host.canvas_key_name(key_code)
    }
    fn write_console_error(&mut self, line: &str) -> Result<(), EmuError> {
        self.host.write_console_error(line)
    }
    fn write_console_output(&mut self, text: &str, newline: bool) -> Result<(), EmuError> {
        self.host.write_console_output(text, newline)
    }
}
