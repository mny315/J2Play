//! Output stream lifetime, flushing and file position.

use super::{
    ACC_FINAL, ACC_PRIVATE, ACC_PUBLIC, BUFFER, CONNECTION, ClassFile, Code, OUTPUT,
    OUTPUT_DESCRIPTOR, append_field_ref, append_method_ref, method, throw,
};

pub(crate) fn repair_output(class: &mut ClassFile) {
    let owner = append_field_ref(
        class,
        OUTPUT,
        "owner",
        "Ljavax/microedition/io/file/FileConnectionImpl;",
    );
    let offset = append_field_ref(class, OUTPUT, "offset", "J");
    let active = append_field_ref(class, CONNECTION, "__output", OUTPUT_DESCRIPTOR);
    let offset_field = class
        .fields
        .iter()
        .position(|field| class.utf8(field.name_index) == Some("offset"))
        .unwrap();
    // The connection updates the stream position after truncating its file.
    class.fields[offset_field].access_flags &= !(ACC_FINAL | ACC_PRIVATE);
    let require = append_method_ref(class, OUTPUT, "__requireOpen", "()V");
    let flush = append_method_ref(class, OUTPUT, "flush", "()V");
    let mut code = Code::default();
    code.emit(&[0x2a]);
    code.reference(0xb4, owner);
    code.jump(0xc7, "open");
    throw(&mut code, class, "java/io/IOException");
    code.label("open");
    code.emit(&[0xb1]);
    method(class, ACC_PRIVATE, "__requireOpen", "()V", 1, code);

    let mut code = Code::default();
    code.emit(&[0x2a]);
    code.reference(0xb7, require);
    code.emit(&[0x2a]);
    code.reference(0xb6, append_method_ref(class, BUFFER, "size", "()I"));
    code.jump(0x99, "empty");
    code.emit(&[0x2a]);
    code.reference(
        0xb6,
        append_method_ref(class, BUFFER, "toByteArray", "()[B"),
    );
    code.emit(&[0x4c, 0x2a]);
    code.reference(0xb4, owner);
    code.reference(
        0xb8,
        append_method_ref(
            class,
            CONNECTION,
            "access$000",
            "(Ljavax/microedition/io/file/FileConnectionImpl;)Ljava/lang/String;",
        ),
    );
    code.emit(&[0x2b, 0x2a]);
    code.reference(0xb4, offset);
    code.reference(
        0xb8,
        append_method_ref(class, CONNECTION, "access$200", "(Ljava/lang/String;[BJ)V"),
    );
    // Commit the position and clear buffered bytes only after the host write succeeds.
    code.emit(&[0x2a, 0x59]);
    code.reference(0xb4, offset);
    code.emit(&[0x2b, 0xbe, 0x85, 0x61]);
    code.reference(0xb5, offset);
    code.emit(&[0x2a]);
    code.reference(0xb6, append_method_ref(class, BUFFER, "reset", "()V"));
    code.label("empty");
    code.emit(&[0xb1]);
    method(class, ACC_PUBLIC, "flush", "()V", 2, code);

    let mut code = Code::default();
    code.emit(&[0x2a]);
    code.reference(0xb4, owner);
    code.jump(0xc6, "closed");
    code.emit(&[0x2a]);
    code.reference(0xb6, flush);
    code.emit(&[0x2a]);
    code.reference(0xb4, owner);
    code.emit(&[0x01]);
    code.reference(0xb5, active);
    code.emit(&[0x2a, 0x01]);
    code.reference(0xb5, owner);
    code.label("closed");
    code.emit(&[0xb1]);
    method(class, ACC_PUBLIC, "close", "()V", 1, code);
    for (descriptor, locals, arguments) in [
        ("(I)V", 2, &[0x2a, 0x1b][..]),
        ("([BII)V", 4, &[0x2a, 0x2b, 0x1c, 0x1d][..]),
    ] {
        let mut code = Code::default();
        code.emit(&[0x2a]);
        code.reference(0xb7, require);
        code.emit(arguments);
        code.reference(0xb7, append_method_ref(class, BUFFER, "write", descriptor));
        code.emit(&[0xb1]);
        method(class, ACC_PUBLIC, "write", descriptor, locals, code);
    }
}
