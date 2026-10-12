//! Input stream lifetime and refresh after filesystem mutations.

use super::{
    ACC_FINAL, ACC_PRIVATE, ACC_PUBLIC, CONNECTION, ClassFile, Code, Constant, INPUT, INPUT_BUFFER,
    INPUT_DESCRIPTOR, Member, append_class, append_constant, append_field_ref, append_method_ref,
    method, throw,
};

pub(crate) fn input_class() -> ClassFile {
    let mut class = ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: vec![None],
        access_flags: ACC_FINAL | 0x20,
        this_class: 0,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        methods: vec![],
        attributes: vec![],
    };
    class.this_class = append_class(&mut class, INPUT);
    class.super_class = append_class(&mut class, INPUT_BUFFER);
    let name_index = append_constant(&mut class, Constant::Utf8("owner".into()));
    let descriptor = "Ljavax/microedition/io/file/FileConnectionImpl;";
    let descriptor_index = append_constant(&mut class, Constant::Utf8(descriptor.into()));
    class.fields.push(Member {
        access_flags: ACC_PRIVATE,
        name_index,
        descriptor_index,
        attributes: vec![],
    });
    let owner = append_field_ref(&mut class, INPUT, "owner", descriptor);
    let name_index = append_constant(&mut class, Constant::Utf8("revision".into()));
    let descriptor_index = append_constant(&mut class, Constant::Utf8("J".into()));
    class.fields.push(Member {
        access_flags: ACC_PRIVATE,
        name_index,
        descriptor_index,
        attributes: vec![],
    });
    let revision = append_field_ref(&mut class, INPUT, "revision", "J");
    let active = append_field_ref(&mut class, CONNECTION, "__input", INPUT_DESCRIPTOR);
    let mut code = Code::default();
    code.emit(&[0x2a, 0x19, 0x04]);
    code.reference(
        0xb7,
        append_method_ref(&mut class, INPUT_BUFFER, "<init>", "([B)V"),
    );
    code.emit(&[0x2a, 0x2b]);
    code.reference(0xb5, owner);
    code.emit(&[0x2a, 0x20]); // lload_2
    code.reference(0xb5, revision);
    code.emit(&[0xb1]);
    method(
        &mut class,
        0,
        "<init>",
        "(Ljavax/microedition/io/file/FileConnectionImpl;J[B)V",
        5,
        code,
    );

    refresh_input(&mut class, owner, revision);

    let mut code = Code::default();
    code.emit(&[0x2a]);
    code.reference(0xb4, owner);
    code.jump(0xc7, "open");
    throw(&mut code, &mut class, "java/io/IOException");
    code.label("open");
    code.emit(&[0x2a]);
    code.reference(
        0xb7,
        append_method_ref(&mut class, INPUT, "__refresh", "()V"),
    );
    code.emit(&[0xb1]);
    method(&mut class, ACC_PRIVATE, "__requireOpen", "()V", 1, code);

    let mut code = Code::default();
    code.emit(&[0x2a]);
    code.reference(0xb4, owner);
    code.jump(0xc6, "closed");
    code.emit(&[0x2a]);
    code.reference(0xb4, owner);
    code.emit(&[0x01]);
    code.reference(0xb5, active);
    code.emit(&[0x2a, 0x01]);
    code.reference(0xb5, owner);
    // A retained, closed stream must not retain the file's entire byte snapshot.
    code.emit(&[0x2a, 0x01]);
    code.reference(
        0xb5,
        append_field_ref(&mut class, INPUT_BUFFER, "buf", "[B"),
    );
    code.label("closed");
    code.emit(&[0xb1]);
    method(&mut class, ACC_PUBLIC, "close", "()V", 1, code);

    let require = append_method_ref(&mut class, INPUT, "__requireOpen", "()V");
    for (name, descriptor, locals, arguments, result) in [
        ("read", "()I", 1, &[][..], 0xac),
        ("read", "([BII)I", 4, &[0x2b, 0x1c, 0x1d][..], 0xac),
        ("reset", "()V", 1, &[][..], 0xb1),
    ] {
        let mut code = Code::default();
        code.emit(&[0x2a]);
        code.reference(0xb7, require);
        code.emit(&[0x2a]);
        code.emit(arguments);
        code.reference(
            0xb7,
            append_method_ref(&mut class, INPUT_BUFFER, name, descriptor),
        );
        code.emit(&[result]);
        method(&mut class, ACC_PUBLIC, name, descriptor, locals, code);
    }
    input_position_methods(&mut class, require);
    class
}

fn refresh_input(class: &mut ClassFile, owner: u16, revision: u16) {
    let buffer = append_field_ref(class, INPUT_BUFFER, "buf", "[B");
    let count = append_field_ref(class, INPUT_BUFFER, "count", "I");
    let mut code = Code::default();
    code.reference(
        0xb8,
        append_method_ref(class, CONNECTION, "revision0", "()J"),
    );
    code.emit(&[0x40, 0x2a]); // lstore_1, aload_0
    code.reference(0xb4, revision);
    code.emit(&[0x1f, 0x94]);
    code.jump(0x99, "current");
    // Let allocation collect the previous snapshot. A failed refresh keeps the
    // previous token, so the next access retries before touching the buffer.
    code.emit(&[0x2a, 0x01]);
    code.reference(0xb5, buffer);
    code.emit(&[0x2a]);
    code.reference(0xb4, owner);
    code.reference(
        0xb6,
        append_method_ref(class, CONNECTION, "__readSnapshot", "()[B"),
    );
    code.emit(&[0x4e, 0x2a, 0x2d]);
    code.reference(0xb5, buffer);
    code.emit(&[0x2a, 0x2d, 0xbe]);
    code.reference(0xb5, count);
    code.emit(&[0x2a, 0x1f]);
    code.reference(0xb5, revision);
    code.label("current");
    code.emit(&[0xb1]);
    method(class, ACC_PRIVATE, "__refresh", "()V", 4, code);
}

fn input_position_methods(class: &mut ClassFile, require: u16) {
    let count = append_field_ref(class, INPUT_BUFFER, "count", "I");
    let position = append_field_ref(class, INPUT_BUFFER, "pos", "I");
    for name in ["available", "skip"] {
        let mut code = Code::default();
        code.emit(&[0x2a]);
        code.reference(0xb7, require);
        code.emit(&[0x2a]);
        code.reference(0xb4, count);
        code.emit(&[0x2a]);
        code.reference(0xb4, position);
        code.emit(&[0x64, 0x03]); // isub, iconst_0
        code.reference(
            0xb8,
            append_method_ref(class, "java/lang/Math", "max", "(II)I"),
        );
        if name == "available" {
            code.emit(&[0xac]);
            method(class, ACC_PUBLIC, name, "()I", 1, code);
        } else {
            // Truncation can leave the cursor beyond EOF; never report or skip
            // a negative number, and retain the original absolute position.
            code.emit(&[0x85, 0x1f, 0x09]);
            code.reference(
                0xb8,
                append_method_ref(class, "java/lang/Math", "max", "(JJ)J"),
            );
            code.reference(
                0xb8,
                append_method_ref(class, "java/lang/Math", "min", "(JJ)J"),
            );
            code.emit(&[0x40, 0x2a, 0x59]);
            code.reference(0xb4, position);
            code.emit(&[0x1f, 0x88, 0x60]);
            code.reference(0xb5, position);
            code.emit(&[0x1f, 0xad]);
            method(class, ACC_PUBLIC, name, "(J)J", 3, code);
        }
    }
}
