use crate::bytecode_builder::Code;

use super::{
    ACC_PRIVATE, ClassFile, Constant, append_code_method, append_constant, append_field_ref,
    append_method_ref, method_code_mut,
};

const FIELD: &str = "javax/microedition/lcdui/TextField";
const STRING: &str = "java/lang/String";

/// Host text and normalized phone keys share one editor. Cursor movement and
/// deletion stop at Unicode scalar boundaries while public positions stay UTF-16.
pub(crate) fn implement_text_field_input(class: &mut ClassFile) {
    append_caret_step(class);
    let text = append_field_ref(class, FIELD, "text", "Ljava/lang/String;");
    let caret = append_field_ref(class, FIELD, "caret", "I");
    let constraints = append_field_ref(class, FIELD, "constraints", "I");
    let uneditable_mask = append_constant(class, Constant::Integer(0x20000));
    let maximum = append_field_ref(class, FIELD, "maximum", "I");
    let length = append_method_ref(class, STRING, "length", "()I");
    let substring_range = append_method_ref(class, STRING, "substring", "(II)Ljava/lang/String;");
    let substring_tail = append_method_ref(class, STRING, "substring", "(I)Ljava/lang/String;");
    let value_of_char = append_method_ref(class, STRING, "valueOf", "(C)Ljava/lang/String;");
    let concat = append_method_ref(
        class,
        STRING,
        "concat",
        "(Ljava/lang/String;)Ljava/lang/String;",
    );
    let valid = append_method_ref(class, FIELD, "valid", "(Ljava/lang/String;I)Z");
    let step = append_method_ref(class, FIELD, "__caretStep", "(I)I");
    let delete = append_method_ref(class, FIELD, "delete", "(II)V");
    let invalidate = append_method_ref(class, FIELD, "invalidate", "()V");
    let mut code = Code::default();
    // UNEDITABLE fields ignore both phone keys and host edits.
    code.emit(&[0x2a])
        .reference(0xb4, constraints)
        .reference(0x13, uneditable_mask)
        .emit(&[0x7e])
        .jump(0x99, "editable")
        .emit(&[0xb1]);
    code.label("editable");
    for (key, target) in [
        (0xfd, "left"),
        (0xfc, "right"),
        (0xf9, "backward"),
        (0x08, "backward"),
        (0x7f, "forward"),
    ] {
        code.emit(&[0x1b, 0x10, key]).jump(0x9f, target);
    }
    code.emit(&[0x1b, 0x10, 0x20]).jump(0xa1, "done"); // Ignore control codes.
    code.emit(&[0x2a])
        .reference(0xb4, text)
        .reference(0xb6, length)
        .emit(&[0x2a])
        .reference(0xb4, maximum)
        .jump(0xa2, "done"); // Preserve the field's UTF-16 limit.
    code.emit(&[0x2a])
        .reference(0xb4, text)
        .emit(&[0x03, 0x2a])
        .reference(0xb4, caret)
        .reference(0xb6, substring_range)
        .emit(&[0x1b, 0x92])
        .reference(0xb8, value_of_char)
        .reference(0xb6, concat);
    code.emit(&[0x2a])
        .reference(0xb4, text)
        .emit(&[0x2a])
        .reference(0xb4, caret)
        .reference(0xb6, substring_tail)
        .reference(0xb6, concat)
        .emit(&[0x4d]); // Candidate text in local 2.
    code.emit(&[0x2c, 0x2a])
        .reference(0xb4, constraints)
        .reference(0xb8, valid)
        .jump(0x99, "done");
    code.emit(&[0x2a, 0x2c])
        .reference(0xb5, text)
        .emit(&[0x2a, 0x59])
        .reference(0xb4, caret)
        .emit(&[0x04, 0x60])
        .reference(0xb5, caret)
        .jump(0xa7, "invalidate");
    for (label, direction) in [("left", 0x02), ("right", 0x04)] {
        code.label(label)
            .emit(&[0x2a, 0x2a, direction])
            .reference(0xb6, step)
            .reference(0xb5, caret)
            .jump(0xa7, "invalidate");
    }
    code.label("backward")
        .emit(&[0x2a, 0x02])
        .reference(0xb6, step)
        .emit(&[0x3d, 0x2a, 0x1c, 0x2a])
        .reference(0xb4, caret)
        .emit(&[0x1c, 0x64])
        .reference(0xb6, delete)
        .emit(&[0xb1]);
    code.label("forward")
        .emit(&[0x2a, 0x2a])
        .reference(0xb4, caret)
        .emit(&[0x2a, 0x04])
        .reference(0xb6, step)
        .emit(&[0x2a])
        .reference(0xb4, caret)
        .emit(&[0x64])
        .reference(0xb6, delete)
        .emit(&[0xb1]);
    code.label("invalidate")
        .emit(&[0x2a])
        .reference(0xb6, invalidate);
    code.label("done").emit(&[0xb1]);
    let method = method_code_mut(class, "__key", "(I)V");
    method.max_stack = 5;
    method.max_locals = 3;
    method.code = code.finish();
}

fn append_caret_step(class: &mut ClassFile) {
    let text = append_field_ref(class, FIELD, "text", "Ljava/lang/String;");
    let caret = append_field_ref(class, FIELD, "caret", "I");
    let length = append_method_ref(class, STRING, "length", "()I");
    let char_at = append_method_ref(class, STRING, "charAt", "(I)C");
    let minimum = append_method_ref(class, "java/lang/Math", "min", "(II)I");
    let maximum = append_method_ref(class, "java/lang/Math", "max", "(II)I");
    let mut code = Code::default();
    code.emit(&[0x03, 0x2a])
        .reference(0xb4, text)
        .reference(0xb6, length)
        .emit(&[0x2a])
        .reference(0xb4, caret)
        .emit(&[0x1b, 0x60])
        .reference(0xb8, minimum)
        .reference(0xb8, maximum)
        .emit(&[0x3d, 0x1c])
        .jump(0x9e, "done");
    code.emit(&[0x1c, 0x2a])
        .reference(0xb4, text)
        .reference(0xb6, length)
        .jump(0xa2, "done");
    code.emit(&[0x2a])
        .reference(0xb4, text)
        .emit(&[0x1c, 0x04, 0x64])
        .reference(0xb6, char_at)
        .emit(&[0x10, 0x0a, 0x7a, 0x10, 0x36])
        .jump(0xa0, "done");
    code.emit(&[0x2a])
        .reference(0xb4, text)
        .emit(&[0x1c])
        .reference(0xb6, char_at)
        .emit(&[0x10, 0x0a, 0x7a, 0x10, 0x37])
        .jump(0xa0, "done");
    // Skip the other UTF-16 surrogate in the same direction.
    code.emit(&[0x1c, 0x1b, 0x60, 0xac]);
    code.label("done").emit(&[0x1c, 0xac]);
    append_code_method(
        class,
        ACC_PRIVATE,
        "__caretStep",
        "(I)I",
        5,
        3,
        code.finish(),
    );
}
