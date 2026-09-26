use crate::bytecode_builder::Code;

use super::{ClassFile, append_method_ref, find_method_ref, method_code_mut};

pub(crate) fn separate_canvas_serial_dispatch(class: &mut ClassFile) {
    let drain_serial = find_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__drainSerial",
        "()V",
    )
    .expect("Canvas constant pool contains Display.__drainSerial");
    let is_current = append_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__isCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)Z",
    );
    // append_canvas_size_change_dispatch builds the final __hostIdle body
    // after this pass, so only the surviving serviceRepaints body needs its
    // legacy synchronous drains removed here.
    let code = method_code_mut(class, "serviceRepaints", "()V");
    let instructions = bytecode::decode(&code.code)
        .expect("generated Canvas.serviceRepaints bytecode must remain valid");
    let mut patched = 0;
    for instruction in &instructions {
        if instruction.opcode == 0xb8 && *instruction.operands == drain_serial.to_be_bytes() {
            code.code[instruction.offset..instruction.offset + 3].fill(0x00); // nop
            patched += 1;
        }
    }
    assert_eq!(
        patched, 2,
        "Canvas.serviceRepaints must have two synchronous serial drains"
    );

    // A repaint requested by paint() belongs to the next display turn. If
    // serviceRepaints() immediately loops over that new request, a Canvas that
    // stages a loading frame from paint() can keep the caller inside the paint
    // pump forever and never advance its loading state machine.
    let mut patched = 0;
    for instruction in instructions {
        if instruction.opcode != 0xa7 || instruction.operands.len() != 2 {
            continue;
        }
        let branch = i16::from_be_bytes([instruction.operands[0], instruction.operands[1]]);
        if branch >= 0 {
            continue;
        }
        code.code[instruction.offset..instruction.offset + 3].copy_from_slice(&[0xb1, 0x00, 0x00]);
        patched += 1;
    }
    assert_eq!(
        patched, 1,
        "Canvas.serviceRepaints must have one paint-drain loop back edge"
    );

    // MIDP only services repaint requests while a Canvas is visible. Some
    // games call serviceRepaints() while constructing their Canvas and rely on
    // the device deferring paint() until Display.setCurrent(canvas).
    let mut guard = Code::default();
    guard
        .emit(&[0x2a])
        .reference(0xb8, is_current)
        .jump(0x9a, "body")
        .emit(&[0xb1])
        .label("body");
    code.code.splice(0..0, guard.finish());
}

pub(crate) fn limit_display_serial_dispatch_to_one_event(class: &mut ClassFile) {
    let code = method_code_mut(class, "drainSerial", "()V");
    let mut patched = 0;
    for instruction in bytecode::decode(&code.code)
        .expect("generated Display.drainSerial bytecode must remain valid")
    {
        if instruction.opcode != 0xa7 {
            continue;
        }
        let branch = i16::from_be_bytes([instruction.operands[0], instruction.operands[1]]);
        if branch < 0 {
            code.code[instruction.offset..instruction.offset + 3].fill(0x00);
            patched += 1;
        }
    }
    assert_eq!(
        patched, 1,
        "Display.drainSerial must have one callback-loop back edge"
    );
}

pub(crate) fn isolate_display_serial_callback_exceptions(class: &mut ClassFile) {
    let code = method_code_mut(class, "drainSerial", "()V");
    let handler = code
        .exception_table
        .iter()
        .find(|handler| handler.catch_type == 0)
        .expect("Display.drainSerial resets dispatch state through a catch-all handler");
    let throw_offset = usize::from(handler.handler_pc) + 7;
    assert_eq!(
        code.code.get(throw_offset - 1..=throw_offset),
        Some([0x2d, 0xbf].as_slice()),
        "Display.drainSerial catch-all must end with aload_3; athrow"
    );
    // Display callbacks run on the implementation-owned event dispatcher.
    // A broken application callback must not unwind through the AMS host
    // driver and terminate the whole MIDlet. The VM still records the caught
    // Throwable in its bounded caught-exception diagnostics.
    code.code[throw_offset - 1] = 0x00; // nop
    code.code[throw_offset] = 0xb1; // return
}
