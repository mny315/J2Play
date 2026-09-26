use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{Attribute, ClassFile, ExceptionHandler};
use diagnostics::EmuError;
use mmapi::PlayerEventKind::{Closed, Started, Stopped};
use natives::{NativeRegistry, NativeSignature, NativeValue};

fn sound_fixture() -> ClassFile {
    const SOUND: &str = "com/nokia/mid/sound/Sound";
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/NokiaSoundLifecycle");
    let super_class = pool.class("java/lang/Object");
    let sound = pool.class(SOUND);
    let illegal_argument = pool.class("java/lang/IllegalArgumentException");
    let code_name = pool.utf8("Code");
    let constructor = pool.method(SOUND, "<init>", "(IJ)V");
    let init = pool.method(SOUND, "init", "(IJ)V");
    let state = pool.method(SOUND, "getState", "()I");
    let play = pool.method(SOUND, "play", "(I)V");
    let stop = pool.method(SOUND, "stop", "()V");
    let resume = pool.method(SOUND, "resume", "()V");
    let release = pool.method(SOUND, "release", "()V");
    let set_gain = pool.method(SOUND, "setGain", "(I)V");
    let get_gain = pool.method(SOUND, "getGain", "()I");
    let mut code = Vec::new();
    emit_reference(&mut code, 0xbb, sound);
    code.extend([0x59, 0x11, 1, 0xb8, 0x11, 0, 200, 0x85]); // new Sound(440, 200L)
    emit_reference(&mut code, 0xb7, constructor);
    code.push(0x4b);
    expect_value(&mut code, state, 1);
    code.push(0x2a);
    emit_reference(&mut code, 0xb6, resume);
    expect_value(&mut code, state, 1); // resume before the first play does nothing
    for (gain, expected) in [(-100_i16, 0_i16), (1, 1), (300, 255)] {
        code.extend([0x2a, 0x11]);
        code.extend(gain.to_be_bytes());
        emit_reference(&mut code, 0xb6, set_gain);
        expect_value(&mut code, get_gain, expected);
    }
    for loops in [1, 2] {
        code.extend([0x2a, 0x10, loops]);
        emit_reference(&mut code, 0xb6, play);
        expect_value(&mut code, state, 0);
    }
    code.push(0x2a);
    emit_reference(&mut code, 0xb6, release);
    expect_value(&mut code, state, 3);
    for method in [stop, resume, release] {
        code.push(0x2a);
        emit_reference(&mut code, 0xb6, method);
    }
    code.extend([0x2a, 0x04]);
    emit_reference(&mut code, 0xb6, play);
    expect_value(&mut code, state, 3);
    code.extend([0x2a, 0x04]);
    emit_reference(&mut code, 0xb6, set_gain);
    expect_value(&mut code, get_gain, 1);
    code.extend([0x2a, 0x11, 1, 0xb8, 0x11, 0, 200, 0x85]);
    emit_reference(&mut code, 0xb6, init);
    expect_value(&mut code, state, 1);
    // A failed reinitialization must leave the Sound released and queryable.
    let start_pc = u16::try_from(code.len()).unwrap();
    code.extend([0x2a, 0x11, 1, 0xb8, 0x02, 0x85]);
    emit_reference(&mut code, 0xb6, init);
    let end_pc = u16::try_from(code.len()).unwrap();
    code.extend([0x02, 0xac]); // missing exception
    let handler_pc = u16::try_from(code.len()).unwrap();
    code.push(0x57);
    expect_value(&mut code, state, 3);
    code.extend([0x04, 0xac]);
    let mut run = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 9,
            name: "run",
            descriptor: "()I",
            max_stack: 5,
            max_locals: 1,
            code,
        },
    );
    let Attribute::Code(body) = &mut run.attributes[0] else {
        unreachable!()
    };
    body.exception_table.push(ExceptionHandler {
        start_pc,
        end_pc,
        handler_pc,
        catch_type: illegal_argument,
    });
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x21,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![run],
        attributes: Vec::new(),
    }
}

fn expect_value(code: &mut Vec<u8>, getter: u16, expected: i16) {
    code.push(0x2a);
    emit_reference(code, 0xb6, getter);
    code.push(0x11);
    code.extend(expected.to_be_bytes());
    code.extend([0x9f, 0, 5, 0x02, 0xac]); // return -1 on mismatch
}

struct Host(mmapi::Runtime<mmapi::NullAudioSink>, i64);

impl natives::VmAccess for Host {}

impl natives::HostServices for Host {
    fn monotonic_millis(&self) -> i64 {
        self.1
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _name: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _name: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
    fn mmapi_create_bytes(&mut self, content_type: &str, data: &[u8]) -> Result<u64, EmuError> {
        self.0.create_from_bytes(content_type, data)
    }
    fn mmapi_state(&mut self, handle: u64, now: i64) -> Result<i32, EmuError> {
        self.0.state(handle, now).map(|state| state as i32)
    }
    fn mmapi_transition(&mut self, handle: u64, transition: i32, now: i64) -> Result<(), EmuError> {
        self.0.transition(handle, transition, now)
    }
    fn mmapi_set_media_time(
        &mut self,
        handle: u64,
        position: i64,
        now: i64,
    ) -> Result<i64, EmuError> {
        self.0.set_media_time(handle, position, now)
    }
    fn mmapi_set_loop_count(&mut self, handle: u64, count: i32) -> Result<(), EmuError> {
        self.0.set_loop_count(handle, count)
    }
    fn mmapi_set_volume(&mut self, handle: u64, volume: i32) -> Result<i32, EmuError> {
        self.0.set_volume(handle, volume)
    }
    fn mmapi_content_type(&mut self, handle: u64) -> Result<String, EmuError> {
        self.0.content_type(handle).map(str::to_owned)
    }
}

#[test]
fn nokia_sound_restarts_clamps_gain_and_releases_failed_reinitialization() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "nokia_sound_restarts_clamps_gain_and_releases_failed_reinitialization"
    )) {
        return;
    }
    let limits = vm::Limits::default();
    let mut program = vm::Program::new();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    program.add_class(&sound_fixture(), &limits).unwrap();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    mmapi::register_natives(program.native_registry_mut()).unwrap();
    let mut host = Host(
        mmapi::Runtime::new(mmapi::NullAudioSink::default(), mmapi::Limits::default()),
        0,
    );
    let execution = program
        .execute_with_context(
            "fixtures/NokiaSoundLifecycle",
            "run",
            "()I",
            limits,
            false,
            &mut host,
        )
        .unwrap();
    assert_eq!(execution.value, Some(vm::Value::Int(1)));
    assert_eq!(execution.thread_failure_count, 0);
    let mut events = Vec::new();
    while let Some(event) = host.0.next_event(1, 0).unwrap() {
        events.push(event.kind);
    }
    assert_eq!(events, [Started, Stopped, Started, Closed]);
}

#[test]
fn nokia_resume_restarts_a_stopped_tone_and_preserves_an_active_cursor() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "nokia_resume_restarts_a_stopped_tone_and_preserves_an_active_cursor"
    )) {
        return;
    }
    let mut registry = NativeRegistry::new();
    mmapi::register_natives(&mut registry).unwrap();
    let mut host = Host(
        mmapi::Runtime::new(mmapi::NullAudioSink::default(), mmapi::Limits::default()),
        0,
    );
    let handle = host
        .0
        .create_from_bytes("audio/x-tone-seq", &[254, 1, 69, 64])
        .unwrap();
    let reference = [NativeValue::Long(i64::try_from(handle).unwrap())];
    let play = NativeSignature::new("com/nokia/mid/sound/Sound", "play0", "(JII)V");
    let resume = NativeSignature::new("com/nokia/mid/sound/Sound", "resume0", "(J)V");
    let stop = NativeSignature::new("com/nokia/mid/sound/Sound", "stop0", "(J)V");
    registry
        .invoke(
            &play,
            &mut host,
            &[
                reference[0].clone(),
                NativeValue::Int(1),
                NativeValue::Int(1),
            ],
        )
        .unwrap();
    assert_eq!(host.0.volume(handle).unwrap(), 1);
    host.1 = 20;
    registry.invoke(&resume, &mut host, &reference).unwrap();
    assert_eq!(host.0.media_time(handle, 20_000).unwrap(), 20_000);
    registry.invoke(&stop, &mut host, &reference).unwrap();
    host.1 = 40;
    registry.invoke(&resume, &mut host, &reference).unwrap();
    assert_eq!(host.0.media_time(handle, 40_000).unwrap(), 0);
    assert_eq!(
        host.0.state(handle, 40_000).unwrap(),
        mmapi::PlayerState::Started
    );
}
