use crate::bytecode_builder::Code;

use super::{
    ACC_PROTECTED, ClassFile, append_class, append_code_method, append_method_ref, find_field_ref,
    find_method_ref, method_mut,
};

/// Gives vendor `Canvas` subclasses one narrow hook for adapting callback key
/// codes without changing the profile-owned raw input seen by ordinary
/// applications. The base MIDP implementation preserves the raw code.
pub(crate) fn append_canvas_callback_key_adapter(class: &mut ClassFile) {
    append_code_method(
        class,
        ACC_PROTECTED,
        "__callbackKeyCode",
        "(II)I",
        1,
        3,
        vec![
            0x1b, // iload_1: raw profile code
            0xac, // ireturn
        ],
    );
}

/// Nokia `FullCanvas` exposes Nokia's two selection-key codes even when the
/// class is used as a compatibility adapter on a non-Nokia persona. Direction,
/// fire and numeric callbacks retain the raw profile code so MIDP
/// `getGameAction` continues to use the active device mapping.
pub(crate) fn append_nokia_full_canvas_callback_key_adapter(class: &mut ClassFile) {
    let mut code = Code::default();
    code.emit(&[0x1c, 0x10, 0xfa]) // normalized Nokia SOFT_LEFT
        .jump(0x9f, "normalized");
    code.emit(&[0x1c, 0x10, 0xf9]) // normalized Nokia SOFT_RIGHT
        .jump(0xa0, "raw");
    code.label("normalized").emit(&[0x1c, 0xac]);
    code.label("raw").emit(&[0x1b, 0xac]);
    append_code_method(
        class,
        ACC_PROTECTED,
        "__callbackKeyCode",
        "(II)I",
        2,
        3,
        code.finish(),
    );
}

/// Adds a host-only Canvas dispatch that keeps the profile's raw key code for
/// application callbacks while using a normalized action code for LCDUI's
/// command handling and `GameCanvas` suppression checks.
pub(crate) fn append_canvas_normalized_key_dispatch(class: &mut ClassFile) {
    let pressed_flags = method_mut(class, "__hostKeyPressed", "(II)V").access_flags;
    let released_flags = method_mut(class, "__hostKeyReleased", "(II)V").access_flags;
    let repeated_flags = method_mut(class, "__hostKeyRepeated", "(II)V").access_flags;

    let key_states = find_field_ref(class, "javax/microedition/lcdui/Canvas", "keyStates", "I")
        .expect("Canvas constant pool contains keyStates");
    let callback_key_code = append_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "__callbackKeyCode",
        "(II)I",
    );
    let has_soft_key_command = find_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "__hasSoftKeyCommand",
        "(I)Z",
    )
    .expect("Canvas constant pool contains __hasSoftKeyCommand");
    let dispatch_soft_key = find_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "__dispatchSoftKey",
        "(I)V",
    )
    .expect("Canvas constant pool contains __dispatchSoftKey");
    let suppress_key = find_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "__suppressKey",
        "(I)Z",
    )
    .expect("Canvas constant pool contains __suppressKey");
    let key_pressed = find_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "keyPressed",
        "(I)V",
    )
    .expect("Canvas constant pool contains keyPressed");
    let key_released = find_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "keyReleased",
        "(I)V",
    )
    .expect("Canvas constant pool contains keyReleased");
    let key_repeated = find_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "keyRepeated",
        "(I)V",
    )
    .expect("Canvas constant pool contains keyRepeated");

    for (name, flags, callback) in [
        ("__hostKeyPressed", pressed_flags, key_pressed),
        ("__hostKeyReleased", released_flags, key_released),
        ("__hostKeyRepeated", repeated_flags, key_repeated),
    ] {
        let mut code = Code::default();
        // Locals: this, raw/callback code, GameCanvas state, normalized code.
        code.emit(&[0x2a, 0x1b, 0x1d])
            .reference(0xb6, callback_key_code)
            .emit(&[0x3c, 0x2a, 0x1c])
            .reference(0xb5, key_states);
        code.emit(&[0x2a, 0x1d])
            .reference(0xb6, has_soft_key_command);
        if name == "__hostKeyPressed" {
            code.jump(0x99, "non_command");
            code.emit(&[0x2a, 0x1d])
                .reference(0xb6, dispatch_soft_key)
                .jump(0xa7, "done");
            code.label("non_command");
        } else {
            code.jump(0x9a, "done");
        }
        code.emit(&[0x2a, 0x1d])
            .reference(0xb7, suppress_key)
            .jump(0x9a, "done");
        code.emit(&[0x2a, 0x1b]).reference(0xb6, callback);
        code.label("done").emit(&[0xb1]);
        append_code_method(class, flags, name, "(III)V", 3, 4, code.finish());
    }
}

/// Routes normalized key actions to built-in Screens while retaining raw
/// profile key codes for application Canvases.
pub(crate) fn append_display_normalized_key_dispatch(class: &mut ClassFile) {
    let pressed_flags = method_mut(class, "__hostKeyPressed", "(II)V").access_flags;
    let released_flags = method_mut(class, "__hostKeyReleased", "(II)V").access_flags;
    let repeated_flags = method_mut(class, "__hostKeyRepeated", "(II)V").access_flags;

    let instance = find_field_ref(
        class,
        "javax/microedition/lcdui/Display",
        "INSTANCE",
        "Ljavax/microedition/lcdui/Display;",
    )
    .expect("Display constant pool contains INSTANCE");
    let current = find_field_ref(
        class,
        "javax/microedition/lcdui/Display",
        "current",
        "Ljavax/microedition/lcdui/Displayable;",
    )
    .expect("Display constant pool contains current");
    let canvas = append_class(class, "javax/microedition/lcdui/Canvas");
    let screen = append_class(class, "javax/microedition/lcdui/Screen");
    let canvas_pressed = append_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "__hostKeyPressed",
        "(III)V",
    );
    let canvas_released = append_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "__hostKeyReleased",
        "(III)V",
    );
    let canvas_repeated = append_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "__hostKeyRepeated",
        "(III)V",
    );
    let screen_pressed = find_method_ref(
        class,
        "javax/microedition/lcdui/Screen",
        "__hostKeyPressed",
        "(I)V",
    )
    .expect("Display constant pool contains Screen.__hostKeyPressed");
    let accepts_text_input = find_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__acceptsTextInput",
        "(Ljavax/microedition/lcdui/Displayable;)Z",
    )
    .expect("Display constant pool contains __acceptsTextInput");
    let set_text_input_active = find_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__setTextInputActive",
        "(Z)V",
    )
    .expect("Display constant pool contains __setTextInputActive");

    for (name, flags, callback) in [
        ("__hostKeyPressed", pressed_flags, canvas_pressed),
        ("__hostKeyReleased", released_flags, canvas_released),
        ("__hostKeyRepeated", repeated_flags, canvas_repeated),
    ] {
        let mut code = Code::default();
        let screen_dispatch = name != "__hostKeyReleased";
        code.reference(0xb2, instance)
            .reference(0xb4, current)
            .reference(0xc1, canvas)
            .jump(0x99, if screen_dispatch { "screen" } else { "done" });
        code.reference(0xb2, instance)
            .reference(0xb4, current)
            .reference(0xc0, canvas)
            .emit(&[0x1a, 0x1b, 0x1c]) // raw code, GameCanvas state, normalized code
            .reference(0xb6, callback);
        if screen_dispatch {
            code.jump(0xa7, "done");
            code.label("screen")
                .reference(0xb2, instance)
                .reference(0xb4, current)
                .reference(0xc1, screen)
                .jump(0x99, "done");
            code.reference(0xb2, instance)
                .reference(0xb4, current)
                .reference(0xc0, screen)
                .emit(&[0x1c]) // normalized code
                .reference(0xb6, screen_pressed);
        }
        code.label("done");
        if name == "__hostKeyPressed" {
            code.reference(0xb2, instance)
                .reference(0xb4, current)
                .reference(0xb8, accepts_text_input)
                .reference(0xb8, set_text_input_active);
        }
        code.emit(&[0xb1]);
        append_code_method(class, flags, name, "(III)V", 4, 3, code.finish());
    }
}
