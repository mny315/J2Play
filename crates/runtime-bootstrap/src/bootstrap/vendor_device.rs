//! Vendor light and vibration API declarations.

use super::{
    ACC_FINAL, ACC_PRIVATE, ACC_PUBLIC, ACC_STATIC, ACC_SUPER, ClassFile, ConstantPool,
    code_method, native_method,
};

pub(crate) fn com_nokia_mid_ui_device_control() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("com/nokia/mid/ui/DeviceControl");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PRIVATE,
            "<init>",
            "()V",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb7,
                (object_init >> 8) as u8,
                object_init as u8, // invokespecial Object.<init>:()V
                0xb1,              // return
            ],
        ),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "setLights", "(II)V"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "startVibra", "(IJ)V"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "stopVibra", "()V"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "flashLights", "(J)V"),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_FINAL | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn com_siemens_mp_game_light() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("com/siemens/mp/game/Light");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "<init>",
            "()V",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb7,
                (object_init >> 8) as u8,
                object_init as u8, // invokespecial Object.<init>:()V
                0xb1,              // return
            ],
        ),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "setLightOff", "()V"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "setLightOn", "()V"),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn com_siemens_mp_game_vibrator() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("com/siemens/mp/game/Vibrator");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "<init>",
            "()V",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb7,
                (object_init >> 8) as u8,
                object_init as u8, // invokespecial Object.<init>:()V
                0xb1,              // return
            ],
        ),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "startVibrator", "()V"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "stopVibrator", "()V"),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "triggerVibrator",
            "(I)V",
        ),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}
