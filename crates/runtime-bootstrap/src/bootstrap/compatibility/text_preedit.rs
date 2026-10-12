use crate::bytecode_builder::Code;

use super::{
    ACC_PRIVATE, ClassFile, Constant, Member, append_code_method, append_constant,
    append_field_ref, append_method_ref, find_field_ref, method_code_mut,
};

const FIELD: &str = "javax/microedition/lcdui/TextField";
const STRING: &str = "java/lang/String";

pub(crate) fn implement_text_preedit(class: &mut ClassFile) {
    for (name, descriptor) in [
        ("__preedit", "Ljava/lang/String;"),
        ("__anchor", "I"),
        ("__cursor", "I"),
    ] {
        let name_index = append_constant(class, Constant::Utf8(name.to_owned()));
        let descriptor_index = append_constant(class, Constant::Utf8(descriptor.to_owned()));
        class.fields.push(Member {
            access_flags: ACC_PRIVATE,
            name_index,
            descriptor_index,
            attributes: Vec::new(),
        });
    }
    append_display_text(class);
    append_host_edit(class);
    append_editor_paint(class);
    let edit = append_method_ref(class, FIELD, "__hostEdit", "(I)V");
    let preedit = append_field_ref(class, FIELD, "__preedit", "Ljava/lang/String;");
    let host_operation = append_constant(class, Constant::Integer(0x10000));
    let mut prefix = Code::default();
    prefix
        .emit(&[0x1b])
        .reference(0x13, host_operation)
        .jump(0xa1, "ordinary");
    prefix
        .emit(&[0x2a, 0x1b])
        .reference(0xb7, edit)
        .emit(&[0xb1]);
    prefix
        .label("ordinary")
        .emit(&[0x2a, 0x01])
        .reference(0xb5, preedit);
    let prefix = prefix.finish();
    let code = method_code_mut(class, "__key", "(I)V");
    // Keep the UNEDITABLE guard before both host operations and phone keys.
    code.code.splice(12..12, prefix);
    // A programmatic edit invalidates the pending preview as well.
    for (name, descriptor) in [
        ("setString", "(Ljava/lang/String;)V"),
        ("setConstraints", "(I)V"),
        ("setMaxSize", "(I)I"),
    ] {
        let code = method_code_mut(class, name, descriptor);
        assert!(code.exception_table.is_empty());
        let [high, low] = preedit.to_be_bytes();
        code.code.splice(..0, [0x2a, 0x01, 0xb5, high, low]);
    }
}

fn append_display_text(class: &mut ClassFile) {
    let text = find_field_ref(class, FIELD, "text", "Ljava/lang/String;").unwrap();
    let preedit = append_field_ref(class, FIELD, "__preedit", "Ljava/lang/String;");
    let caret = append_field_ref(class, FIELD, "caret", "I");
    let range = append_method_ref(class, STRING, "substring", "(II)Ljava/lang/String;");
    let tail = append_method_ref(class, STRING, "substring", "(I)Ljava/lang/String;");
    let concat = append_method_ref(
        class,
        STRING,
        "concat",
        "(Ljava/lang/String;)Ljava/lang/String;",
    );
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb4, preedit)
        .jump(0xc6, "plain");
    code.emit(&[0x2a])
        .reference(0xb4, text)
        .emit(&[0x03, 0x2a])
        .reference(0xb4, caret)
        .reference(0xb6, range);
    code.emit(&[0x2a])
        .reference(0xb4, preedit)
        .reference(0xb6, concat);
    code.emit(&[0x2a])
        .reference(0xb4, text)
        .emit(&[0x2a])
        .reference(0xb4, caret)
        .reference(0xb6, tail)
        .reference(0xb6, concat)
        .emit(&[0xb0]);
    code.label("plain")
        .emit(&[0x2a])
        .reference(0xb4, text)
        .emit(&[0xb0]);
    append_code_method(
        class,
        ACC_PRIVATE,
        "__displayText",
        "()Ljava/lang/String;",
        4,
        1,
        code.finish(),
    );
    let display_text = append_method_ref(class, FIELD, "__displayText", "()Ljava/lang/String;");
    let code = method_code_mut(
        class,
        "__paint",
        "(Ljavax/microedition/lcdui/Graphics;IIIZ)V",
    );
    let mut replaced = 0;
    for instruction in bytecode::decode(&code.code).unwrap() {
        let pc = instruction.offset;
        if code.code[pc] == 0xb4 && code.code[pc + 1..pc + 3] == text.to_be_bytes() {
            code.code[pc] = 0xb7;
            code.code[pc + 1..pc + 3].copy_from_slice(&display_text.to_be_bytes());
            replaced += 1;
        }
    }
    // Password masking must use the preview too, never expose plain preedit.
    assert_eq!(replaced, 2);
}

#[allow(clippy::too_many_lines)]
fn append_host_edit(class: &mut ClassFile) {
    let preedit = append_field_ref(class, FIELD, "__preedit", "Ljava/lang/String;");
    let text = append_field_ref(class, FIELD, "text", "Ljava/lang/String;");
    let caret = append_field_ref(class, FIELD, "caret", "I");
    let anchor = append_field_ref(class, FIELD, "__anchor", "I");
    let cursor = append_field_ref(class, FIELD, "__cursor", "I");
    let empty_utf8 = append_constant(class, Constant::Utf8(String::new()));
    let empty = append_constant(
        class,
        Constant::String {
            string_index: empty_utf8,
        },
    );
    let length = append_method_ref(class, STRING, "length", "()I");
    let character = append_method_ref(class, STRING, "valueOf", "(C)Ljava/lang/String;");
    let concat = append_method_ref(
        class,
        STRING,
        "concat",
        "(Ljava/lang/String;)Ljava/lang/String;",
    );
    let minimum = append_method_ref(class, "java/lang/Math", "min", "(II)I");
    let maximum = append_method_ref(class, "java/lang/Math", "max", "(II)I");
    let delete = append_method_ref(class, FIELD, "delete", "(II)V");
    let invalidate = append_method_ref(class, FIELD, "invalidate", "()V");
    let mut code = Code::default();
    code.emit(&[0x1b, 0x10, 16, 0x7c, 0x3d, 0x1b, 0x92, 0x3e]); // operation, unsigned low word
    for (operation, label) in [
        (1, "reset"),
        (2, "append"),
        (3, "selection"),
        (4, "selection"),
        (5, "backward"),
        (6, "forward"),
        (7, "move"),
    ] {
        code.emit(&[0x1c, 0x10, operation]).jump(0x9f, label);
    }
    code.emit(&[0xb1]);
    code.label("reset")
        .emit(&[0x2a, 0x01])
        .reference(0xb5, preedit);
    code.emit(&[0x2a, 0x03])
        .reference(0xb5, anchor)
        .emit(&[0x2a, 0x03])
        .reference(0xb5, cursor)
        .jump(0xa7, "invalidate");
    code.label("append")
        .emit(&[0x2a])
        .reference(0xb4, preedit)
        .jump(0xc7, "bounded");
    code.emit(&[0x2a])
        .reference(0x13, empty)
        .reference(0xb5, preedit);
    code.label("bounded")
        .emit(&[0x2a])
        .reference(0xb4, preedit)
        .reference(0xb6, length)
        .emit(&[0x11, 0x10, 0x00])
        .jump(0xa2, "done");
    code.emit(&[0x2a, 0x2a])
        .reference(0xb4, preedit)
        .emit(&[0x1d, 0x92])
        .reference(0xb8, character)
        .reference(0xb6, concat)
        .reference(0xb5, preedit)
        .emit(&[0xb1]);
    code.label("selection")
        .emit(&[0x2a])
        .reference(0xb4, preedit)
        .jump(0xc6, "done");
    code.emit(&[0x1d, 0x2a])
        .reference(0xb4, preedit)
        .reference(0xb6, length)
        .reference(0xb8, minimum)
        .emit(&[0x3e]);
    code.emit(&[0x1c, 0x06]).jump(0xa0, "cursor");
    code.emit(&[0x2a, 0x1d])
        .reference(0xb5, anchor)
        .emit(&[0xb1]);
    code.label("cursor")
        .emit(&[0x2a, 0x1d])
        .reference(0xb5, cursor)
        .jump(0xa7, "invalidate");
    code.label("backward")
        .emit(&[0x1d, 0x10, 64])
        .reference(0xb8, minimum)
        .emit(&[0x2a])
        .reference(0xb4, caret)
        .reference(0xb8, minimum)
        .emit(&[0x3e]);
    code.emit(&[0x2a, 0x2a])
        .reference(0xb4, caret)
        .emit(&[0x1d, 0x64, 0x1d])
        .reference(0xb6, delete)
        .emit(&[0xb1]);
    code.label("forward")
        .emit(&[0x1d, 0x10, 64])
        .reference(0xb8, minimum);
    code.emit(&[0x2a])
        .reference(0xb4, text)
        .reference(0xb6, length)
        .emit(&[0x2a])
        .reference(0xb4, caret)
        .emit(&[0x64])
        .reference(0xb8, minimum)
        .emit(&[0x3e]);
    code.emit(&[0x2a, 0x2a])
        .reference(0xb4, caret)
        .emit(&[0x1d])
        .reference(0xb6, delete)
        .emit(&[0xb1]);
    code.label("move")
        .emit(&[0x2a, 0x03, 0x2a])
        .reference(0xb4, text)
        .reference(0xb6, length);
    code.emit(&[0x2a])
        .reference(0xb4, caret)
        .emit(&[0x1d, 0x93, 0x60])
        .reference(0xb8, minimum)
        .reference(0xb8, maximum)
        .reference(0xb5, caret);
    code.label("invalidate")
        .emit(&[0x2a])
        .reference(0xb6, invalidate);
    code.label("done").emit(&[0xb1]);
    append_code_method(
        class,
        ACC_PRIVATE,
        "__hostEdit",
        "(I)V",
        5,
        4,
        code.finish(),
    );
}

#[allow(clippy::too_many_lines)]
fn append_editor_paint(class: &mut ClassFile) {
    const GRAPHICS: &str = "javax/microedition/lcdui/Graphics";
    const FONT: &str = "javax/microedition/lcdui/Font";
    let caret = append_field_ref(class, FIELD, "caret", "I");
    let preedit = append_field_ref(class, FIELD, "__preedit", "Ljava/lang/String;");
    let anchor = append_field_ref(class, FIELD, "__anchor", "I");
    let cursor = append_field_ref(class, FIELD, "__cursor", "I");
    let length = append_method_ref(class, STRING, "length", "()I");
    let width = append_method_ref(class, FONT, "substringWidth", "(Ljava/lang/String;II)I");
    let height = append_method_ref(class, FONT, "getHeight", "()I");
    let minimum = append_method_ref(class, "java/lang/Math", "min", "(II)I");
    let maximum = append_method_ref(class, "java/lang/Math", "max", "(II)I");
    let clip = append_method_ref(class, GRAPHICS, "clipRect", "(IIII)V");
    let restore_clip = append_method_ref(class, GRAPHICS, "setClip", "(IIII)V");
    let draw_text = append_method_ref(class, GRAPHICS, "drawString", "(Ljava/lang/String;III)V");
    let draw_line = append_method_ref(class, GRAPHICS, "drawLine", "(IIII)V");
    let fill = append_method_ref(class, GRAPHICS, "fillRect", "(IIII)V");
    let get_color = append_method_ref(class, GRAPHICS, "getColor", "()I");
    let set_color = append_method_ref(class, GRAPHICS, "setColor", "(I)V");
    let blue = append_constant(class, Constant::Integer(0x0024_4b8f));
    let mut code = Code::default();
    // Locals: this, graphics, display text, font, x, y, width, focused;
    // 8/9 caret/anchor pixels, 10 scroll, 11..14 previous clip, 15 color.
    code.emit(&[0x03, 0x15, 6])
        .reference(0xb8, maximum)
        .emit(&[0x36, 6]);
    for (field, target, label) in [(cursor, 8, "caret"), (anchor, 9, "anchor")] {
        code.emit(&[0x2a])
            .reference(0xb4, caret)
            .emit(&[0x36, target]);
        code.emit(&[0x2a])
            .reference(0xb4, preedit)
            .jump(0xc6, label);
        code.emit(&[0x15, target, 0x2a])
            .reference(0xb4, field)
            .emit(&[0x60, 0x36, target]);
        code.label(label)
            .emit(&[0x2d, 0x2c, 0x03, 0x15, target, 0x2c])
            .reference(0xb6, length)
            .reference(0xb8, minimum)
            .reference(0xb6, width)
            .emit(&[0x36, target]);
    }
    code.emit(&[0x03, 0x15, 8, 0x15, 6, 0x64, 0x04, 0x60])
        .reference(0xb8, maximum)
        .emit(&[0x36, 10]);
    for (name, local) in [
        ("getClipX", 11),
        ("getClipY", 12),
        ("getClipWidth", 13),
        ("getClipHeight", 14),
    ] {
        let method = append_method_ref(class, GRAPHICS, name, "()I");
        code.emit(&[0x2b])
            .reference(0xb6, method)
            .emit(&[0x36, local]);
    }
    code.emit(&[0x2b, 0x15, 4, 0x15, 5, 0x15, 6, 0x2d])
        .reference(0xb6, height)
        .reference(0xb6, clip);
    code.emit(&[0x15, 7]).jump(0x99, "text");
    code.emit(&[0x15, 8, 0x15, 9]).jump(0x9f, "text");
    code.emit(&[0x2b])
        .reference(0xb6, get_color)
        .emit(&[0x36, 15]);
    code.emit(&[0x2b])
        .reference(0x13, blue)
        .reference(0xb6, set_color);
    code.emit(&[0x2b, 0x15, 4, 0x15, 8, 0x15, 9])
        .reference(0xb8, minimum)
        .emit(&[0x60, 0x15, 10, 0x64, 0x15, 5]);
    code.emit(&[0x15, 8, 0x15, 9])
        .reference(0xb8, maximum)
        .emit(&[0x15, 8, 0x15, 9])
        .reference(0xb8, minimum)
        .emit(&[0x64, 0x2d])
        .reference(0xb6, height)
        .reference(0xb6, fill);
    code.emit(&[0x2b, 0x15, 15]).reference(0xb6, set_color);
    code.label("text")
        .emit(&[0x2b, 0x2c, 0x15, 4, 0x15, 10, 0x64, 0x15, 5, 0x10, 20])
        .reference(0xb6, draw_text);
    code.emit(&[0x15, 7]).jump(0x99, "restore");
    code.emit(&[0x2b, 0x15, 4, 0x15, 8, 0x60, 0x15, 10, 0x64, 0x15, 5]);
    code.emit(&[0x15, 4, 0x15, 8, 0x60, 0x15, 10, 0x64, 0x15, 5, 0x2d])
        .reference(0xb6, height)
        .emit(&[0x60, 0x04, 0x64])
        .reference(0xb6, draw_line);
    code.label("restore")
        .emit(&[0x2b, 0x15, 11, 0x15, 12, 0x15, 13, 0x15, 14])
        .reference(0xb6, restore_clip)
        .emit(&[0xb1]);
    let descriptor = "(Ljavax/microedition/lcdui/Graphics;Ljava/lang/String;Ljavax/microedition/lcdui/Font;IIIZ)V";
    append_code_method(
        class,
        ACC_PRIVATE,
        "__paintEditor",
        descriptor,
        8,
        16,
        code.finish(),
    );
    let paint = append_method_ref(class, FIELD, "__paintEditor", descriptor);
    let code = method_code_mut(
        class,
        "__paint",
        "(Ljavax/microedition/lcdui/Graphics;IIIZ)V",
    );
    assert_eq!(
        &code.code[code.code.len() - 15..code.code.len() - 12],
        &[0x2b, 0x19, 0x07]
    );
    code.code.truncate(code.code.len() - 15);
    let mut tail = Code::default();
    tail.emit(&[
        0x2a, 0x2b, 0x19, 7, 0x19, 6, 0x1c, 0x06, 0x60, 0x1d, 0x05, 0x60, 0x15, 4, 0x10, 6, 0x64,
        0x15, 5,
    ])
    .reference(0xb7, paint)
    .emit(&[0xb1]);
    code.code.extend(tail.finish());
    code.max_stack = 9;
}
