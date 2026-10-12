use crate::*;

#[test]
fn canvas_dispatch_patch_preserves_numeric_operands_that_resemble_branches() {
    let mut canvas = generated_bootstrap_classes()
        .into_iter()
        .find(|class| class.class_name(class.this_class) == Some("javax/microedition/lcdui/Canvas"))
        .unwrap();
    // A valid, unreachable numeric expression contains a7 ff 02 across a
    // sipush operand and iconst_m1. Only the actual goto may be patched.
    let literal = [0x11, 0xa7, 0xff, 0x02, 0x57, 0x57, 0xb1];
    bootstrap::method_code_mut(&mut canvas, "serviceRepaints", "()V")
        .code
        .extend_from_slice(&literal);
    bootstrap::separate_canvas_serial_dispatch(&mut canvas);
    assert!(
        bootstrap::method_code_mut(&mut canvas, "serviceRepaints", "()V")
            .code
            .ends_with(&literal)
    );
}

#[test]
fn serial_callbacks_are_isolated_and_dispatched_once_per_display_event_turn() {
    let classes = production_bootstrap_classes();
    let canvas = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("javax/microedition/lcdui/Canvas"))
        .unwrap();
    let drain_serial = canvas
        .constant_pool
        .iter()
        .enumerate()
        .find_map(|(index, constant)| {
            let Constant::Methodref {
                class_index,
                name_and_type_index,
            } = constant.as_ref()?
            else {
                return None;
            };
            let Constant::NameAndType {
                name_index,
                descriptor_index,
            } = canvas
                .constant_pool
                .get(usize::from(*name_and_type_index))?
                .as_ref()?
            else {
                return None;
            };
            (canvas.class_name(*class_index) == Some("javax/microedition/lcdui/Display")
                && canvas.utf8(*name_index) == Some("__drainSerial")
                && canvas.utf8(*descriptor_index) == Some("()V"))
            .then(|| u16::try_from(index).unwrap().to_be_bytes())
        })
        .unwrap();
    for name in ["serviceRepaints", "__hostIdle"] {
        let method = canvas
            .methods
            .iter()
            .find(|method| {
                canvas.utf8(method.name_index) == Some(name)
                    && canvas.utf8(method.descriptor_index) == Some("()V")
            })
            .unwrap();
        let Attribute::Code(code) = &method.attributes[0] else {
            panic!("Canvas.{name} must have bytecode");
        };
        assert!(
            !code
                .code
                .windows(3)
                .any(|bytes| bytes == [0xb8, drain_serial[0], drain_serial[1]]),
            "Canvas.{name} must leave serial dispatch to Display.__hostIdle"
        );
    }

    let display = classes
        .iter()
        .find(|class| {
            class.class_name(class.this_class) == Some("javax/microedition/lcdui/Display")
        })
        .unwrap();
    let method = display
        .methods
        .iter()
        .find(|method| {
            display.utf8(method.name_index) == Some("drainSerial")
                && display.utf8(method.descriptor_index) == Some("()V")
        })
        .unwrap();
    let Attribute::Code(code) = &method.attributes[0] else {
        panic!("Display.drainSerial must have bytecode");
    };
    assert!(
        !code
            .code
            .windows(3)
            .any(|bytes| { bytes[0] == 0xa7 && i16::from_be_bytes([bytes[1], bytes[2]]) < 0 })
    );
    let handler = code
        .exception_table
        .iter()
        .find(|handler| handler.catch_type == 0)
        .unwrap();
    let handler_start = usize::from(handler.handler_pc);
    assert_eq!(
        &code.code[handler_start..handler_start + 4],
        &[0x4e, 0x2a, 0x03, 0xb5],
        "callback failure handler stores the exception and resets dispatching"
    );
    assert_eq!(
        &code.code[handler_start + 6..handler_start + 9],
        &[0x00, 0xb1, 0xb1],
        "callback failure returns to the host event loop instead of unwinding the AMS"
    );
}
