use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{ClassFile, Member};

mod text_input;

fn callback_run_code(
    this_class: u16,
    this_init: u16,
    host_callback: u16,
    last_key: u16,
    raw_key: &[u8],
    normalized_key: &[u8],
) -> Vec<u8> {
    let mut code = Vec::new();
    emit_reference(&mut code, 0xbb, this_class); // new fixture
    code.push(0x59); // dup
    emit_reference(&mut code, 0xb7, this_init);
    code.extend_from_slice(&[0x4b, 0x2a]); // astore_0; aload_0
    code.extend_from_slice(raw_key);
    code.push(0x03); // iconst_0: GameCanvas state
    code.extend_from_slice(normalized_key);
    emit_reference(&mut code, 0xb6, host_callback);
    code.push(0x2a); // aload_0
    emit_reference(&mut code, 0xb4, last_key);
    code.push(0xac); // ireturn
    code
}

#[allow(clippy::too_many_lines)]
fn profile_key_canvas_fixture(class_name: &str, suppress_key_events: bool) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(class_name);
    let super_name = if suppress_key_events {
        "javax/microedition/lcdui/game/GameCanvas"
    } else {
        "javax/microedition/lcdui/Canvas"
    };
    let super_class = pool.class(super_name);
    let code_name = pool.utf8("Code");
    let super_init_descriptor = if suppress_key_events { "(Z)V" } else { "()V" };
    let super_init = pool.method(super_name, "<init>", super_init_descriptor);
    let this_init = pool.method(class_name, "<init>", "()V");
    let host_key_pressed = pool.method(
        "javax/microedition/lcdui/Canvas",
        "__hostKeyPressed",
        "(III)V",
    );
    let last_key = pool.field(class_name, "lastKey", "I");

    let mut constructor = vec![0x2a]; // aload_0
    if suppress_key_events {
        constructor.push(0x04); // iconst_1
    }
    emit_reference(&mut constructor, 0xb7, super_init);
    constructor.push(0xb1); // return

    let mut key_pressed = vec![0x2a, 0x1b]; // aload_0; iload_1
    emit_reference(&mut key_pressed, 0xb5, last_key);
    key_pressed.push(0xb1); // return

    let mut run = Vec::new();
    emit_reference(&mut run, 0xbb, this_class); // new fixture
    run.push(0x59); // dup
    emit_reference(&mut run, 0xb7, this_init);
    run.extend_from_slice(&[
        0x4b, // astore_0
        0x2a, // aload_0
        0x10, 0xc5, // bipush -59: Siemens UP raw code
        0x05, // iconst_2: UP_PRESSED state bit
        0x02, // iconst_m1: normalized LCDUI UP
    ]);
    emit_reference(&mut run, 0xb6, host_key_pressed);
    run.push(0x2a); // aload_0
    emit_reference(&mut run, 0xb4, last_key);
    run.push(0xac); // ireturn

    let last_key_name = pool.utf8("lastKey");
    let int_descriptor = pool.utf8("I");
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0001,
                name: "<init>",
                descriptor: "()V",
                max_stack: 2,
                max_locals: 1,
                code: constructor,
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0004,
                name: "paint",
                descriptor: "(Ljavax/microedition/lcdui/Graphics;)V",
                max_stack: 0,
                max_locals: 2,
                code: vec![0xb1],
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0004,
                name: "keyPressed",
                descriptor: "(I)V",
                max_stack: 2,
                max_locals: 2,
                code: key_pressed,
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0009,
                name: "run",
                descriptor: "()I",
                max_stack: 4,
                max_locals: 1,
                code: run,
            },
        ),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: vec![Member {
            access_flags: 0x0002,
            name_index: last_key_name,
            descriptor_index: int_descriptor,
            attributes: Vec::new(),
        }],
        methods,
        attributes: Vec::new(),
    }
}

#[allow(clippy::too_many_lines)]
fn nokia_full_canvas_key_fixture() -> ClassFile {
    const CLASS_NAME: &str = "com/nokia/mid/ui/ProfileFullCanvasKeys";
    const SUPER_NAME: &str = "com/nokia/mid/ui/FullCanvas";

    let mut pool = Pool::new();
    let this_class = pool.class(CLASS_NAME);
    let super_class = pool.class(SUPER_NAME);
    let code_name = pool.utf8("Code");
    let super_init = pool.method(SUPER_NAME, "<init>", "()V");
    let this_init = pool.method(CLASS_NAME, "<init>", "()V");
    let host_key_pressed = pool.method(
        "javax/microedition/lcdui/Canvas",
        "__hostKeyPressed",
        "(III)V",
    );
    let host_key_repeated = pool.method(
        "javax/microedition/lcdui/Canvas",
        "__hostKeyRepeated",
        "(III)V",
    );
    let host_key_released = pool.method(
        "javax/microedition/lcdui/Canvas",
        "__hostKeyReleased",
        "(III)V",
    );
    let last_key = pool.field(CLASS_NAME, "lastKey", "I");
    let last_key_name = pool.utf8("lastKey");
    let int_descriptor = pool.utf8("I");

    let mut constructor = vec![0x2a]; // aload_0
    emit_reference(&mut constructor, 0xb7, super_init);
    constructor.push(0xb1); // return

    let callback = |pool: &mut Pool, name| {
        let mut code = vec![0x2a, 0x1b]; // aload_0; iload_1
        emit_reference(&mut code, 0xb5, last_key);
        code.push(0xb1); // return
        code_method(
            pool,
            code_name,
            MethodCode {
                flags: 0x0004,
                name,
                descriptor: "(I)V",
                max_stack: 2,
                max_locals: 2,
                code,
            },
        )
    };
    let run = |pool: &mut Pool, name, host_callback, raw_key: &[u8], normalized_key: &[u8]| {
        code_method(
            pool,
            code_name,
            MethodCode {
                flags: 0x0009,
                name,
                descriptor: "()I",
                max_stack: 4,
                max_locals: 1,
                code: callback_run_code(
                    this_class,
                    this_init,
                    host_callback,
                    last_key,
                    raw_key,
                    normalized_key,
                ),
            },
        )
    };

    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0001,
                name: "<init>",
                descriptor: "()V",
                max_stack: 1,
                max_locals: 1,
                code: constructor,
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0004,
                name: "paint",
                descriptor: "(Ljavax/microedition/lcdui/Graphics;)V",
                max_stack: 0,
                max_locals: 2,
                code: vec![0xb1],
            },
        ),
        callback(&mut pool, "keyPressed"),
        callback(&mut pool, "keyRepeated"),
        callback(&mut pool, "keyReleased"),
        run(
            &mut pool,
            "runPressed",
            host_key_pressed,
            &[0x02],
            &[0x10, 0xfa],
        ),
        run(
            &mut pool,
            "runRepeated",
            host_key_repeated,
            &[0x10, 0xfc],
            &[0x10, 0xf9],
        ),
        run(
            &mut pool,
            "runReleased",
            host_key_released,
            &[0x02],
            &[0x10, 0xfa],
        ),
        run(
            &mut pool,
            "runDirection",
            host_key_pressed,
            &[0x10, 0xc5],
            &[0x02],
        ),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: vec![Member {
            access_flags: 0x0002,
            name_index: last_key_name,
            descriptor_index: int_descriptor,
            attributes: Vec::new(),
        }],
        methods,
        attributes: Vec::new(),
    }
}

#[allow(clippy::too_many_lines)]
fn profile_soft_key_canvas_fixture() -> ClassFile {
    const CLASS_NAME: &str = "javax/microedition/lcdui/ProfileSoftKeyCanvas";

    let mut pool = Pool::new();
    let this_class = pool.class(CLASS_NAME);
    let super_class = pool.class("javax/microedition/lcdui/Canvas");
    let command_listener = pool.class("javax/microedition/lcdui/CommandListener");
    let command_class = pool.class("javax/microedition/lcdui/Command");
    let command_label = pool.string("accept");
    let code_name = pool.utf8("Code");
    let super_init = pool.method("javax/microedition/lcdui/Canvas", "<init>", "()V");
    let this_init = pool.method(CLASS_NAME, "<init>", "()V");
    let command_init = pool.method(
        "javax/microedition/lcdui/Command",
        "<init>",
        "(Ljava/lang/String;II)V",
    );
    let add_command = pool.method(
        "javax/microedition/lcdui/Displayable",
        "addCommand",
        "(Ljavax/microedition/lcdui/Command;)V",
    );
    let set_listener = pool.method(
        "javax/microedition/lcdui/Displayable",
        "setCommandListener",
        "(Ljavax/microedition/lcdui/CommandListener;)V",
    );
    let host_key_pressed = pool.method(
        "javax/microedition/lcdui/Canvas",
        "__hostKeyPressed",
        "(III)V",
    );
    let command_count = pool.field(CLASS_NAME, "commandCount", "I");

    let mut constructor = vec![0x2a]; // aload_0
    emit_reference(&mut constructor, 0xb7, super_init);
    constructor.push(0x2a); // aload_0
    emit_reference(&mut constructor, 0xbb, command_class);
    constructor.extend_from_slice(&[0x59, 0x12, u8::try_from(command_label).unwrap(), 0x07, 0x03]);
    // dup; ldc "accept"; iconst_4 (Command.OK); iconst_0
    emit_reference(&mut constructor, 0xb7, command_init);
    emit_reference(&mut constructor, 0xb6, add_command);
    constructor.extend_from_slice(&[0x2a, 0x2a]); // aload_0; aload_0
    emit_reference(&mut constructor, 0xb6, set_listener);
    constructor.push(0xb1); // return

    let mut command_action = vec![0x2a, 0x59]; // aload_0; dup
    emit_reference(&mut command_action, 0xb4, command_count);
    command_action.extend_from_slice(&[0x04, 0x60]); // iconst_1; iadd
    emit_reference(&mut command_action, 0xb5, command_count);
    command_action.push(0xb1); // return

    let mut run = Vec::new();
    emit_reference(&mut run, 0xbb, this_class);
    run.push(0x59); // dup
    emit_reference(&mut run, 0xb7, this_init);
    run.extend_from_slice(&[
        0x4b, // astore_0
        0x2a, // aload_0
        0x02, // iconst_m1: Siemens SOFT_LEFT raw code
        0x03, // iconst_0: key state
        0x10, 0xfa, // bipush -6: normalized SOFT_LEFT
    ]);
    emit_reference(&mut run, 0xb6, host_key_pressed);
    run.extend_from_slice(&[
        0x2a, // aload_0
        0x10, 0xfc, // bipush -4: Siemens SOFT_RIGHT raw code
        0x03, // iconst_0: key state
        0x10, 0xf9, // bipush -7: normalized SOFT_RIGHT
    ]);
    emit_reference(&mut run, 0xb6, host_key_pressed);
    run.push(0x2a); // aload_0
    emit_reference(&mut run, 0xb4, command_count);
    run.push(0xac); // ireturn

    let field_name = pool.utf8("commandCount");
    let int_descriptor = pool.utf8("I");
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0001,
                name: "<init>",
                descriptor: "()V",
                max_stack: 6,
                max_locals: 1,
                code: constructor,
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0004,
                name: "paint",
                descriptor: "(Ljavax/microedition/lcdui/Graphics;)V",
                max_stack: 0,
                max_locals: 2,
                code: vec![0xb1],
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0001,
                name: "commandAction",
                descriptor: "(Ljavax/microedition/lcdui/Command;Ljavax/microedition/lcdui/Displayable;)V",
                max_stack: 3,
                max_locals: 3,
                code: command_action,
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0009,
                name: "run",
                descriptor: "()I",
                max_stack: 4,
                max_locals: 1,
                code: run,
            },
        ),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: vec![command_listener],
        fields: vec![Member {
            access_flags: 0x0002,
            name_index: field_name,
            descriptor_index: int_descriptor,
            attributes: Vec::new(),
        }],
        methods,
        attributes: Vec::new(),
    }
}

fn profile_key_program(limits: &vm::Limits) -> vm::Program {
    let mut program = vm::Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display_modes(
        (limits.lcd_width, limits.lcd_height),
        (limits.lcd_normal_width, limits.lcd_normal_height),
    ) {
        program.add_class(&entry.class, limits).unwrap();
    }
    for fixture in [
        profile_key_canvas_fixture("javax/microedition/lcdui/ProfileKeyCanvas", false),
        profile_key_canvas_fixture("javax/microedition/lcdui/ProfileKeyGameCanvas", true),
        profile_soft_key_canvas_fixture(),
        nokia_full_canvas_key_fixture(),
    ] {
        program.add_class(&fixture, limits).unwrap();
    }
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    midp::register_natives(program.native_registry_mut()).unwrap();
    program
}

#[test]
fn ordinary_canvas_receives_the_profiles_raw_key_code() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "ordinary_canvas_receives_the_profiles_raw_key_code"
    )) {
        return;
    }
    let limits = vm::Limits::default();
    let execution = profile_key_program(&limits)
        .execute(
            "javax/microedition/lcdui/ProfileKeyCanvas",
            "run",
            "()I",
            limits,
            false,
        )
        .unwrap();
    assert_eq!(execution.value, Some(vm::Value::Int(-59)));
}

#[test]
fn game_canvas_suppression_uses_the_normalized_action() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "game_canvas_suppression_uses_the_normalized_action"
    )) {
        return;
    }
    let limits = vm::Limits::default();
    let execution = profile_key_program(&limits)
        .execute(
            "javax/microedition/lcdui/ProfileKeyGameCanvas",
            "run",
            "()I",
            limits,
            false,
        )
        .unwrap();
    assert_eq!(execution.value, Some(vm::Value::Int(0)));
}

#[test]
fn canvas_commands_use_normalized_soft_keys_without_rewriting_callbacks() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "canvas_commands_use_normalized_soft_keys_without_rewriting_callbacks"
    )) {
        return;
    }
    let limits = vm::Limits::default();
    let execution = profile_key_program(&limits)
        .execute(
            "javax/microedition/lcdui/ProfileSoftKeyCanvas",
            "run",
            "()I",
            limits,
            false,
        )
        .unwrap();
    let Some(vm::Value::Int(command_count)) = execution.value else {
        panic!("soft-key fixture did not return an int");
    };
    assert!(command_count > 0);
}

#[test]
fn nokia_full_canvas_adapts_soft_keys_across_the_callback_lifecycle_only() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "nokia_full_canvas_adapts_soft_keys_across_the_callback_lifecycle_only"
    )) {
        return;
    }
    for (method, expected) in [
        ("runPressed", -6),
        ("runRepeated", -7),
        ("runReleased", -6),
        ("runDirection", -59),
    ] {
        let limits = vm::Limits::default();
        let execution = profile_key_program(&limits)
            .execute(
                "com/nokia/mid/ui/ProfileFullCanvasKeys",
                method,
                "()I",
                limits,
                false,
            )
            .unwrap();
        assert_eq!(execution.value, Some(vm::Value::Int(expected)), "{method}");
    }
}
