use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::ClassFile;

fn probe_scene() -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/ZhScene");
    let super_class = pool.class("zh/system/d");
    let code_name = pool.utf8("Code");
    let super_init = pool.method("zh/system/d", "<init>", "()V");
    let mut constructor_code = vec![0x2a]; // aload_0
    emit_reference(&mut constructor_code, 0xb7, super_init);
    constructor_code.push(0xb1);
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
                code: constructor_code,
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0001,
                name: "a",
                descriptor: "()V",
                max_stack: 0,
                max_locals: 1,
                code: vec![0xb1],
            },
        ),
        code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 0x0001,
                name: "a",
                descriptor: "(Ljavax/microedition/lcdui/Graphics;)V",
                max_stack: 0,
                max_locals: 2,
                code: vec![0xb1],
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
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

fn probe_runner() -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/ZhSystemProbe");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let scene_class = pool.class("fixtures/ZhScene");
    let scene_init = pool.method("fixtures/ZhScene", "<init>", "()V");
    let canvas_class = pool.class("zh/system/f");
    let canvas_init = pool.method("zh/system/f", "<init>", "(Lzh/system/d;)V");
    let key_pressed = pool.method("zh/system/f", "keyPressed", "(I)V");
    let key_released = pool.method("zh/system/f", "keyReleased", "(I)V");
    let held = pool.method("zh/system/f", "a", "(I)Z");
    let pressed = pool.method("zh/system/f", "b", "(I)Z");
    let clear_pressed = pool.method("zh/system/f", "d", "()V");

    let mut code = Vec::new();
    emit_reference(&mut code, 0xbb, scene_class);
    code.push(0x59);
    emit_reference(&mut code, 0xb7, scene_init);
    code.push(0x4b); // scene = new ZhScene()
    emit_reference(&mut code, 0xbb, canvas_class);
    code.extend_from_slice(&[0x59, 0x2a]);
    emit_reference(&mut code, 0xb7, canvas_init);
    code.extend_from_slice(&[0x4c, 0x2b, 0x10, 0x2a]); // canvas = new f(scene); key 42
    emit_reference(&mut code, 0xb6, key_pressed);
    code.extend_from_slice(&[0x03, 0x3d]); // result = 0

    code.extend_from_slice(&[0x1c, 0x2b, 0x10, 0x2a]);
    emit_reference(&mut code, 0xb6, held);
    code.extend_from_slice(&[0x11, 0x03, 0xe8, 0x68, 0x60, 0x3d]);
    // result += held(42) * 1000

    code.extend_from_slice(&[0x1c, 0x2b, 0x10, 0x2a]);
    emit_reference(&mut code, 0xb6, pressed);
    code.extend_from_slice(&[0x10, 0x64, 0x68, 0x60, 0x3d]);
    // result += pressed(42) * 100

    code.push(0x2b);
    emit_reference(&mut code, 0xb6, clear_pressed);
    code.extend_from_slice(&[0x1c, 0x2b, 0x10, 0x2a]);
    emit_reference(&mut code, 0xb6, pressed);
    code.extend_from_slice(&[0x10, 0x0a, 0x68, 0x60, 0x3d]);
    // result += pressed(42) * 10 after clearing edge state

    code.extend_from_slice(&[0x2b, 0x10, 0x2a]);
    emit_reference(&mut code, 0xb6, key_released);
    code.extend_from_slice(&[0x1c, 0x2b, 0x10, 0x2a]);
    emit_reference(&mut code, 0xb6, held);
    code.extend_from_slice(&[0x60, 0xac]); // return result + held(42)

    let run = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 0x0009,
            name: "run",
            descriptor: "()I",
            max_stack: 3,
            max_locals: 3,
            code,
        },
    );
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![run],
        attributes: Vec::new(),
    }
}

#[test]
fn zh_system_canvas_preserves_held_and_edge_key_contracts() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "zh_system_canvas_preserves_held_and_edge_key_contracts"
    )) {
        return;
    }
    let limits = vm::Limits::default();
    let mut program = vm::Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory() {
        program.add_class(&entry.class, &limits).unwrap();
    }
    program.add_class(&probe_scene(), &limits).unwrap();
    program.add_class(&probe_runner(), &limits).unwrap();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    midp::register_natives(program.native_registry_mut()).unwrap();

    let execution = program
        .execute("fixtures/ZhSystemProbe", "run", "()I", limits, false)
        .unwrap();
    assert_eq!(execution.value, Some(vm::Value::Int(1_100)));
}
