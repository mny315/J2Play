use crate::bytecode_builder::Code;

use super::{
    ACC_FINAL, ACC_PRIVATE, ACC_PUBLIC, ACC_STATIC, ACC_SUPER, Attribute, ClassFile, Constant,
    ConstantPool, Member, code_method, field, native_method,
};

/// Siemens published its early Mobile Media API in a vendor namespace with
/// the same class and method model as MMAPI.  Reuse the canonical Rust-owned
/// implementation while translating both binary names and the dotted control
/// names accepted by `Player.getControl`.
pub(crate) fn com_siemens_mp_media_classes(classes: &[ClassFile]) -> Vec<ClassFile> {
    classes
        .iter()
        .filter(|class| {
            class
                .class_name(class.this_class)
                .is_some_and(|name| name.starts_with("javax/microedition/media/"))
        })
        .cloned()
        .map(|mut class| {
            for constant in class.constant_pool.iter_mut().flatten() {
                let Constant::Utf8(value) = constant else {
                    continue;
                };
                if value.contains("javax/microedition/media") {
                    *value = value.replace("javax/microedition/media", "com/siemens/mp/media");
                }
                if value.contains("javax.microedition.media") {
                    *value = value.replace("javax.microedition.media", "com.siemens.mp.media");
                }
            }
            class
        })
        .collect()
}

pub(crate) fn com_samsung_util_audio_clip() -> ClassFile {
    const CLASS: &str = "com/samsung/util/AudioClip";

    let mut pool = ConstantPool::new();
    let this_class = pool.class(CLASS);
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let handle = pool.field_ref(CLASS, "handle", "J");
    let create_default = pool.method_ref(CLASS, "createDefault0", "()J");
    let create_bytes = pool.method_ref(CLASS, "createBytes0", "(I[BII)J");
    let create_resource = pool.method_ref(CLASS, "createResource0", "(ILjava/lang/String;)J");
    let pause = pool.method_ref(CLASS, "pause0", "(J)V");
    let play = pool.method_ref(CLASS, "play0", "(JII)V");
    let resume = pool.method_ref(CLASS, "resume0", "(J)V");
    let stop = pool.method_ref(CLASS, "stop0", "(J)V");

    let mut methods = Vec::new();
    for (descriptor, arguments, create, max_stack, max_locals) in [
        ("()V", &[][..], create_default, 3, 1),
        (
            "(I[BII)V",
            &[0x1b, 0x2c, 0x1d, 0x15, 0x04][..],
            create_bytes,
            5,
            5,
        ),
        (
            "(ILjava/lang/String;)V",
            &[0x1b, 0x2c][..],
            create_resource,
            3,
            3,
        ),
    ] {
        let mut code = Code::default();
        code.emit(&[0x2a])
            .reference(0xb7, object_init)
            .emit(&[0x2a])
            .emit(arguments)
            .reference(0xb8, create)
            .reference(0xb5, handle)
            .emit(&[0xb1]);
        methods.push(code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "<init>",
            descriptor,
            max_stack,
            max_locals,
            code.finish(),
        ));
    }
    methods.extend([
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            "isSupported",
            "()Z",
            1,
            0,
            vec![0x04, 0xac], // iconst_1; ireturn
        ),
        audio_clip_handle_method(&mut pool, code_name, "pause", pause),
        code_method(&mut pool, code_name, ACC_PUBLIC, "play", "(II)V", 4, 3, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb4, handle)
                .emit(&[0x1b, 0x1c])
                .reference(0xb8, play)
                .emit(&[0xb1]);
            code.finish()
        }),
        audio_clip_handle_method(&mut pool, code_name, "resume", resume),
        audio_clip_handle_method(&mut pool, code_name, "stop", stop),
        native_method(&mut pool, ACC_PRIVATE | ACC_STATIC, "createDefault0", "()J"),
        native_method(
            &mut pool,
            ACC_PRIVATE | ACC_STATIC,
            "createBytes0",
            "(I[BII)J",
        ),
        native_method(
            &mut pool,
            ACC_PRIVATE | ACC_STATIC,
            "createResource0",
            "(ILjava/lang/String;)J",
        ),
        native_method(&mut pool, ACC_PRIVATE | ACC_STATIC, "pause0", "(J)V"),
        native_method(&mut pool, ACC_PRIVATE | ACC_STATIC, "play0", "(JII)V"),
        native_method(&mut pool, ACC_PRIVATE | ACC_STATIC, "resume0", "(J)V"),
        native_method(&mut pool, ACC_PRIVATE | ACC_STATIC, "stop0", "(J)V"),
    ]);
    let fields = vec![
        int_constant_field(&mut pool, "TYPE_MMF", 1),
        int_constant_field(&mut pool, "TYPE_MP3", 2),
        int_constant_field(&mut pool, "TYPE_MIDI", 3),
        field(&mut pool, ACC_PRIVATE, "handle", "J"),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn int_constant_field(pool: &mut ConstantPool, name: &str, value: i32) -> Member {
    let constant_value_name = pool.utf8("ConstantValue");
    let value_index = pool.push(Constant::Integer(value));
    Member {
        access_flags: ACC_PUBLIC | ACC_STATIC | ACC_FINAL,
        name_index: pool.utf8(name),
        descriptor_index: pool.utf8("I"),
        attributes: vec![Attribute::Raw {
            name_index: constant_value_name,
            name: "ConstantValue".to_owned(),
            bytes: value_index.to_be_bytes().to_vec(),
        }],
    }
}

pub(crate) fn com_nokia_mid_sound_sound() -> ClassFile {
    const CLASS: &str = "com/nokia/mid/sound/Sound";

    let mut pool = ConstantPool::new();
    let this_class = pool.class(CLASS);
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let handle = pool.field_ref(CLASS, "handle", "J");
    let gain = pool.field_ref(CLASS, "gain", "I");
    let create_bytes = pool.method_ref(CLASS, "createBytes0", "([BI)J");
    let create_tone = pool.method_ref(CLASS, "createTone0", "(IJ)J");
    let close = pool.method_ref(CLASS, "close0", "(J)V");
    let play = pool.method_ref(CLASS, "play0", "(JII)V");
    let resume = pool.method_ref(CLASS, "resume0", "(J)V");
    let state = pool.method_ref(CLASS, "state0", "(J)I");
    let stop = pool.method_ref(CLASS, "stop0", "(J)V");
    let set_gain = pool.method_ref(CLASS, "setGain0", "(JI)I");

    let mut methods = vec![
        nokia_sound_constructor(
            &mut pool,
            code_name,
            "([BI)V",
            object_init,
            handle,
            gain,
            create_bytes,
            false,
        ),
        nokia_sound_constructor(
            &mut pool,
            code_name,
            "(IJ)V",
            object_init,
            handle,
            gain,
            create_tone,
            true,
        ),
        nokia_sound_init_method(
            &mut pool,
            code_name,
            "([BI)V",
            handle,
            close,
            create_bytes,
            false,
        ),
        nokia_sound_init_method(
            &mut pool,
            code_name,
            "(IJ)V",
            handle,
            close,
            create_tone,
            true,
        ),
        code_method(&mut pool, code_name, ACC_PUBLIC, "play", "(I)V", 5, 2, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb4, handle)
                .emit(&[0x1b, 0x2a]) // loop count, this
                .reference(0xb4, gain)
                .reference(0xb8, play)
                .emit(&[0xb1]);
            code.finish()
        }),
        audio_handle_method(&mut pool, code_name, "resume", resume, handle),
        audio_handle_method(&mut pool, code_name, "stop", stop, handle),
        code_method(&mut pool, code_name, ACC_PUBLIC, "release", "()V", 3, 1, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb4, handle)
                .reference(0xb8, close)
                .emit(&[0x2a, 0x09]) // this, lconst_0
                .reference(0xb5, handle)
                .emit(&[0xb1]);
            code.finish()
        }),
        code_method(&mut pool, code_name, ACC_PUBLIC, "getState", "()I", 2, 1, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb4, handle)
                .reference(0xb8, state)
                .emit(&[0xac]);
            code.finish()
        }),
        code_method(&mut pool, code_name, ACC_PUBLIC, "setGain", "(I)V", 4, 2, {
            let mut code = Code::default();
            code.emit(&[0x2a, 0x2a]) // retain this for the normalized gain
                .reference(0xb4, handle)
                .emit(&[0x1b])
                .reference(0xb8, set_gain)
                .reference(0xb5, gain)
                .emit(&[0xb1]);
            code.finish()
        }),
        code_method(&mut pool, code_name, ACC_PUBLIC, "getGain", "()I", 1, 1, {
            let mut code = Code::default();
            code.emit(&[0x2a]).reference(0xb4, gain).emit(&[0xac]);
            code.finish()
        }),
    ];
    for (name, descriptor) in [
        ("createBytes0", "([BI)J"),
        ("createTone0", "(IJ)J"),
        ("close0", "(J)V"),
        ("play0", "(JII)V"),
        ("resume0", "(J)V"),
        ("state0", "(J)I"),
        ("stop0", "(J)V"),
        ("setGain0", "(JI)I"),
    ] {
        methods.push(native_method(
            &mut pool,
            ACC_PRIVATE | ACC_STATIC,
            name,
            descriptor,
        ));
    }
    let fields = vec![
        field(&mut pool, ACC_PRIVATE, "handle", "J"),
        field(&mut pool, ACC_PRIVATE, "gain", "I"),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn nokia_sound_constructor(
    pool: &mut ConstantPool,
    code_name: u16,
    descriptor: &str,
    object_init: u16,
    handle: u16,
    gain: u16,
    create: u16,
    tone: bool,
) -> Member {
    let arguments = if tone {
        [0x1b, 0x20] // iload_1, lload_2
    } else {
        [0x2b, 0x1c] // aload_1, iload_2
    };
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb7, object_init)
        .emit(&[0x2a])
        .emit(&arguments)
        .reference(0xb8, create)
        .reference(0xb5, handle)
        .emit(&[0x2a, 0x11, 0x00, 0xff]) // this, default gain 255
        .reference(0xb5, gain)
        .emit(&[0xb1]);
    code_method(
        pool,
        code_name,
        ACC_PUBLIC,
        "<init>",
        descriptor,
        4,
        4,
        code.finish(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn nokia_sound_init_method(
    pool: &mut ConstantPool,
    code_name: u16,
    descriptor: &str,
    handle: u16,
    close: u16,
    create: u16,
    tone: bool,
) -> Member {
    let arguments = if tone {
        [0x1b, 0x20] // iload_1, lload_2
    } else {
        [0x2b, 0x1c] // aload_1, iload_2
    };
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb4, handle)
        .reference(0xb8, close)
        .emit(&[0x2a, 0x09]) // clear the released handle before fallible reinitialization
        .reference(0xb5, handle)
        .emit(&[0x2a])
        .emit(&arguments)
        .reference(0xb8, create)
        .reference(0xb5, handle)
        .emit(&[0xb1]);
    code_method(
        pool,
        code_name,
        ACC_PUBLIC,
        "init",
        descriptor,
        4,
        4,
        code.finish(),
    )
}

fn audio_handle_method(
    pool: &mut ConstantPool,
    code_name: u16,
    name: &str,
    native: u16,
    handle: u16,
) -> Member {
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb4, handle)
        .reference(0xb8, native)
        .emit(&[0xb1]);
    code_method(
        pool,
        code_name,
        ACC_PUBLIC,
        name,
        "()V",
        2,
        1,
        code.finish(),
    )
}

pub(crate) fn audio_clip_handle_method(
    pool: &mut ConstantPool,
    code_name: u16,
    name: &str,
    native: u16,
) -> Member {
    let handle = pool.field_ref("com/samsung/util/AudioClip", "handle", "J");
    audio_handle_method(pool, code_name, name, native, handle)
}
