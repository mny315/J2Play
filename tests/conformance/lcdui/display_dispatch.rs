use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{Attribute, ClassFile, Constant, Member};

const DISPLAY: &str = "javax/microedition/lcdui/Display";
const TARGET: &str = "Ljavax/microedition/lcdui/Displayable;";
const HARNESS: &str = "fixtures/DisplayDispatchRace";
const CANVAS: &str = "javax/microedition/lcdui/DispatchCanvas";
const FORM: &str = "javax/microedition/lcdui/DispatchForm";
const TEXT_BOX: &str = "javax/microedition/lcdui/DispatchTextBox";

fn class(pool: Pool, this_class: u16, super_class: u16, methods: Vec<Member>) -> ClassFile {
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

fn receiver(name: &str, parent: &str, id: u8) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(name);
    let super_class = pool.class(parent);
    let code_name = pool.utf8("Code");
    let mut init = vec![0x2a];
    let descriptor = match name {
        FORM => {
            init.push(0x01);
            "(Ljava/lang/String;)V"
        }
        TEXT_BOX => {
            init.extend_from_slice(&[0x01, 0x01, 0x10, 8, 0x03]);
            "(Ljava/lang/String;Ljava/lang/String;II)V"
        }
        _ => "()V",
    };
    emit_reference(&mut init, 0xb7, pool.method(parent, "<init>", descriptor));
    init.push(0xb1);
    let mut methods = vec![code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 1,
            name: "<init>",
            descriptor: "()V",
            max_stack: 5,
            max_locals: 1,
            code: init,
        },
    )];
    let calls = pool.field(HARNESS, "calls", "I");
    let value = pool.field(HARNESS, "value", "I");
    for (name, descriptor, args) in [
        ("__hostIdle", "()V", 0),
        ("__hostKeyPressed", "(I)V", 1),
        ("__hostKeyPressed", "(II)V", 2),
        ("__hostKeyReleased", "(II)V", 2),
        ("__hostKeyRepeated", "(II)V", 2),
        ("__hostKeyPressed", "(III)V", 3),
        ("__hostKeyReleased", "(III)V", 3),
        ("__hostKeyRepeated", "(III)V", 3),
        ("pointerPressed", "(II)V", 2),
        ("pointerReleased", "(II)V", 2),
        ("pointerDragged", "(II)V", 2),
        ("__hostTextInput", "(I)V", 1),
    ] {
        let mut code = Vec::new();
        emit_reference(&mut code, 0xb2, calls);
        code.extend_from_slice(&[0x04, 0x60]);
        emit_reference(&mut code, 0xb3, calls);
        code.extend_from_slice(&[0x10, id]);
        for local in 1..=args {
            code.extend_from_slice(&[0x10, 10, 0x68, 0x1a + local, 0x60]);
        }
        emit_reference(&mut code, 0xb3, value);
        code.push(0xb1);
        methods.push(code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 1,
                name,
                descriptor,
                max_stack: 2,
                max_locals: u16::from(args) + 1,
                code,
            },
        ));
    }
    methods.push(code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 4,
            name: "paint",
            descriptor: "(Ljavax/microedition/lcdui/Graphics;)V",
            max_stack: 0,
            max_locals: 2,
            code: vec![0xb1],
        },
    ));
    class(pool, this_class, super_class, methods)
}

fn new_receiver(code: &mut Vec<u8>, pool: &mut Pool, name: Option<&str>) {
    if let Some(name) = name {
        emit_reference(code, 0xbb, pool.class(name));
        code.push(0x59);
        emit_reference(code, 0xb7, pool.method(name, "<init>", "()V"));
    } else {
        code.push(0x01);
    }
}

#[allow(clippy::too_many_lines)]
fn harness(
    initial: Option<&str>,
    replacement: Option<&str>,
    event: &str,
    descriptor: &str,
) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(HARNESS);
    let super_class = pool.class("javax/microedition/midlet/MIDlet");
    let code_name = pool.utf8("Code");
    let replacement_field = pool.field(HARNESS, "replacement", TARGET);
    let calls = pool.field(HARNESS, "calls", "I");
    let value = pool.field(HARNESS, "value", "I");
    let mut code = Vec::new();
    new_receiver(&mut code, &mut pool, Some(HARNESS));
    emit_reference(
        &mut code,
        0xb8,
        pool.method(
            DISPLAY,
            "getDisplay",
            "(Ljavax/microedition/midlet/MIDlet;)Ljavax/microedition/lcdui/Display;",
        ),
    );
    new_receiver(&mut code, &mut pool, initial);
    emit_reference(
        &mut code,
        0xb6,
        pool.method(
            DISPLAY,
            "setCurrent",
            "(Ljavax/microedition/lcdui/Displayable;)V",
        ),
    );
    new_receiver(&mut code, &mut pool, replacement);
    emit_reference(&mut code, 0xb3, replacement_field);
    for field in [calls, value] {
        code.push(0x03);
        emit_reference(&mut code, 0xb3, field);
    }
    let args = descriptor.bytes().filter(|byte| *byte == b'I').count();
    code.extend((0..args).map(|index| 0x05 + u8::try_from(index).unwrap())); // 2, 3, 4
    emit_reference(&mut code, 0xb8, pool.method(DISPLAY, event, descriptor));
    emit_reference(&mut code, 0xb2, calls);
    code.extend_from_slice(&[0x11, 0x27, 0x10, 0x68]); // * 10000
    emit_reference(&mut code, 0xb2, value);
    code.extend_from_slice(&[0x60, 0xac]);
    let methods = vec![code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 9,
            name: "run",
            descriptor: "()I",
            max_stack: 3,
            max_locals: 0,
            code,
        },
    )];
    let mut methods = methods;
    let mut init = vec![0x2a];
    emit_reference(
        &mut init,
        0xb7,
        pool.method("javax/microedition/midlet/MIDlet", "<init>", "()V"),
    );
    init.push(0xb1);
    methods.push(code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 1,
            name: "<init>",
            descriptor: "()V",
            max_stack: 1,
            max_locals: 1,
            code: init,
        },
    ));
    for (name, descriptor, locals) in [
        ("startApp", "()V", 1),
        ("pauseApp", "()V", 1),
        ("destroyApp", "(Z)V", 2),
    ] {
        methods.push(code_method(
            &mut pool,
            code_name,
            MethodCode {
                flags: 4,
                name,
                descriptor,
                max_stack: 0,
                max_locals: locals,
                code: vec![0xb1],
            },
        ));
    }
    let fields =
        [("replacement", TARGET), ("calls", "I"), ("value", "I")].map(|(name, descriptor)| {
            Member {
                access_flags: 9,
                name_index: pool.utf8(name),
                descriptor_index: pool.utf8(descriptor),
                attributes: Vec::new(),
            }
        });
    let mut result = class(pool, this_class, super_class, methods);
    result.fields = fields.to_vec();
    result
}

/// Substitute a stack-equivalent getter for reads of Display.current in the
/// dispatcher under test. It returns the old receiver but immediately changes
/// the field, deterministically exercising a worker switch after the read.
fn switch_after_read(display: &mut ClassFile, event: &str, descriptor: &str) {
    let current_refs: Vec<_> = display
        .constant_pool
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let Constant::Fieldref {
                class_index,
                name_and_type_index,
            } = entry.as_ref()?
            else {
                return None;
            };
            let Some(Constant::NameAndType { name_index, .. }) =
                display.constant_pool[usize::from(*name_and_type_index)].as_ref()
            else {
                return None;
            };
            (display.class_name(*class_index) == Some(DISPLAY)
                && display.utf8(*name_index) == Some("current"))
            .then(|| u16::try_from(index).unwrap())
        })
        .collect();
    assert!(!current_refs.is_empty());
    let mut pool = Pool {
        entries: std::mem::take(&mut display.constant_pool),
    };
    let getter_descriptor =
        "(Ljavax/microedition/lcdui/Display;)Ljavax/microedition/lcdui/Displayable;";
    let getter = pool.method(DISPLAY, "__readCurrentForTest", getter_descriptor);
    let replacement = pool.field(HARNESS, "replacement", TARGET);
    let code_name = pool.utf8("Code");
    let mut code = vec![0x2a];
    emit_reference(&mut code, 0xb4, current_refs[0]);
    code.extend_from_slice(&[0x4c, 0x2a]);
    emit_reference(&mut code, 0xb2, replacement);
    emit_reference(&mut code, 0xb5, current_refs[0]);
    code.extend_from_slice(&[0x2b, 0xb0]);
    display.methods.push(code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 9,
            name: "__readCurrentForTest",
            descriptor: getter_descriptor,
            max_stack: 2,
            max_locals: 2,
            code,
        },
    ));
    display.constant_pool = pool.entries;
    let method_index = display
        .methods
        .iter()
        .position(|method| {
            display.utf8(method.name_index) == Some(event)
                && display.utf8(method.descriptor_index) == Some(descriptor)
        })
        .unwrap();
    let Attribute::Code(code) = &mut display.methods[method_index].attributes[0] else {
        panic!("dispatcher has bytecode");
    };
    let mut patched = 0;
    for instruction in bytecode::decode(&code.code).unwrap() {
        if instruction.opcode == 0xb4
            && current_refs.contains(&u16::from_be_bytes([
                instruction.operands[0],
                instruction.operands[1],
            ]))
        {
            code.code[instruction.offset] = 0xb8;
            code.code[instruction.offset + 1..instruction.offset + 3]
                .copy_from_slice(&getter.to_be_bytes());
            patched += 1;
        }
    }
    assert!(patched > 0);
}

#[test]
fn display_dispatch_retains_receiver_when_current_changes() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::display_dispatch_retains_receiver_when_current_changes"
    )) {
        return;
    }
    let limits = vm::Limits {
        max_instructions: 100_000,
        ..vm::Limits::default()
    };
    let receivers = [
        receiver(CANVAS, "javax/microedition/lcdui/Canvas", 1),
        receiver(FORM, "javax/microedition/lcdui/Form", 2),
        receiver(TEXT_BOX, "javax/microedition/lcdui/TextBox", 3),
    ];
    for (event, descriptor) in [
        ("__hostIdle", "()V"),
        ("__hostKeyPressed", "(II)V"),
        ("__hostKeyReleased", "(II)V"),
        ("__hostKeyRepeated", "(II)V"),
        ("__hostKeyPressed", "(III)V"),
        ("__hostKeyReleased", "(III)V"),
        ("__hostKeyRepeated", "(III)V"),
        ("__hostPointerPressed", "(II)V"),
        ("__hostPointerReleased", "(II)V"),
        ("__hostPointerDragged", "(II)V"),
        ("__hostTextInput", "(I)V"),
    ] {
        for (initial, replacement) in [
            (Some(CANVAS), Some(FORM)),
            (Some(FORM), Some(CANVAS)),
            (Some(TEXT_BOX), Some(FORM)),
            (Some(FORM), Some(TEXT_BOX)),
            (Some(CANVAS), None),
            (None, Some(FORM)),
        ] {
            let mut program = vm::Program::new();
            for mut entry in runtime_bootstrap::production_bootstrap_inventory() {
                if entry.class.class_name(entry.class.this_class) == Some(DISPLAY) {
                    switch_after_read(&mut entry.class, event, descriptor);
                }
                program.add_class(&entry.class, &limits).unwrap();
            }
            for receiver in &receivers {
                program.add_class(receiver, &limits).unwrap();
            }
            program
                .add_class(&harness(initial, replacement, event, descriptor), &limits)
                .unwrap();
            cldc::register_core_natives(program.native_registry_mut()).unwrap();
            midp::register_natives(program.native_registry_mut()).unwrap();
            let mut host = super::canvas_modes::FrameCapture::default();
            let execution = program
                .execute_with_context(HARNESS, "run", "()I", limits.clone(), false, &mut host)
                .unwrap_or_else(|error| {
                    panic!("{event}{descriptor} {initial:?} -> {replacement:?}: {error:?}")
                });
            let args = descriptor.bytes().filter(|byte| *byte == b'I').count();
            let (id, delivered_args): (i32, Vec<i32>) = match initial {
                Some(CANVAS) if event != "__hostTextInput" => {
                    (1, (2..2 + i32::try_from(args).unwrap()).collect())
                }
                Some(name)
                    if name != CANVAS
                        && !event.starts_with("__hostPointer")
                        && event != "__hostKeyReleased" =>
                {
                    let args = if event == "__hostIdle" {
                        vec![]
                    } else {
                        vec![if descriptor == "(III)V" { 4 } else { 2 }]
                    };
                    (if name == FORM { 2 } else { 3 }, args)
                }
                _ => (0, vec![]),
            };
            let expected = if id == 0 {
                0
            } else {
                10_000
                    + delivered_args
                        .into_iter()
                        .fold(id, |value, arg| value * 10 + arg)
            };
            assert_eq!(
                execution.value,
                Some(vm::Value::Int(expected)),
                "{event}{descriptor} {initial:?} -> {replacement:?}"
            );
        }
    }
}
