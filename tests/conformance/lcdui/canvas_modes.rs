use classfile::{Attribute, ClassFile, CodeAttribute, Constant, Member};

mod game_canvas;

#[allow(clippy::too_many_lines)]
fn canvas_modes_fixture() -> ClassFile {
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: vec![
            None,
            Some(Constant::Utf8("fixtures/CanvasModes".to_owned())),
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::Utf8("javax/microedition/lcdui/Canvas".to_owned())),
            Some(Constant::Class { name_index: 3 }),
            Some(Constant::Utf8("<init>".to_owned())),
            Some(Constant::Utf8("()V".to_owned())),
            Some(Constant::Utf8("Code".to_owned())),
            Some(Constant::NameAndType {
                name_index: 5,
                descriptor_index: 6,
            }),
            Some(Constant::Methodref {
                class_index: 4,
                name_and_type_index: 8,
            }),
            Some(Constant::Utf8("paint".to_owned())),
            Some(Constant::Utf8(
                "(Ljavax/microedition/lcdui/Graphics;)V".to_owned(),
            )),
            Some(Constant::Utf8("run".to_owned())),
            Some(Constant::Utf8("()I".to_owned())),
            Some(Constant::Methodref {
                class_index: 2,
                name_and_type_index: 8,
            }),
            Some(Constant::Utf8("getWidth".to_owned())),
            Some(Constant::NameAndType {
                name_index: 15,
                descriptor_index: 13,
            }),
            Some(Constant::Methodref {
                class_index: 4,
                name_and_type_index: 16,
            }),
            Some(Constant::Utf8("getHeight".to_owned())),
            Some(Constant::NameAndType {
                name_index: 18,
                descriptor_index: 13,
            }),
            Some(Constant::Methodref {
                class_index: 4,
                name_and_type_index: 19,
            }),
            Some(Constant::Utf8("setFullScreenMode".to_owned())),
            Some(Constant::Utf8("(Z)V".to_owned())),
            Some(Constant::NameAndType {
                name_index: 21,
                descriptor_index: 22,
            }),
            Some(Constant::Methodref {
                class_index: 4,
                name_and_type_index: 23,
            }),
        ],
        access_flags: 0x0021,
        this_class: 2,
        super_class: 4,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![
            Member {
                access_flags: 0x0001,
                name_index: 5,
                descriptor_index: 6,
                attributes: vec![Attribute::Code(CodeAttribute {
                    name_index: 7,
                    max_stack: 1,
                    max_locals: 1,
                    code: vec![
                        0x2a, // aload_0
                        0xb7, 0x00, 0x09, // invokespecial Canvas.<init>()V
                        0xb1, // return
                    ],
                    exception_table: Vec::new(),
                    attributes: Vec::new(),
                })],
            },
            Member {
                access_flags: 0x0004,
                name_index: 10,
                descriptor_index: 11,
                attributes: vec![Attribute::Code(CodeAttribute {
                    name_index: 7,
                    max_stack: 0,
                    max_locals: 2,
                    code: vec![0xb1], // return
                    exception_table: Vec::new(),
                    attributes: Vec::new(),
                })],
            },
            Member {
                access_flags: 0x0009,
                name_index: 12,
                descriptor_index: 13,
                attributes: vec![Attribute::Code(CodeAttribute {
                    name_index: 7,
                    max_stack: 3,
                    max_locals: 2,
                    code: vec![
                        0xbb, 0x00, 0x02, // new CanvasModes
                        0x59, // dup
                        0xb7, 0x00, 0x0e, // invokespecial CanvasModes.<init>()V
                        0x4b, // astore_0
                        0x2a, // aload_0
                        0x03, // iconst_0
                        0xb6, 0x00, 0x18, // invokevirtual setFullScreenMode(false)
                        0x03, // iconst_0
                        0x3c, // istore_1
                        0x1b, // iload_1
                        0x2a, // aload_0
                        0xb6, 0x00, 0x11, // invokevirtual getWidth()
                        0x60, // iadd
                        0x3c, // istore_1
                        0x1b, // iload_1
                        0x2a, // aload_0
                        0xb6, 0x00, 0x14, // invokevirtual getHeight()
                        0x11, 0x03, 0xe8, // sipush 1000
                        0x68, // imul
                        0x60, // iadd
                        0x3c, // istore_1
                        0x2a, // aload_0
                        0x04, // iconst_1
                        0xb6, 0x00, 0x18, // invokevirtual setFullScreenMode(true)
                        0x1b, // iload_1
                        0x2a, // aload_0
                        0xb6, 0x00, 0x11, // invokevirtual getWidth()
                        0x60, // iadd
                        0x3c, // istore_1
                        0x1b, // iload_1
                        0x2a, // aload_0
                        0xb6, 0x00, 0x14, // invokevirtual getHeight()
                        0x11, 0x03, 0xe8, // sipush 1000
                        0x68, // imul
                        0x60, // iadd
                        0x3c, // istore_1
                        0x2a, // aload_0
                        0x04, // iconst_1
                        0xb6, 0x00, 0x18, // duplicate true request must be a no-op
                        0x2a, // aload_0
                        0x03, // iconst_0
                        0xb6, 0x00, 0x18, // invokevirtual setFullScreenMode(false)
                        0x1b, // iload_1
                        0x2a, // aload_0
                        0xb6, 0x00, 0x11, // invokevirtual getWidth()
                        0x60, // iadd
                        0x3c, // istore_1
                        0x1b, // iload_1
                        0x2a, // aload_0
                        0xb6, 0x00, 0x14, // invokevirtual getHeight()
                        0x11, 0x03, 0xe8, // sipush 1000
                        0x68, // imul
                        0x60, // iadd
                        0x3c, // istore_1
                        0x1b, // iload_1
                        0xac, // ireturn
                    ],
                    exception_table: Vec::new(),
                    attributes: Vec::new(),
                })],
            },
        ],
        attributes: Vec::new(),
    }
}

fn throwing_paint_canvas_fixture() -> ClassFile {
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: vec![
            None,
            Some(Constant::Utf8("fixtures/ThrowingPaintCanvas".to_owned())),
            Some(Constant::Class { name_index: 1 }),
            Some(Constant::Utf8("javax/microedition/lcdui/Canvas".to_owned())),
            Some(Constant::Class { name_index: 3 }),
            Some(Constant::Utf8("<init>".to_owned())),
            Some(Constant::Utf8("()V".to_owned())),
            Some(Constant::Utf8("Code".to_owned())),
            Some(Constant::NameAndType {
                name_index: 5,
                descriptor_index: 6,
            }),
            Some(Constant::Methodref {
                class_index: 4,
                name_and_type_index: 8,
            }),
            Some(Constant::Utf8("paint".to_owned())),
            Some(Constant::Utf8(
                "(Ljavax/microedition/lcdui/Graphics;)V".to_owned(),
            )),
            Some(Constant::Utf8("java/lang/NullPointerException".to_owned())),
            Some(Constant::Class { name_index: 12 }),
            Some(Constant::Methodref {
                class_index: 13,
                name_and_type_index: 8,
            }),
        ],
        access_flags: 0x0021,
        this_class: 2,
        super_class: 4,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![
            Member {
                access_flags: 0x0001,
                name_index: 5,
                descriptor_index: 6,
                attributes: vec![Attribute::Code(CodeAttribute {
                    name_index: 7,
                    max_stack: 1,
                    max_locals: 1,
                    code: vec![
                        0x2a, // aload_0
                        0xb7, 0x00, 0x09, // invokespecial Canvas.<init>()V
                        0xb1, // return
                    ],
                    exception_table: Vec::new(),
                    attributes: Vec::new(),
                })],
            },
            Member {
                access_flags: 0x0004,
                name_index: 10,
                descriptor_index: 11,
                attributes: vec![Attribute::Code(CodeAttribute {
                    name_index: 7,
                    max_stack: 2,
                    max_locals: 2,
                    code: vec![
                        0xbb, 0x00, 0x0d, // new NullPointerException
                        0x59, // dup
                        0xb7, 0x00, 0x0e, // invokespecial NullPointerException.<init>()V
                        0xbf, // athrow
                    ],
                    exception_table: Vec::new(),
                    attributes: Vec::new(),
                })],
            },
        ],
        attributes: Vec::new(),
    }
}

fn nokia_full_canvas_fixture() -> ClassFile {
    let mut class = canvas_modes_fixture();
    class.constant_pool[1] = Some(Constant::Utf8("fixtures/NokiaFullCanvas".to_owned()));
    class.constant_pool[3] = Some(Constant::Utf8("com/nokia/mid/ui/FullCanvas".to_owned()));
    let Attribute::Code(run) = &mut class.methods[2].attributes[0] else {
        panic!("Canvas mode fixture run method has bytecode");
    };
    run.max_stack = 3;
    run.max_locals = 1;
    run.code = vec![
        0xbb, 0x00, 0x02, // new NokiaFullCanvas
        0x59, // dup
        0xb7, 0x00, 0x0e, // invokespecial NokiaFullCanvas.<init>()V
        0x4b, // astore_0
        0x2a, // aload_0
        0xb6, 0x00, 0x11, // invokevirtual getWidth()
        0x2a, // aload_0
        0xb6, 0x00, 0x14, // invokevirtual getHeight()
        0x11, 0x03, 0xe8, // sipush 1000
        0x68, // imul
        0x60, // iadd
        0xac, // ireturn
    ];
    class
}

fn canvas_modes_program(limits: &vm::Limits, fixture: &ClassFile) -> vm::Program {
    let mut program = vm::Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display_modes(
        (limits.lcd_width, limits.lcd_height),
        (limits.lcd_normal_width, limits.lcd_normal_height),
    ) {
        program.add_class(&entry.class, limits).unwrap();
    }
    program.add_class(fixture, limits).unwrap();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    midp::register_natives(program.native_registry_mut()).unwrap();
    program
}

#[derive(Default)]
pub(super) struct FrameCapture {
    pub(super) frames: Vec<(u32, u32, Vec<u32>)>,
    full_size: Option<(u32, u32)>,
    normal_size: Option<(u32, u32)>,
}

impl natives::HostServices for FrameCapture {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _name: &str) -> Option<&str> {
        None
    }

    fn read_resource(&self, _name: &str) -> Result<Option<Vec<u8>>, diagnostics::EmuError> {
        Ok(None)
    }

    fn lcd_dimensions(&self) -> (u32, u32) {
        self.full_size.unwrap_or((240, 320))
    }

    fn lcd_non_fullscreen_dimensions(&self) -> (u32, u32) {
        self.normal_size.unwrap_or((240, 266))
    }

    fn present_lcdui_frame(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u32],
    ) -> Result<(), diagnostics::EmuError> {
        self.frames.push((width, height, pixels.to_vec()));
        Ok(())
    }
}

#[test]
fn displayables_follow_profile_sizes_and_canvas_fullscreen_requests() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "displayables_follow_profile_sizes_and_canvas_fullscreen_requests"
    )) {
        return;
    }
    let canvas = canvas_modes_fixture();
    let mut screen = canvas.clone();
    screen.constant_pool[3] = Some(Constant::Utf8("javax/microedition/lcdui/Screen".into()));
    let Attribute::Code(code) = &mut screen.methods[2].attributes[0] else {
        unreachable!();
    };
    code.code = vec![
        0xbb, 0x00, 0x02, 0x59, 0xb7, 0x00, 0x0e, 0x4b, // new Screen subclass
        0x2a, 0xb6, 0x00, 0x11, // getWidth()
        0x11, 0x03, 0xe8, 0x68, // * 1000
        0x2a, 0xb6, 0x00, 0x14, 0x60, 0xac, // + getHeight()
    ];
    for (full, normal, canvas_expected, screen_expected) in [
        ((240, 320), (240, 266), 852_720, 240_320),
        ((176, 208), (176, 182), 572_528, 176_208),
    ] {
        let limits = vm::Limits {
            lcd_width: full.0,
            lcd_height: full.1,
            lcd_normal_width: normal.0,
            lcd_normal_height: normal.1,
            ..vm::Limits::default()
        };
        for (fixture, expected) in [(&canvas, canvas_expected), (&screen, screen_expected)] {
            let mut host = FrameCapture {
                full_size: Some(full),
                normal_size: Some(normal),
                ..FrameCapture::default()
            };
            let execution = canvas_modes_program(&limits, fixture)
                .execute_with_context(
                    "fixtures/CanvasModes",
                    "run",
                    "()I",
                    limits.clone(),
                    false,
                    &mut host,
                )
                .unwrap();
            assert_eq!(execution.value, Some(vm::Value::Int(expected)), "{full:?}");
        }
    }
}

#[test]
fn paint_callback_failure_does_not_escape_the_lcdui_dispatcher() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "paint_callback_failure_does_not_escape_the_lcdui_dispatcher"
    )) {
        return;
    }
    let limits = vm::Limits {
        lcd_width: 240,
        lcd_height: 320,
        lcd_normal_width: 240,
        lcd_normal_height: 266,
        ..vm::Limits::default()
    };
    let mut context = FrameCapture::default();
    let calls = [vm::InstanceCall {
        target: vm::CallTarget::Instance,
        name: "__hostIdle".to_owned(),
        descriptor: "()V".to_owned(),
        arguments: Vec::new(),
    }];
    let execution = canvas_modes_program(&limits, &throwing_paint_canvas_fixture())
        .execute_instance_sequence_with_context(
            "fixtures/ThrowingPaintCanvas",
            &calls,
            limits,
            false,
            &mut context,
        )
        .unwrap();

    assert!(context.frames.is_empty());
    assert!(execution.caught_exception_diagnostics.iter().any(|entry| {
        entry.contains("java/lang/NullPointerException")
            && entry.contains("fixtures/ThrowingPaintCanvas::paint")
    }));
}

#[test]
fn profile_can_refuse_a_fullscreen_request() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "profile_can_refuse_a_fullscreen_request"
    )) {
        return;
    }
    let limits = vm::Limits {
        lcd_width: 240,
        lcd_height: 320,
        lcd_normal_width: 240,
        lcd_normal_height: 266,
        lcd_fullscreen_available: false,
        ..vm::Limits::default()
    };
    let execution = canvas_modes_program(&limits, &canvas_modes_fixture())
        .execute("fixtures/CanvasModes", "run", "()I", limits, false)
        .unwrap();

    assert_eq!(execution.value, Some(vm::Value::Int(798_720)));
}

#[test]
fn nokia_full_canvas_keeps_legacy_fullscreen_games_full_size() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "nokia_full_canvas_keeps_legacy_fullscreen_games_full_size"
    )) {
        return;
    }
    let limits = vm::Limits {
        lcd_width: 240,
        lcd_height: 320,
        lcd_normal_width: 240,
        lcd_normal_height: 266,
        ..vm::Limits::default()
    };
    let execution = canvas_modes_program(&limits, &nokia_full_canvas_fixture())
        .execute("fixtures/NokiaFullCanvas", "run", "()I", limits, false)
        .unwrap();

    assert_eq!(execution.value, Some(vm::Value::Int(320_240)));
}
