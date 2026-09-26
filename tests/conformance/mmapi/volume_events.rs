use classfile::{Attribute, ClassFile, CodeAttribute, Constant, Member};
use diagnostics::EmuError;

#[derive(Default)]
struct Fixture {
    pool: Vec<Option<Constant>>,
    methods: Vec<Member>,
    fields: Vec<Member>,
}

impl Fixture {
    fn constant(&mut self, value: Constant) -> u16 {
        if self.pool.is_empty() {
            self.pool.push(None);
        }
        let index = u16::try_from(self.pool.len()).unwrap();
        self.pool.push(Some(value));
        index
    }
    fn text(&mut self, value: &str) -> u16 {
        self.constant(Constant::Utf8(value.into()))
    }
    fn class(&mut self, name: &str) -> u16 {
        let name_index = self.text(name);
        self.constant(Constant::Class { name_index })
    }
    fn reference(&mut self, owner: &str, name: &str, descriptor: &str, field: bool) -> u16 {
        let class_index = self.class(owner);
        let name_index = self.text(name);
        let descriptor_index = self.text(descriptor);
        let name_and_type_index = self.constant(Constant::NameAndType {
            name_index,
            descriptor_index,
        });
        self.constant(if field {
            Constant::Fieldref {
                class_index,
                name_and_type_index,
            }
        } else {
            Constant::Methodref {
                class_index,
                name_and_type_index,
            }
        })
    }
    fn string(&mut self, text: &str) -> u16 {
        let string_index = self.text(text);
        self.constant(Constant::String { string_index })
    }
    fn field(&mut self, name: &str, descriptor: &str) -> u16 {
        let name_index = self.text(name);
        let descriptor_index = self.text(descriptor);
        self.fields.push(Member {
            access_flags: 9,
            name_index,
            descriptor_index,
            attributes: vec![],
        });
        self.reference("fixtures/VolumeEvents", name, descriptor, true)
    }
    fn method(&mut self, name: &str, descriptor: &str, flags: u16, code: Vec<u8>) {
        let name_index = self.text(name);
        let descriptor_index = self.text(descriptor);
        let code_name = self.text("Code");
        self.methods.push(Member {
            access_flags: flags,
            name_index,
            descriptor_index,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: code_name,
                max_stack: 8,
                max_locals: 5,
                code,
                exception_table: vec![],
                attributes: vec![],
            })],
        });
    }
}

fn reference(code: &mut Vec<u8>, opcode: u8, index: u16) {
    code.push(opcode);
    code.extend(index.to_be_bytes());
}

// This listener fades from zero to full volume using one-unit steps. Each
// callback changes the volume again: synchronous delivery exceeds even the
// normal VM frame limit, while deferred delivery has constant stack depth.
#[allow(clippy::too_many_lines)]
fn fixture(namespace: &str) -> ClassFile {
    let mut f = Fixture::default();
    let this = f.class("fixtures/VolumeEvents");
    let object = f.class("java/lang/Object");
    let player = format!("{namespace}/PlayerImpl");
    let control = format!("{namespace}/VolumeControlImpl");
    let listener = f.class(&format!("{namespace}/PlayerListener"));
    let control_class = f.class(&control);
    let control_field = f.field("control", &format!("L{control};"));
    let count = f.field("count", "I");
    let bad = f.field("bad", "I");
    let init = f.reference("java/lang/Object", "<init>", "()V", false);
    let own_init = f.reference("fixtures/VolumeEvents", "<init>", "()V", false);
    let create = f.reference(
        &player,
        "fromLocator",
        &format!("(Ljava/lang/String;)L{namespace}/Player;"),
        false,
    );
    let get_control = f.reference(
        &player,
        "getControl",
        &format!("(Ljava/lang/String;)L{namespace}/Control;"),
        false,
    );
    let add_listener = f.reference(
        &player,
        "addPlayerListener",
        &format!("(L{namespace}/PlayerListener;)V"),
        false,
    );
    let set_level = f.reference(&control, "setLevel", "(I)I", false);
    let get_level = f.reference(&control, "getLevel", "()I", false);
    let set_mute = f.reference(&control, "setMute", "(Z)V", false);
    let close = f.reference(&player, "close", "()V", false);
    let equals = f.reference("java/lang/String", "equals", "(Ljava/lang/Object;)Z", false);
    let sleep = f.reference("java/lang/Thread", "sleep", "(J)V", false);
    let locator = f.string("device://tone");
    let control_name = f.string("VolumeControl");
    let changed = f.string("volumeChanged");

    let mut code = vec![0x2a];
    reference(&mut code, 0xb7, init);
    code.push(0xb1);
    f.method("<init>", "()V", 1, code);

    let mut callback = vec![0x2c];
    reference(&mut callback, 0x13, changed);
    reference(&mut callback, 0xb6, equals);
    callback.extend([0x9a, 0, 4, 0xb1]); // ignore lifecycle events
    callback.push(0x2d);
    reference(&mut callback, 0xb2, control_field);
    callback.extend([0xa5, 0, 7, 0x04]);
    reference(&mut callback, 0xb3, bad);
    reference(&mut callback, 0xb2, count);
    callback.extend([0x04, 0x60]);
    reference(&mut callback, 0xb3, count);
    reference(&mut callback, 0xb2, control_field);
    reference(&mut callback, 0xb6, get_level);
    callback.extend([0x36, 4, 0x15, 4, 0x10, 100, 0xa1, 0, 4, 0xb1]);
    reference(&mut callback, 0xb2, control_field);
    callback.extend([0x15, 4, 0x04, 0x60]);
    reference(&mut callback, 0xb6, set_level);
    callback.extend([0x57, 0xb1]);
    f.method(
        "playerUpdate",
        &format!("(L{namespace}/Player;Ljava/lang/String;Ljava/lang/Object;)V"),
        1,
        callback,
    );

    let mut run = Vec::new();
    reference(&mut run, 0x13, locator);
    reference(&mut run, 0xb8, create);
    run.push(0x4b);
    run.push(0x2a);
    reference(&mut run, 0x13, control_name);
    reference(&mut run, 0xb6, get_control);
    reference(&mut run, 0xc0, control_class);
    reference(&mut run, 0xb3, control_field);
    run.push(0x2a);
    reference(&mut run, 0xbb, this);
    run.push(0x59);
    reference(&mut run, 0xb7, own_init);
    reference(&mut run, 0xb6, add_listener);
    reference(&mut run, 0xb2, control_field);
    run.push(0x03);
    reference(&mut run, 0xb6, set_level);
    run.push(0x57);
    reference(&mut run, 0xb2, count);
    run.extend([0x99, 0, 5, 0x02, 0xac]); // setter must return before callback
    // Bounded waiting allows the real Timer and VM scheduler to dispatch.
    for _ in 0..150 {
        run.extend([0x10, 10, 0x85]);
        reference(&mut run, 0xb8, sleep);
    }
    reference(&mut run, 0xb2, count);
    run.extend([0x10, 101, 0x9f, 0, 5, 0x02, 0xac]);
    // Repeated mute/level settings must not generate duplicate callbacks.
    for _ in 0..2 {
        reference(&mut run, 0xb2, control_field);
        run.push(0x04);
        reference(&mut run, 0xb6, set_mute);
        reference(&mut run, 0xb2, control_field);
        run.extend([0x11, 0, 200]);
        reference(&mut run, 0xb6, set_level);
        run.push(0x57);
    }
    run.extend([0x10, 30, 0x85]);
    reference(&mut run, 0xb8, sleep);
    reference(&mut run, 0xb2, count);
    run.extend([0x10, 102, 0x9f, 0, 5, 0x02, 0xac]);
    // A pending volume change must not survive close or keep its timer alive.
    reference(&mut run, 0xb2, control_field);
    run.push(0x03);
    reference(&mut run, 0xb6, set_mute);
    run.push(0x2a);
    reference(&mut run, 0xb6, close);
    run.extend([0x10, 30, 0x85]);
    reference(&mut run, 0xb8, sleep);
    reference(&mut run, 0xb2, bad);
    run.extend([0x99, 0, 5, 0x02, 0xac]);
    // Control lookup worked before realize above; after close it must throw.
    let closed_lookup = u16::try_from(run.len()).unwrap();
    run.push(0x2a);
    reference(&mut run, 0x13, control_name);
    reference(&mut run, 0xb6, get_control);
    run.extend([0x57, 0x02, 0xac]); // a successful lookup fails the fixture
    let closed_handler = u16::try_from(run.len()).unwrap();
    run.push(0x57); // discard the expected IllegalStateException
    reference(&mut run, 0xb2, count);
    run.push(0xac);
    let closed_exception = f.class("java/lang/IllegalStateException");
    f.method("run", "()I", 9, run);
    let Attribute::Code(code) = &mut f.methods.last_mut().unwrap().attributes[0] else {
        unreachable!();
    };
    code.exception_table.push(classfile::ExceptionHandler {
        start_pc: closed_lookup,
        end_pc: closed_handler,
        handler_pc: closed_handler,
        catch_type: closed_exception,
    });
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: f.pool,
        access_flags: 0x21,
        this_class: this,
        super_class: object,
        interfaces: vec![listener],
        fields: f.fields,
        methods: f.methods,
        attributes: vec![],
    }
}

struct Host(mmapi::Runtime<mmapi::NullAudioSink>);

impl natives::HostServices for Host {
    fn monotonic_millis(&self) -> i64 {
        0
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
    fn mmapi_create_tone(&mut self) -> Result<u64, EmuError> {
        self.0.create_tone_player()
    }
    fn mmapi_state(&mut self, handle: u64, now: i64) -> Result<i32, EmuError> {
        self.0.state(handle, now).map(|s| s as i32)
    }
    fn mmapi_transition(&mut self, handle: u64, transition: i32, now: i64) -> Result<(), EmuError> {
        assert_eq!(transition, 5);
        self.0.close(handle, now)
    }
    fn mmapi_volume(&mut self, handle: u64) -> Result<i32, EmuError> {
        self.0.volume(handle)
    }
    fn mmapi_set_volume(&mut self, handle: u64, volume: i32) -> Result<i32, EmuError> {
        self.0.set_volume(handle, volume)
    }
    fn mmapi_muted(&mut self, handle: u64) -> Result<bool, EmuError> {
        self.0.muted(handle)
    }
    fn mmapi_set_muted(&mut self, handle: u64, muted: bool) -> Result<(), EmuError> {
        self.0.set_muted(handle, muted)
    }
    fn mmapi_next_event(&mut self, handle: u64, now: i64) -> Result<i32, EmuError> {
        self.0
            .next_event(handle, now)
            .map(|e| e.map_or(0, |e| e.kind.code()))
    }
    fn mmapi_time(&mut self, handle: u64, selector: i32, _now: i64) -> Result<i64, EmuError> {
        assert_eq!(selector, 2);
        self.0.last_event_data(handle)
    }
}

#[test]
fn volume_listener_can_fade_without_recursive_callbacks_in_both_namespaces() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "volume_listener_can_fade_without_recursive_callbacks_in_both_namespaces"
    )) {
        return;
    }
    for namespace in ["javax/microedition/media", "com/siemens/mp/media"] {
        let limits = vm::Limits {
            max_frames: 64,
            ..vm::Limits::default()
        };
        let mut program = vm::Program::new();
        for class in runtime_bootstrap::production_bootstrap_classes() {
            program.add_class(&class, &limits).unwrap();
        }
        program.add_class(&fixture(namespace), &limits).unwrap();
        cldc::register_core_natives(program.native_registry_mut()).unwrap();
        mmapi::register_natives(program.native_registry_mut()).unwrap();
        let mut host = Host(mmapi::Runtime::new(
            mmapi::NullAudioSink::default(),
            mmapi::Limits::default(),
        ));
        let result = program
            .execute_with_context(
                "fixtures/VolumeEvents",
                "run",
                "()I",
                limits,
                false,
                &mut host,
            )
            .unwrap();
        assert_eq!(result.value, Some(vm::Value::Int(102)), "{namespace}");
        assert_eq!(result.thread_failure_count, 0);
    }
}
