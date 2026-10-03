use crate::bytecode_builder::Code;

use super::{
    ACC_PRIVATE, ACC_PUBLIC, ACC_STATIC, ClassFile, Constant, Member, append_class,
    append_code_method, append_constant, append_field_ref, append_method_ref, method_code_mut,
    method_mut,
};

pub(crate) fn append_host_pointer_dispatch(class: &mut ClassFile) {
    let instance = append_field_ref(
        class,
        "javax/microedition/lcdui/Display",
        "INSTANCE",
        "Ljavax/microedition/lcdui/Display;",
    );
    let current = append_field_ref(
        class,
        "javax/microedition/lcdui/Display",
        "current",
        "Ljavax/microedition/lcdui/Displayable;",
    );
    let canvas = append_class(class, "javax/microedition/lcdui/Canvas");
    for (host_name, callback_name) in [
        ("__hostPointerPressed", "pointerPressed"),
        ("__hostPointerReleased", "pointerReleased"),
        ("__hostPointerDragged", "pointerDragged"),
    ] {
        let callback = append_method_ref(
            class,
            "javax/microedition/lcdui/Canvas",
            callback_name,
            "(II)V",
        );
        let mut code = Code::default();
        code.reference(0xb2, instance)
            .reference(0xb4, current)
            .reference(0xc1, canvas)
            .jump(0x99, "done")
            .reference(0xb2, instance)
            .reference(0xb4, current)
            .reference(0xc0, canvas)
            .emit(&[0x1a, 0x1b]) // pointer coordinates
            .reference(0xb6, callback)
            .label("done")
            .emit(&[0xb1]);
        append_code_method(
            class,
            ACC_PUBLIC | ACC_STATIC,
            host_name,
            "(II)V",
            3,
            2,
            code.finish(),
        );
    }
}

pub(crate) fn guard_display_same_current(class: &mut ClassFile) {
    let current = append_field_ref(
        class,
        "javax/microedition/lcdui/Display",
        "current",
        "Ljavax/microedition/lcdui/Displayable;",
    );
    let code = method_code_mut(
        class,
        "setCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)V",
    );
    let mut guard = Code::default();
    guard
        .emit(&[0x2a])
        .reference(0xb4, current)
        .emit(&[0x2b])
        .jump(0xa6, "body")
        .emit(&[0xb1])
        .label("body");
    let mut guarded = guard.finish();
    let prefix_len = u16::try_from(guarded.len()).expect("bootstrap guard fits u16");
    guarded.extend_from_slice(&code.code);
    code.code = guarded;
    for handler in &mut code.exception_table {
        handler.start_pc = handler.start_pc.saturating_add(prefix_len);
        handler.end_pc = handler.end_pc.saturating_add(prefix_len);
        handler.handler_pc = handler.handler_pc.saturating_add(prefix_len);
    }
}

pub(crate) fn defer_display_transition(class: &mut ClassFile) {
    let apply_name = append_constant(class, Constant::Utf8("__applyCurrent".to_owned()));
    method_mut(
        class,
        "setCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)V",
    )
    .name_index = apply_name;

    let current_refs = class
        .constant_pool
        .iter()
        .enumerate()
        .filter_map(|(index, constant)| {
            let Constant::Fieldref {
                class_index,
                name_and_type_index,
            } = constant.as_ref()?
            else {
                return None;
            };
            if class.class_name(*class_index) != Some("javax/microedition/lcdui/Display") {
                return None;
            }
            let Some(Constant::NameAndType {
                name_index,
                descriptor_index,
            }) = class
                .constant_pool
                .get(usize::from(*name_and_type_index))
                .and_then(Option::as_ref)
            else {
                return None;
            };
            (class.utf8(*name_index) == Some("current")
                && class.utf8(*descriptor_index) == Some("Ljavax/microedition/lcdui/Displayable;"))
            .then(|| u16::try_from(index).expect("constant pool index fits u16"))
        })
        .collect::<Vec<_>>();
    let applied_name = append_constant(class, Constant::Utf8("appliedCurrent".to_owned()));
    let displayable_descriptor = append_constant(
        class,
        Constant::Utf8("Ljavax/microedition/lcdui/Displayable;".to_owned()),
    );
    class.fields.push(Member {
        access_flags: ACC_PRIVATE,
        name_index: applied_name,
        descriptor_index: displayable_descriptor,
        attributes: Vec::new(),
    });
    let applied_current = append_field_ref(
        class,
        "javax/microedition/lcdui/Display",
        "appliedCurrent",
        "Ljavax/microedition/lcdui/Displayable;",
    );
    let apply_code = method_code_mut(
        class,
        "__applyCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)V",
    );
    for instruction in bytecode::decode(&apply_code.code)
        .expect("generated Display.__applyCurrent bytecode must remain valid")
    {
        if matches!(instruction.opcode, 0xb4 | 0xb5) {
            let referenced = u16::from_be_bytes([instruction.operands[0], instruction.operands[1]]);
            if current_refs.contains(&referenced) {
                apply_code.code[instruction.offset + 1..instruction.offset + 3]
                    .copy_from_slice(&applied_current.to_be_bytes());
            }
        }
    }

    let is_current_code = method_code_mut(
        class,
        "__isCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)Z",
    );
    let mut patched_is_current = 0;
    for instruction in bytecode::decode(&is_current_code.code)
        .expect("generated Display.__isCurrent bytecode must remain valid")
    {
        if instruction.opcode != 0xb4 {
            continue;
        }
        let referenced = u16::from_be_bytes([instruction.operands[0], instruction.operands[1]]);
        if current_refs.contains(&referenced) {
            is_current_code.code[instruction.offset + 1..instruction.offset + 3]
                .copy_from_slice(&applied_current.to_be_bytes());
            patched_is_current += 1;
        }
    }
    assert_eq!(
        patched_is_current, 1,
        "Display.__isCurrent must compare the applied display target"
    );

    let transition = append_class(class, "javax/microedition/lcdui/DisplayTransition");
    let transition_init = append_method_ref(
        class,
        "javax/microedition/lcdui/DisplayTransition",
        "<init>",
        "(Ljavax/microedition/lcdui/Display;Ljavax/microedition/lcdui/Displayable;)V",
    );
    let call_serially = append_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "callSerially",
        "(Ljava/lang/Runnable;)V",
    );
    let accepts_text_input = append_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__acceptsTextInput",
        "(Ljavax/microedition/lcdui/Displayable;)Z",
    );
    let set_text_input_active = append_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__setTextInputActive",
        "(Z)V",
    );
    append_code_method(
        class,
        ACC_PRIVATE,
        "__shouldDeferCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)Z",
        1,
        2,
        vec![0x03, 0xac], // iconst_0; ireturn (VM compatibility intrinsic overrides this)
    );
    let should_defer = append_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__shouldDeferCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)Z",
    );
    let apply_current = append_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__applyCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)V",
    );
    let current = append_field_ref(
        class,
        "javax/microedition/lcdui/Display",
        "current",
        "Ljavax/microedition/lcdui/Displayable;",
    );
    let mut code = Code::default();
    code.emit(&[0x2a, 0x2b])
        .reference(0xb5, current) // observable requested target
        .emit(&[0x2b])
        .reference(0xb8, accepts_text_input)
        .reference(0xb8, set_text_input_active)
        .emit(&[0x2a, 0x2b])
        .reference(0xb7, should_defer)
        .jump(0x9a, "defer")
        .emit(&[0x2a, 0x2b])
        .reference(0xb6, apply_current)
        .emit(&[0xb1]);
    // Constructor-reentrant transitions run on the Java event thread.
    code.label("defer")
        .emit(&[0x2a]) // Display.callSerially receiver
        .reference(0xbb, transition)
        .emit(&[0x59, 0x2a, 0x2b])
        .reference(0xb7, transition_init)
        .reference(0xb6, call_serially)
        .emit(&[0xb1]);
    append_code_method(
        class,
        ACC_PUBLIC,
        "setCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)V",
        5,
        2,
        code.finish(),
    );
}
