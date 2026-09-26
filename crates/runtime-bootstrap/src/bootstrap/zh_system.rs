//! The `zh.system` scene, input, font and `MIDlet` framework declarations.

use crate::bytecode_builder::Code;

use super::{
    ACC_ABSTRACT, ACC_FINAL, ACC_PRIVATE, ACC_PROTECTED, ACC_PUBLIC, ACC_STATIC, ACC_SUPER,
    ClassFile, ConstantPool, abstract_method, code_method, field, interface_class,
};

pub(crate) fn zh_system_input_target() -> ClassFile {
    interface_class(
        "zh/system/a",
        &[],
        &[("a", "(II)Z"), ("b", "(II)Z"), ("c", "(II)Z")],
    )
}

pub(crate) fn zh_system_scene() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("zh/system/d");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let input_target = pool.field_ref("zh/system/d", "a", "Lzh/system/a;");
    let pointer_pressed = pool.interface_method_ref("zh/system/a", "a", "(II)Z");
    let pointer_released = pool.interface_method_ref("zh/system/a", "b", "(II)Z");
    let pointer_dragged = pool.interface_method_ref("zh/system/a", "c", "(II)Z");

    let mut methods = vec![
        code_method(&mut pool, code_name, ACC_PUBLIC, "<init>", "()V", 1, 1, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb7, object_init)
                .emit(&[0xb1]);
            code.finish()
        }),
        abstract_method(&mut pool, ACC_PUBLIC, "a", "()V"),
        abstract_method(
            &mut pool,
            ACC_PUBLIC,
            "a",
            "(Ljavax/microedition/lcdui/Graphics;)V",
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "a",
            "(I)V",
            0,
            2,
            vec![0xb1],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "b",
            "(I)V",
            0,
            2,
            vec![0xb1],
        ),
    ];
    for (name, callback) in [
        ("a", pointer_pressed),
        ("b", pointer_released),
        ("c", pointer_dragged),
    ] {
        methods.push(code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            name,
            "(II)Z",
            3,
            3,
            {
                let mut code = Code::default();
                code.emit(&[0x2a])
                    .reference(0xb4, input_target)
                    .jump(0xc7, "dispatch");
                code.emit(&[0x03, 0xac]);
                code.label("dispatch")
                    .emit(&[0x2a])
                    .reference(0xb4, input_target)
                    .emit(&[0x1b, 0x1c])
                    .reference(0xb9, callback)
                    .emit(&[0x03, 0x00, 0xac]); // three argument slots, padding, ireturn
                code.finish()
            },
        ));
    }
    let fields = vec![field(&mut pool, ACC_PUBLIC, "a", "Lzh/system/a;")];
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER | ACC_ABSTRACT,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn zh_system_fonts() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("zh/system/e");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let get_font = pool.method_ref(
        "javax/microedition/lcdui/Font",
        "getFont",
        "(III)Ljavax/microedition/lcdui/Font;",
    );
    let small = pool.field_ref("zh/system/e", "a", "Ljavax/microedition/lcdui/Font;");
    let medium = pool.field_ref("zh/system/e", "b", "Ljavax/microedition/lcdui/Font;");
    let large = pool.field_ref("zh/system/e", "c", "Ljavax/microedition/lcdui/Font;");
    let methods = vec![code_method(
        &mut pool,
        code_name,
        ACC_STATIC,
        "<clinit>",
        "()V",
        3,
        0,
        {
            let mut code = Code::default();
            code.emit(&[0x03, 0x03, 0x10, 0x08]) // system/plain/small
                .reference(0xb8, get_font)
                .reference(0xb3, small)
                .emit(&[0x03, 0x03, 0x03]) // system/plain/medium
                .reference(0xb8, get_font)
                .reference(0xb3, medium)
                .emit(&[0x03, 0x03, 0x10, 0x10]) // system/plain/large
                .reference(0xb8, get_font)
                .reference(0xb3, large)
                .emit(&[0xb1]);
            code.finish()
        },
    )];
    let fields = ["a", "b", "c"]
        .into_iter()
        .map(|name| {
            field(
                &mut pool,
                ACC_PUBLIC | ACC_STATIC,
                name,
                "Ljavax/microedition/lcdui/Font;",
            )
        })
        .collect();
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_FINAL | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn zh_system_canvas() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("zh/system/f");
    let super_class = pool.class("javax/microedition/lcdui/Canvas");
    let runnable = pool.class("java/lang/Runnable");
    let code_name = pool.utf8("Code");
    let canvas_init = pool.method_ref("javax/microedition/lcdui/Canvas", "<init>", "()V");
    let repaint = pool.method_ref("javax/microedition/lcdui/Canvas", "repaint", "()V");
    let service_repaints =
        pool.method_ref("javax/microedition/lcdui/Canvas", "serviceRepaints", "()V");
    let thread_yield = pool.method_ref("java/lang/Thread", "yield", "()V");
    let update = pool.method_ref("zh/system/d", "a", "()V");
    let paint_scene = pool.method_ref("zh/system/d", "a", "(Ljavax/microedition/lcdui/Graphics;)V");
    let key_pressed = pool.method_ref("zh/system/d", "a", "(I)V");
    let key_released = pool.method_ref("zh/system/d", "b", "(I)V");
    let pointer_pressed = pool.method_ref("zh/system/d", "a", "(II)Z");
    let pointer_released = pool.method_ref("zh/system/d", "b", "(II)Z");
    let pointer_dragged = pool.method_ref("zh/system/d", "c", "(II)Z");
    let current = pool.field_ref("zh/system/f", "current", "Lzh/system/d;");
    let held_key = pool.field_ref("zh/system/f", "heldKey", "I");
    let pressed_key = pool.field_ref("zh/system/f", "pressedKey", "I");
    let running = pool.field_ref("zh/system/f", "running", "Z");

    let constructor = code_method(
        &mut pool,
        code_name,
        ACC_PUBLIC,
        "<init>",
        "(Lzh/system/d;)V",
        2,
        2,
        {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb7, canvas_init)
                .emit(&[0x2a, 0x2b])
                .reference(0xb5, current)
                .emit(&[0xb1]);
            code.finish()
        },
    );
    let switch_scene = code_method(
        &mut pool,
        code_name,
        ACC_PUBLIC | ACC_SUPER,
        "a",
        "(Lzh/system/d;)Lzh/system/d;",
        2,
        3,
        {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb4, current)
                .emit(&[0x4d, 0x2a, 0x2b]) // retain the previous scene in local 2
                .reference(0xb5, current)
                .emit(&[0x2a, 0x03])
                .reference(0xb5, pressed_key)
                .emit(&[0x2a])
                .reference(0xb6, repaint)
                .emit(&[0x2c, 0xb0]);
            code.finish()
        },
    );
    let key_query = |pool: &mut ConstantPool, name: &str, key_field: u16| {
        code_method(pool, code_name, ACC_PUBLIC, name, "(I)Z", 2, 2, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb4, key_field)
                .emit(&[0x1b])
                .jump(0xa0, "false");
            code.emit(&[0x04, 0xac]);
            code.label("false").emit(&[0x03, 0xac]);
            code.finish()
        })
    };
    let clear_all = code_method(&mut pool, code_name, ACC_PUBLIC, "c", "()V", 2, 1, {
        let mut code = Code::default();
        code.emit(&[0x2a, 0x03])
            .reference(0xb5, held_key)
            .emit(&[0x2a, 0x03])
            .reference(0xb5, pressed_key)
            .emit(&[0xb1]);
        code.finish()
    });
    let clear_pressed = code_method(&mut pool, code_name, ACC_PUBLIC, "d", "()V", 2, 1, {
        let mut code = Code::default();
        code.emit(&[0x2a, 0x03])
            .reference(0xb5, pressed_key)
            .emit(&[0xb1]);
        code.finish()
    });
    let stop = code_method(&mut pool, code_name, 0, "e", "()V", 2, 1, {
        let mut code = Code::default();
        code.emit(&[0x2a, 0x03])
            .reference(0xb5, running)
            .emit(&[0xb1]);
        code.finish()
    });
    let paint = code_method(
        &mut pool,
        code_name,
        ACC_PUBLIC,
        "paint",
        "(Ljavax/microedition/lcdui/Graphics;)V",
        2,
        2,
        {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb4, current)
                .emit(&[0x2b])
                .reference(0xb6, paint_scene)
                .emit(&[0xb1]);
            code.finish()
        },
    );
    let press = code_method(
        &mut pool,
        code_name,
        ACC_PUBLIC,
        "keyPressed",
        "(I)V",
        2,
        2,
        {
            let mut code = Code::default();
            code.emit(&[0x2a, 0x1b])
                .reference(0xb5, held_key)
                .emit(&[0x2a, 0x1b])
                .reference(0xb5, pressed_key)
                .emit(&[0x2a])
                .reference(0xb4, current)
                .emit(&[0x1b])
                .reference(0xb6, key_pressed)
                .emit(&[0xb1]);
            code.finish()
        },
    );
    let release = code_method(
        &mut pool,
        code_name,
        ACC_PUBLIC,
        "keyReleased",
        "(I)V",
        2,
        2,
        {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb4, held_key)
                .emit(&[0x1b])
                .jump(0xa0, "dispatch");
            code.emit(&[0x2a, 0x03]).reference(0xb5, held_key);
            code.label("dispatch")
                .emit(&[0x2a])
                .reference(0xb4, current)
                .emit(&[0x1b])
                .reference(0xb6, key_released)
                .emit(&[0xb1]);
            code.finish()
        },
    );
    let run = code_method(&mut pool, code_name, ACC_PUBLIC, "run", "()V", 2, 1, {
        let mut code = Code::default();
        code.emit(&[0x2a, 0x04]).reference(0xb5, running);
        code.label("running")
            .emit(&[0x2a])
            .reference(0xb4, running)
            .jump(0x99, "done");
        code.emit(&[0x2a])
            .reference(0xb4, current)
            .reference(0xb6, update)
            .emit(&[0x2a])
            .reference(0xb6, repaint)
            .emit(&[0x2a])
            .reference(0xb6, service_repaints)
            .reference(0xb8, thread_yield)
            .jump(0xa7, "running");
        code.label("done").emit(&[0xb1]);
        code.finish()
    });
    let pointer_dispatch = |pool: &mut ConstantPool, name: &str, callback: u16| {
        code_method(pool, code_name, ACC_PUBLIC, name, "(II)V", 3, 3, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb4, current)
                .emit(&[0x1b, 0x1c])
                .reference(0xb6, callback)
                .emit(&[0x57, 0xb1]); // discard the handled flag
            code.finish()
        })
    };
    let methods = vec![
        constructor,
        switch_scene,
        key_query(&mut pool, "a", held_key),
        key_query(&mut pool, "b", pressed_key),
        clear_all,
        clear_pressed,
        stop,
        paint,
        press,
        release,
        run,
        pointer_dispatch(&mut pool, "pointerPressed", pointer_pressed),
        pointer_dispatch(&mut pool, "pointerReleased", pointer_released),
        pointer_dispatch(&mut pool, "pointerDragged", pointer_dragged),
    ];
    let fields = vec![
        field(&mut pool, ACC_PRIVATE, "current", "Lzh/system/d;"),
        field(&mut pool, ACC_PRIVATE, "heldKey", "I"),
        field(&mut pool, ACC_PRIVATE, "pressedKey", "I"),
        field(&mut pool, ACC_PRIVATE, "running", "Z"),
    ];
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces: vec![runnable],
        fields,
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn zh_system_game_midlet() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("zh/system/GameMIDlet");
    let super_class = pool.class("javax/microedition/midlet/MIDlet");
    let code_name = pool.utf8("Code");
    let midlet_init = pool.method_ref("javax/microedition/midlet/MIDlet", "<init>", "()V");
    let get_display = pool.method_ref(
        "javax/microedition/lcdui/Display",
        "getDisplay",
        "(Ljavax/microedition/midlet/MIDlet;)Ljavax/microedition/lcdui/Display;",
    );
    let set_current = pool.method_ref(
        "javax/microedition/lcdui/Display",
        "setCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)V",
    );
    let canvas_class = pool.class("zh/system/f");
    let canvas_init = pool.method_ref("zh/system/f", "<init>", "(Lzh/system/d;)V");
    let set_fullscreen = pool.method_ref("zh/system/f", "setFullScreenMode", "(Z)V");
    let clear_input = pool.method_ref("zh/system/f", "c", "()V");
    let stop_canvas = pool.method_ref("zh/system/f", "e", "()V");
    let thread_class = pool.class("java/lang/Thread");
    let thread_init = pool.method_ref("java/lang/Thread", "<init>", "(Ljava/lang/Runnable;)V");
    let thread_start = pool.method_ref("java/lang/Thread", "start", "()V");
    let println = pool.method_ref("java/io/PrintStream", "println", "(Ljava/lang/String;)V");
    let system_out = pool.field_ref("java/lang/System", "out", "Ljava/io/PrintStream;");
    let instance = pool.field_ref("zh/system/GameMIDlet", "instance", "Lzh/system/GameMIDlet;");
    let display = pool.field_ref(
        "zh/system/GameMIDlet",
        "display",
        "Ljavax/microedition/lcdui/Display;",
    );
    let canvas = pool.field_ref("zh/system/GameMIDlet", "canvas", "Lzh/system/f;");
    let worker = pool.field_ref("zh/system/GameMIDlet", "worker", "Ljava/lang/Thread;");

    let constructor = code_method(
        &mut pool,
        code_name,
        ACC_PUBLIC,
        "<init>",
        "(Lzh/system/d;)V",
        3,
        2,
        {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb7, midlet_init)
                .emit(&[0x2a])
                .reference(0xb3, instance)
                .emit(&[0x2a])
                .reference(0xb8, get_display)
                .reference(0xb3, display)
                .reference(0xbb, canvas_class)
                .emit(&[0x59, 0x2b])
                .reference(0xb7, canvas_init)
                .reference(0xb3, canvas)
                .reference(0xb2, canvas)
                .emit(&[0x04])
                .reference(0xb6, set_fullscreen)
                .emit(&[0xb1]);
            code.finish()
        },
    );
    let start = code_method(
        &mut pool,
        code_name,
        ACC_PROTECTED,
        "startApp",
        "()V",
        4,
        1,
        {
            let mut code = Code::default();
            code.reference(0xb2, display)
                .reference(0xb2, canvas)
                .reference(0xb6, set_current)
                .emit(&[0x2a])
                .reference(0xb4, worker)
                .jump(0xc7, "done");
            code.emit(&[0x2a])
                .reference(0xbb, thread_class)
                .emit(&[0x59])
                .reference(0xb2, canvas)
                .reference(0xb7, thread_init)
                .reference(0xb5, worker)
                .emit(&[0x2a])
                .reference(0xb4, worker)
                .reference(0xb6, thread_start);
            code.label("done").emit(&[0xb1]);
            code.finish()
        },
    );
    let pause = code_method(
        &mut pool,
        code_name,
        ACC_PROTECTED,
        "pauseApp",
        "()V",
        1,
        1,
        {
            let mut code = Code::default();
            code.reference(0xb2, canvas).jump(0xc6, "done");
            code.reference(0xb2, canvas).reference(0xb6, clear_input);
            code.label("done").emit(&[0xb1]);
            code.finish()
        },
    );
    let destroy = code_method(
        &mut pool,
        code_name,
        ACC_PROTECTED,
        "destroyApp",
        "(Z)V",
        1,
        2,
        {
            let mut code = Code::default();
            code.reference(0xb2, canvas).jump(0xc6, "done");
            code.reference(0xb2, canvas).reference(0xb6, stop_canvas);
            code.label("done").emit(&[0xb1]);
            code.finish()
        },
    );
    let console_method = |pool: &mut ConstantPool, name: &str| {
        code_method(
            pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            name,
            "(Ljava/lang/String;)V",
            2,
            1,
            {
                let mut code = Code::default();
                code.reference(0xb2, system_out)
                    .emit(&[0x2a])
                    .reference(0xb6, println)
                    .emit(&[0xb1]);
                code.finish()
            },
        )
    };
    let methods = vec![
        constructor,
        start,
        pause,
        destroy,
        console_method(&mut pool, "cout"),
        console_method(&mut pool, "testout"),
    ];
    let fields = vec![
        field(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "instance",
            "Lzh/system/GameMIDlet;",
        ),
        field(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "display",
            "Ljavax/microedition/lcdui/Display;",
        ),
        field(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "canvas",
            "Lzh/system/f;",
        ),
        field(&mut pool, ACC_PRIVATE, "worker", "Ljava/lang/Thread;"),
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
