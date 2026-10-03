use super::*;

#[derive(Default)]
pub(crate) struct ManagedHeapNoticeContext {
    pub(crate) notices: Vec<ManagedHeapLimitNotice>,
    pub(crate) decision: ManagedHeapLimitDecision,
    pub(crate) uncaught_heap_limits: Vec<bool>,
}

impl HostServices for ManagedHeapNoticeContext {
    fn uncaught_thread_exception(
        &mut self,
        _: u64,
        _: &str,
        _: Option<&str>,
        _: &[String],
        managed_heap_limit: bool,
    ) {
        self.uncaught_heap_limits.push(managed_heap_limit);
    }

    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn repeated_managed_heap_limit(
        &mut self,
        notice: ManagedHeapLimitNotice,
    ) -> ManagedHeapLimitDecision {
        self.notices.push(notice);
        self.decision
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

pub(crate) fn program_with_exception(class: &str) -> Program {
    let mut program = Program::new();
    program.classes.insert(
        "java/lang/Throwable".to_owned(),
        test_class_definition(Some("java/lang/Object")),
    );
    program.classes.insert(
        class.to_owned(),
        test_class_definition(Some("java/lang/Throwable")),
    );
    program
}

pub(crate) fn program_with_interrupted_exception() -> Program {
    program_with_exception("java/lang/InterruptedException")
}

pub(crate) fn caught_managed_heap_retry_program() -> Program {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("java/lang/OutOfMemoryError".into())),
    ];
    // Keep retrying an allocation from the same catch handler. The normal
    // path also loops so this fixture remains valid with larger test heaps.
    let mut main = runtime_method(
        "T",
        "main",
        "()I",
        &[
            0x11, 0x27, 0x10, 0xbc, 8, 0x57, 0xa7, 0xff, 0xfa, 0x57, 0xa7, 0xff, 0xf6,
        ],
        1,
        0,
        constants,
        true,
    );
    Arc::make_mut(&mut main.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 6,
        handler_pc: 9,
        catch_type: 1,
    });
    let mut program = program_with_exception("java/lang/OutOfMemoryError");
    program.methods.insert(main.key.clone(), main);
    program
}

pub(crate) struct BoundedMmapiContext {
    pub(crate) live_handles: Vec<u64>,
    next_handle: u64,
    pub(crate) allocation_calls: usize,
    pub(crate) transition_calls: usize,
    pub(crate) tone_calls: usize,
    pub(crate) transition_error: &'static str,
    pub(crate) retain_calls: Vec<Vec<u64>>,
}

impl BoundedMmapiContext {
    pub(crate) fn with_live_handle(handle: u64) -> Self {
        Self {
            live_handles: vec![handle],
            next_handle: handle.saturating_add(1),
            allocation_calls: 0,
            transition_calls: 0,
            tone_calls: 0,
            transition_error: "media-work-limit",
            retain_calls: Vec::new(),
        }
    }
}

impl HostServices for BoundedMmapiContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }

    fn mmapi_create_bytes(&mut self, _: &str, _: &[u8]) -> Result<u64, EmuError> {
        self.allocation_calls = self.allocation_calls.saturating_add(1);
        if !self.live_handles.is_empty() {
            return Err(EmuError::new(
                Category::Api,
                "player-limit",
                "test MMAPI player limit reached",
            ));
        }
        let handle = self.next_handle;
        self.next_handle = self.next_handle.saturating_add(1);
        self.live_handles.push(handle);
        Ok(handle)
    }

    fn mmapi_retain_handles(&mut self, handles: &[u64]) {
        self.live_handles.retain(|handle| handles.contains(handle));
        self.retain_calls.push(handles.to_vec());
    }

    fn mmapi_play_tone(&mut self, _: i32, _: i32, _: i32, _: i64) -> Result<(), EmuError> {
        self.tone_calls += 1;
        if self.live_handles.is_empty() {
            Ok(())
        } else {
            Err(EmuError::new(
                Category::Api,
                "media-limit",
                "test PCM limit reached",
            ))
        }
    }

    fn mmapi_transition(&mut self, _: u64, _: i32, _: i64) -> Result<(), EmuError> {
        self.transition_calls += 1;
        Err(EmuError::new(
            Category::Api,
            self.transition_error,
            "test synthesis work limit reached",
        ))
    }
}

#[derive(Default)]
pub(crate) struct PacingContext {
    pub(crate) now_millis: i64,
    pub(crate) paced_millis: u64,
    pub(crate) realtime: bool,
    pub(crate) instructions_per_second: Option<u64>,
    pub(crate) yield_25_millis: bool,
    pub(crate) random_seed: Option<i64>,
}

impl HostServices for PacingContext {
    fn monotonic_millis(&self) -> i64 {
        self.now_millis
    }

    fn wall_clock_millis(&self) -> i64 {
        self.now_millis
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn realtime_pacing(&self) -> bool {
        self.realtime
    }

    fn realtime_interpreter_instructions_per_second(&self) -> Option<u64> {
        self.instructions_per_second
    }

    fn pace_millis(&mut self, millis: u64) -> Result<(), EmuError> {
        self.paced_millis = self.paced_millis.saturating_add(millis);
        self.now_millis = self
            .now_millis
            .saturating_add(i64::try_from(millis).unwrap_or(i64::MAX));
        Ok(())
    }

    fn realtime_thread_sleep_millis(&self, requested: u64) -> u64 {
        if self.yield_25_millis && requested == 25 {
            0
        } else {
            requested
        }
    }

    fn random_seed_override(&self) -> Option<i64> {
        self.random_seed
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }

    fn present_lcdui_frame(
        &mut self,
        _width: u32,
        _height: u32,
        _pixels: &[u32],
    ) -> Result<(), EmuError> {
        Ok(())
    }
}
