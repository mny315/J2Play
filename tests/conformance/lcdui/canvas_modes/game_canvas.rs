use super::*;

fn cached_game_canvas_fixture() -> ClassFile {
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: cached_game_canvas_constant_pool(),
        access_flags: 0x0021,
        this_class: 2,
        super_class: 4,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: cached_game_canvas_methods(),
        attributes: Vec::new(),
    }
}

fn cached_game_canvas_constant_pool() -> Vec<Option<Constant>> {
    vec![
        None,
        Some(Constant::Utf8("fixtures/CachedGameCanvas".to_owned())),
        Some(Constant::Class { name_index: 1 }),
        Some(Constant::Utf8(
            "javax/microedition/lcdui/game/GameCanvas".to_owned(),
        )),
        Some(Constant::Class { name_index: 3 }),
        Some(Constant::Utf8("javax/microedition/lcdui/Canvas".to_owned())),
        Some(Constant::Class { name_index: 5 }),
        Some(Constant::Utf8(
            "javax/microedition/lcdui/Graphics".to_owned(),
        )),
        Some(Constant::Class { name_index: 7 }),
        Some(Constant::Utf8("<init>".to_owned())),
        Some(Constant::Utf8("()V".to_owned())),
        Some(Constant::Utf8("Code".to_owned())),
        Some(Constant::Utf8("(Z)V".to_owned())),
        Some(Constant::NameAndType {
            name_index: 9,
            descriptor_index: 12,
        }),
        Some(Constant::Methodref {
            class_index: 4,
            name_and_type_index: 13,
        }),
        Some(Constant::NameAndType {
            name_index: 9,
            descriptor_index: 10,
        }),
        Some(Constant::Methodref {
            class_index: 2,
            name_and_type_index: 15,
        }),
        Some(Constant::Utf8("getGraphics".to_owned())),
        Some(Constant::Utf8(
            "()Ljavax/microedition/lcdui/Graphics;".to_owned(),
        )),
        Some(Constant::NameAndType {
            name_index: 17,
            descriptor_index: 18,
        }),
        Some(Constant::Methodref {
            class_index: 4,
            name_and_type_index: 19,
        }),
        Some(Constant::Utf8("setFullScreenMode".to_owned())),
        Some(Constant::NameAndType {
            name_index: 21,
            descriptor_index: 12,
        }),
        Some(Constant::Methodref {
            class_index: 6,
            name_and_type_index: 22,
        }),
        Some(Constant::Utf8("setColor".to_owned())),
        Some(Constant::Utf8("(I)V".to_owned())),
        Some(Constant::NameAndType {
            name_index: 24,
            descriptor_index: 25,
        }),
        Some(Constant::Methodref {
            class_index: 8,
            name_and_type_index: 26,
        }),
        Some(Constant::Utf8("fillRect".to_owned())),
        Some(Constant::Utf8("(IIII)V".to_owned())),
        Some(Constant::NameAndType {
            name_index: 28,
            descriptor_index: 29,
        }),
        Some(Constant::Methodref {
            class_index: 8,
            name_and_type_index: 30,
        }),
        Some(Constant::Utf8("flushGraphics".to_owned())),
        Some(Constant::NameAndType {
            name_index: 32,
            descriptor_index: 10,
        }),
        Some(Constant::Methodref {
            class_index: 4,
            name_and_type_index: 33,
        }),
        Some(Constant::Utf8("run".to_owned())),
        Some(Constant::Utf8("()I".to_owned())),
        Some(Constant::Integer(0x00ff_ffff)),
        Some(Constant::Utf8("setClip".to_owned())),
        Some(Constant::NameAndType {
            name_index: 38,
            descriptor_index: 29,
        }),
        Some(Constant::Methodref {
            class_index: 8,
            name_and_type_index: 39,
        }),
    ]
}

fn cached_game_canvas_methods() -> Vec<Member> {
    vec![
        Member {
            access_flags: 0x0001,
            name_index: 9,
            descriptor_index: 10,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: 11,
                max_stack: 2,
                max_locals: 1,
                code: vec![
                    0x2a, // aload_0
                    0x03, // iconst_0
                    0xb7, 0x00, 0x0e, // invokespecial GameCanvas.<init>(false)
                    0xb1, // return
                ],
                exception_table: Vec::new(),
                attributes: Vec::new(),
            })],
        },
        Member {
            access_flags: 0x0009,
            name_index: 35,
            descriptor_index: 36,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: 11,
                max_stack: 5,
                max_locals: 2,
                code: vec![
                    0xbb, 0x00, 0x02, // new CachedGameCanvas
                    0x59, // dup
                    0xb7, 0x00, 0x10, // invokespecial CachedGameCanvas.<init>()V
                    0x4b, // astore_0
                    0x2a, // aload_0
                    0xb6, 0x00, 0x14, // invokevirtual GameCanvas.getGraphics()
                    0x4c, // astore_1
                    0x2a, // aload_0
                    0x04, // iconst_1
                    0xb6, 0x00, 0x17, // invokevirtual Canvas.setFullScreenMode(true)
                    0x2b, // aload_1
                    0x03, // iconst_0: black
                    0xb6, 0x00, 0x1b, // invokevirtual Graphics.setColor(I)
                    0x2b, // aload_1
                    0x03, // iconst_0
                    0x03, // iconst_0
                    0x11, 0x00, 0xf0, // sipush 240
                    0x11, 0x01, 0x40, // sipush 320
                    0xb6, 0x00, 0x1f, // invokevirtual Graphics.fillRect(IIII)
                    0x2a, // aload_0
                    0xb6, 0x00, 0x22, // invokevirtual GameCanvas.flushGraphics()
                    0x2b, // aload_1
                    0x03, // iconst_0
                    0x03, // iconst_0
                    0x04, // iconst_1
                    0x04, // iconst_1
                    0xb6, 0x00, 0x28, // invokevirtual Graphics.setClip(IIII)
                    0x2a, // aload_0
                    0xb6, 0x00, 0x14, // invokevirtual GameCanvas.getGraphics()
                    0x4c, // astore_1
                    0x2b, // aload_1
                    0x12, 0x25, // ldc white
                    0xb6, 0x00, 0x1b, // invokevirtual Graphics.setColor(I)
                    0x2b, // aload_1
                    0x03, // iconst_0
                    0x03, // iconst_0
                    0x11, 0x00, 0xf0, // sipush 240
                    0x11, 0x01, 0x40, // sipush 320
                    0xb6, 0x00, 0x1f, // invokevirtual Graphics.fillRect(IIII)
                    0x2a, // aload_0
                    0xb6, 0x00, 0x22, // invokevirtual GameCanvas.flushGraphics()
                    0x04, // iconst_1
                    0xac, // ireturn
                ],
                exception_table: Vec::new(),
                attributes: Vec::new(),
            })],
        },
    ]
}

#[test]
fn game_canvas_graphics_tracks_resize_and_resets_state_on_each_get() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "game_canvas_graphics_tracks_resize_and_resets_state_on_each_get"
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
    let execution = canvas_modes_program(&limits, &cached_game_canvas_fixture())
        .execute_with_context(
            "fixtures/CachedGameCanvas",
            "run",
            "()I",
            limits,
            false,
            &mut context,
        )
        .unwrap();

    assert_eq!(execution.value, Some(vm::Value::Int(1)));
    assert_eq!(context.frames.len(), 2);
    let (width, height, pixels) = &context.frames[0];
    assert_eq!((*width, *height), (240, 320));
    assert_eq!(pixels.len(), 240 * 320);
    assert!(pixels.iter().all(|pixel| *pixel == 0xff00_0000));

    let (width, height, pixels) = &context.frames[1];
    assert_eq!((*width, *height), (240, 320));
    assert_eq!(pixels.len(), 240 * 320);
    assert!(pixels.iter().all(|pixel| *pixel == 0xffff_ffff));
}
