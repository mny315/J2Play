use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{ClassFile, Constant};
use diagnostics::EmuError;

#[test]
fn single_tone_clamps_volume_and_validates_note_and_duration() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::single_tone_clamps_volume_and_validates_note_and_duration"
    )) {
        return;
    }
    for namespace in ["javax/microedition/media", "com/siemens/mp/media"] {
        let muted = run(namespace, 69, 1, 0).unwrap();
        let loud = run(namespace, 69, 1, 100).unwrap();
        assert_eq!(muted.len(), 22);
        assert!(muted.iter().all(|sample| *sample == 0));
        assert!(loud.iter().any(|sample| *sample != 0));
        for (volume, expected) in [
            (i32::MIN, &muted),
            (-1, &muted),
            (101, &loud),
            (i32::MAX, &loud),
        ] {
            assert_eq!(run(namespace, 69, 1, volume).unwrap(), *expected);
        }
        for (note, duration) in [(-1, 1), (128, 1), (69, 0), (69, -1)] {
            let error = run(namespace, note, duration, 50).unwrap_err();
            assert_eq!(error.code(), "uncaught-exception");
            assert!(
                error
                    .message()
                    .contains("java/lang/IllegalArgumentException")
            );
        }
    }
}

fn run(namespace: &str, note: i32, duration: i32, volume: i32) -> Result<Vec<i16>, EmuError> {
    let limits = vm::Limits::default();
    let mut program = vm::Program::new();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    program
        .add_class(&fixture(namespace, note, duration, volume), &limits)
        .unwrap();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    mmapi::register_natives(program.native_registry_mut()).unwrap();
    let mut host = Host(mmapi::Runtime::new(
        Sink::default(),
        mmapi::Limits::default(),
    ));
    let result = program.execute_with_context(
        "fixtures/SingleTone",
        "run",
        "()V",
        limits,
        false,
        &mut host,
    )?;
    assert_eq!(result.thread_failure_count, 0);
    Ok(host.0.sink().0.clone())
}

fn fixture(namespace: &str, note: i32, duration: i32, volume: i32) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/SingleTone");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let play = pool.method(&format!("{namespace}/Manager"), "playTone", "(III)V");
    let mut code = Vec::new();
    for value in [note, duration, volume] {
        let index = pool.push(Constant::Integer(value));
        emit_reference(&mut code, 0x13, index);
    }
    emit_reference(&mut code, 0xb8, play);
    code.push(0xb1);
    let method = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 9,
            name: "run",
            descriptor: "()V",
            max_stack: 3,
            max_locals: 0,
            code,
        },
    );
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x21,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![method],
        attributes: Vec::new(),
    }
}

#[derive(Default)]
struct Sink(Vec<i16>);

impl mmapi::AudioSink for Sink {
    fn write(&mut self, samples: &[i16]) -> Result<(), EmuError> {
        assert!(self.0.len() + samples.len() <= 22);
        self.0.extend_from_slice(samples);
        Ok(())
    }
}

struct Host(mmapi::Runtime<Sink>);

impl natives::HostServices for Host {
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

    fn mmapi_play_tone(
        &mut self,
        note: i32,
        duration: i32,
        volume: i32,
        now: i64,
    ) -> Result<(), EmuError> {
        self.0.play_tone(note, duration, volume, now)?;
        self.0.tick(now + 1_000)
    }
    fn mmapi_pump(&mut self, now: i64) -> Result<(), EmuError> {
        self.0.tick(now.max(1_000))
    }
}
