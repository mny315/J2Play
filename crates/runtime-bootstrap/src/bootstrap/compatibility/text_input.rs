use crate::bytecode_builder::Code;

use super::{
    ACC_PRIVATE, ACC_PUBLIC, ACC_STATIC, ClassFile, append_class, append_code_method,
    append_field_ref, append_method_ref, find_field_ref, find_method_ref, method_code_mut,
};

pub(crate) fn append_host_text_input_dispatch(class: &mut ClassFile) {
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
    let text_box = append_class(class, "javax/microedition/lcdui/TextBox");
    let callback = append_method_ref(
        class,
        "javax/microedition/lcdui/TextBox",
        "__hostTextInput",
        "(I)V",
    );
    let form = append_class(class, "javax/microedition/lcdui/Form");
    let form_callback = append_method_ref(
        class,
        "javax/microedition/lcdui/Form",
        "__hostTextInput",
        "(I)V",
    );
    let mut code = Code::default();
    for (target, callback, next) in [(text_box, callback, "form"), (form, form_callback, "done")] {
        code.reference(0xb2, instance)
            .reference(0xb4, current)
            .reference(0xc1, target)
            .jump(0x99, next);
        code.reference(0xb2, instance)
            .reference(0xb4, current)
            .reference(0xc0, target)
            .emit(&[0x1a])
            .reference(0xb6, callback);
        if target == text_box {
            code.emit(&[0xb1]);
        }
        code.label(next);
    }
    code.emit(&[0xb1]);
    append_code_method(
        class,
        ACC_PUBLIC | ACC_STATIC,
        "__hostTextInput",
        "(I)V",
        2,
        1,
        code.finish(),
    );
}

pub(crate) fn append_display_text_input_target_check(class: &mut ClassFile) {
    let text_box = append_class(class, "javax/microedition/lcdui/TextBox");
    let form = append_class(class, "javax/microedition/lcdui/Form");
    let form_accepts = append_method_ref(
        class,
        "javax/microedition/lcdui/Form",
        "__acceptsTextInput",
        "()Z",
    );
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xc1, text_box)
        .jump(0x99, "form")
        .emit(&[0x04, 0xac]);
    code.label("form")
        .emit(&[0x2a])
        .reference(0xc1, form)
        .jump(0x99, "reject");
    code.emit(&[0x2a])
        .reference(0xc0, form)
        .reference(0xb6, form_accepts)
        .emit(&[0xac]);
    code.label("reject").emit(&[0x03, 0xac]);
    append_code_method(
        class,
        ACC_PRIVATE | ACC_STATIC,
        "__acceptsTextInput",
        "(Ljavax/microedition/lcdui/Displayable;)Z",
        1,
        1,
        code.finish(),
    );
}

pub(crate) fn refresh_text_input_after_host_activity(class: &mut ClassFile) {
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
    let accepts = append_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__acceptsTextInput",
        "(Ljavax/microedition/lcdui/Displayable;)Z",
    );
    let set_active = append_method_ref(
        class,
        "javax/microedition/lcdui/Display",
        "__setTextInputActive",
        "(Z)V",
    );
    let mut refresh = Code::default();
    refresh
        .reference(0xb2, instance)
        .reference(0xb4, current)
        .reference(0xb8, accepts)
        .reference(0xb8, set_active)
        .emit(&[0xb1]);
    let refresh = refresh.finish();
    for (name, descriptor) in [("__hostKeyPressed", "(II)V"), ("__hostIdle", "()V")] {
        let code = method_code_mut(class, name, descriptor);
        assert_eq!(code.code.pop(), Some(0xb1));
        code.code.extend_from_slice(&refresh);
    }
}

pub(crate) fn append_form_host_input(class: &mut ClassFile) {
    let selected = find_field_ref(class, "javax/microedition/lcdui/Form", "selected", "I")
        .expect("Form constant pool contains selected");
    let size = append_method_ref(class, "javax/microedition/lcdui/Form", "size", "()I");
    let get = find_method_ref(
        class,
        "javax/microedition/lcdui/Form",
        "get",
        "(I)Ljavax/microedition/lcdui/Item;",
    )
    .expect("Form constant pool contains get");
    let text_field = append_class(class, "javax/microedition/lcdui/TextField");
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb6, size)
        .jump(0x99, "reject");
    code.emit(&[0x2a, 0x2a])
        .reference(0xb4, selected)
        .reference(0xb6, get)
        .reference(0xc1, text_field)
        .emit(&[0xac]);
    code.label("reject").emit(&[0x03, 0xac]);
    append_code_method(class, 0, "__acceptsTextInput", "()Z", 2, 1, code.finish());
    let accepts = append_method_ref(
        class,
        "javax/microedition/lcdui/Form",
        "__acceptsTextInput",
        "()Z",
    );
    let key_pressed = append_method_ref(
        class,
        "javax/microedition/lcdui/Form",
        "__keyPressed",
        "(I)V",
    );
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb6, accepts)
        .jump(0x99, "done");
    code.emit(&[0x2a, 0x1b]).reference(0xb6, key_pressed);
    code.label("done").emit(&[0xb1]);
    append_code_method(class, 0, "__hostTextInput", "(I)V", 2, 2, code.finish());
}

pub(crate) fn append_text_box_host_input(class: &mut ClassFile) {
    let dispatch_command = append_method_ref(
        class,
        "javax/microedition/lcdui/TextBox",
        "__dispatchSoftKey",
        "(I)V",
    );
    let code = method_code_mut(class, "__keyPressed", "(I)V");
    let mut with_submit = Code::default();
    with_submit
        .emit(&[0x1b, 0x10, 0xfb]) // normalized Fire
        .jump(0xa0, "edit");
    with_submit
        .emit(&[0x2a, 0x10, 0xfa]) // activate the primary screen command
        .reference(0xb6, dispatch_command)
        .emit(&[0xb1]);
    with_submit.label("edit").emit(&code.code);
    code.code = with_submit.finish();
    let field = find_field_ref(
        class,
        "javax/microedition/lcdui/TextBox",
        "field",
        "Ljavax/microedition/lcdui/TextField;",
    )
    .expect("TextBox constant pool contains its TextField");
    let key = find_method_ref(class, "javax/microedition/lcdui/TextField", "__key", "(I)V")
        .expect("TextBox constant pool contains TextField.__key");
    let repaint = find_method_ref(
        class,
        "javax/microedition/lcdui/TextBox",
        "__requestRepaint",
        "()V",
    )
    .expect("TextBox constant pool contains __requestRepaint");
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb4, field)
        .emit(&[0x1b])
        .reference(0xb6, key)
        .emit(&[0x2a])
        .reference(0xb6, repaint)
        .emit(&[0xb1]);
    append_code_method(class, 0, "__hostTextInput", "(I)V", 2, 2, code.finish());
}
