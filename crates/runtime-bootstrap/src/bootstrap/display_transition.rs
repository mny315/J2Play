//! Deferred LCDUI display changes on the Java event thread.

use crate::bytecode_builder::Code;

use super::{
    ACC_FINAL, ACC_PRIVATE, ACC_PUBLIC, ACC_SUPER, ClassFile, ConstantPool, code_method, field,
};

pub(crate) fn javax_microedition_lcdui_display_transition() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("javax/microedition/lcdui/DisplayTransition");
    let super_class = pool.class("java/lang/Object");
    let runnable = pool.class("java/lang/Runnable");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let display_field = pool.field_ref(
        "javax/microedition/lcdui/DisplayTransition",
        "display",
        "Ljavax/microedition/lcdui/Display;",
    );
    let target_field = pool.field_ref(
        "javax/microedition/lcdui/DisplayTransition",
        "target",
        "Ljavax/microedition/lcdui/Displayable;",
    );
    let apply_current = pool.method_ref(
        "javax/microedition/lcdui/Display",
        "__applyCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)V",
    );

    let mut constructor = Code::default();
    constructor
        .emit(&[0x2a])
        .reference(0xb7, object_init)
        .emit(&[0x2a, 0x2b])
        .reference(0xb5, display_field)
        .emit(&[0x2a, 0x2c])
        .reference(0xb5, target_field)
        .emit(&[0xb1]);
    let constructor = code_method(
        &mut pool,
        code_name,
        0,
        "<init>",
        "(Ljavax/microedition/lcdui/Display;Ljavax/microedition/lcdui/Displayable;)V",
        2,
        3,
        constructor.finish(),
    );
    let mut run = Code::default();
    run.emit(&[0x2a])
        .reference(0xb4, display_field)
        .emit(&[0x2a])
        .reference(0xb4, target_field)
        .reference(0xb6, apply_current)
        .emit(&[0xb1]);
    let run = code_method(
        &mut pool,
        code_name,
        ACC_PUBLIC | ACC_FINAL,
        "run",
        "()V",
        2,
        1,
        run.finish(),
    );
    let fields = vec![
        field(
            &mut pool,
            ACC_PRIVATE | ACC_FINAL,
            "display",
            "Ljavax/microedition/lcdui/Display;",
        ),
        field(
            &mut pool,
            ACC_PRIVATE | ACC_FINAL,
            "target",
            "Ljavax/microedition/lcdui/Displayable;",
        ),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_FINAL | ACC_SUPER,
        this_class,
        super_class,
        interfaces: vec![runnable],
        fields,
        methods: vec![constructor, run],
        attributes: Vec::new(),
    }
}
