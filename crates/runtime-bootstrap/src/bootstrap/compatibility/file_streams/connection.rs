//! Connection access and ownership of its input/output streams.

use super::{
    ACC_PRIVATE, ACC_PUBLIC, ACC_STATIC, CONNECTION, ClassFile, Code, Constant, INPUT,
    INPUT_DESCRIPTOR, Member, OUTPUT, OUTPUT_DESCRIPTOR, append_class, append_constant,
    append_field_ref, append_method_ref, append_native_method, method, nonnegative,
    require_connection_open, require_stream_access, throw,
};

pub(crate) fn repair_connection(class: &mut ClassFile) {
    let mut code = Code::default();
    require_connection_open(
        &mut code,
        class,
        "javax/microedition/io/file/ConnectionClosedException",
    );
    code.emit(&[0xb1]);
    method(class, ACC_PRIVATE, "requireOpen", "()V", 1, code);
    add_connection_input(class);
    let name_index = append_constant(class, Constant::Utf8("__output".into()));
    let descriptor_index = append_constant(class, Constant::Utf8(OUTPUT_DESCRIPTOR.into()));
    class.fields.push(Member {
        access_flags: 0,
        name_index,
        descriptor_index,
        attributes: vec![],
    });
    let active = append_field_ref(class, CONNECTION, "__output", OUTPUT_DESCRIPTOR);
    let url = append_field_ref(class, CONNECTION, "url", "Ljava/lang/String;");
    let offset = append_field_ref(class, OUTPUT, "offset", "J");
    let require = append_method_ref(class, CONNECTION, "requireWrite", "()V");
    let flush = append_method_ref(class, OUTPUT, "flush", "()V");
    append_native_method(
        class,
        ACC_PRIVATE | ACC_STATIC,
        "outputOffset0",
        "(Ljava/lang/String;J)J",
    );
    let position = append_method_ref(class, CONNECTION, "outputOffset0", "(Ljava/lang/String;J)J");
    let mut code = Code::default();
    require_stream_access(&mut code, class, "requireWrite");
    nonnegative(&mut code, class);
    code.emit(&[0x2a]);
    code.reference(0xb4, active);
    code.jump(0xc6, "available");
    throw(&mut code, class, "java/io/IOException");
    code.label("available");
    code.emit(&[0x2a]);
    code.reference(0xb4, url);
    code.emit(&[0x1f]);
    code.reference(0xb8, position);
    code.emit(&[0x40]); // lstore_1
    code.reference(0xbb, append_class(class, OUTPUT));
    code.emit(&[0x59, 0x2a, 0x1f]);
    code.reference(
        0xb7,
        append_method_ref(
            class,
            OUTPUT,
            "<init>",
            "(Ljavax/microedition/io/file/FileConnectionImpl;J)V",
        ),
    );
    code.emit(&[0x4e, 0x2a, 0x2d]);
    code.reference(0xb5, active);
    code.emit(&[0x2d, 0xb0]);
    method(
        class,
        ACC_PUBLIC,
        "openOutputStream",
        "(J)Ljava/io/OutputStream;",
        4,
        code,
    );

    let mut code = Code::default();
    code.emit(&[0x2a]);
    code.reference(0xb7, require);
    nonnegative(&mut code, class);
    code.emit(&[0x2a]);
    code.reference(0xb4, active);
    code.emit(&[0x4e, 0x2d]);
    code.jump(0xc6, "no_flush");
    code.emit(&[0x2d]);
    code.reference(0xb6, flush);
    code.label("no_flush");
    code.emit(&[0x2a]);
    code.reference(0xb4, url);
    code.emit(&[0x1f]);
    code.reference(
        0xb8,
        append_method_ref(class, CONNECTION, "truncate0", "(Ljava/lang/String;J)V"),
    );
    code.emit(&[0x2d]);
    code.jump(0xc6, "no_position");
    code.emit(&[0x2d, 0x59]);
    code.reference(0xb4, offset);
    code.emit(&[0x1f]);
    code.reference(
        0xb8,
        append_method_ref(class, "java/lang/Math", "min", "(JJ)J"),
    );
    code.reference(0xb5, offset);
    code.label("no_position");
    code.emit(&[0xb1]);
    method(class, ACC_PUBLIC, "truncate", "(J)V", 4, code);
    close_streams_before_mutation(class, active);
}

fn close_streams_before_mutation(class: &mut ClassFile, active: u16) {
    let close = append_method_ref(class, OUTPUT, "close", "()V");
    let input = append_field_ref(class, CONNECTION, "__input", INPUT_DESCRIPTOR);
    let close_input = append_method_ref(class, INPUT, "close", "()V");
    let require = append_method_ref(class, CONNECTION, "requireWrite", "()V");
    for (name, descriptor, locals) in [("rename", "(Ljava/lang/String;)V", 2), ("delete", "()V", 1)]
    {
        let index = class
            .methods
            .iter()
            .position(|member| {
                class.utf8(member.name_index) == Some(name)
                    && class.utf8(member.descriptor_index) == Some(descriptor)
            })
            .expect("file mutation exists");
        let implementation = format!("__{name}");
        class.methods[index].name_index =
            append_constant(class, Constant::Utf8(implementation.clone()));
        class.methods[index].access_flags = ACC_PRIVATE;
        let call = append_method_ref(class, CONNECTION, &implementation, descriptor);
        let mut code = Code::default();
        code.emit(&[0x2a]);
        code.reference(0xb7, require);
        code.emit(&[0x2a]);
        code.reference(0xb4, active);
        code.jump(0xc6, "output_absent");
        code.emit(&[0x2a]);
        code.reference(0xb4, active);
        code.reference(0xb6, close);
        code.label("output_absent");
        code.emit(&[0x2a]);
        code.reference(0xb4, input);
        code.jump(0xc6, "input_absent");
        code.emit(&[0x2a]);
        code.reference(0xb4, input);
        code.reference(0xb6, close_input);
        code.label("input_absent");
        code.emit(&[0x2a]);
        if locals == 2 {
            code.emit(&[0x2b]);
        }
        code.reference(0xb7, call);
        code.emit(&[0xb1]);
        method(class, ACC_PUBLIC, name, descriptor, locals, code);
    }
}

fn add_connection_input(class: &mut ClassFile) {
    append_native_method(class, ACC_STATIC, "revision0", "()J");
    let mut code = Code::default();
    code.emit(&[0x2a]);
    code.reference(
        0xb4,
        append_field_ref(class, CONNECTION, "url", "Ljava/lang/String;"),
    );
    code.reference(
        0xb8,
        append_method_ref(class, CONNECTION, "read0", "(Ljava/lang/String;)[B"),
    );
    code.emit(&[0xb0]);
    method(class, 0, "__readSnapshot", "()[B", 1, code);
    let name_index = append_constant(class, Constant::Utf8("__input".into()));
    let descriptor_index = append_constant(class, Constant::Utf8(INPUT_DESCRIPTOR.into()));
    class.fields.push(Member {
        access_flags: 0,
        name_index,
        descriptor_index,
        attributes: vec![],
    });
    let active = append_field_ref(class, CONNECTION, "__input", INPUT_DESCRIPTOR);
    let mut code = Code::default();
    require_stream_access(&mut code, class, "requireRead");
    code.emit(&[0x2a]);
    code.reference(0xb4, active);
    code.jump(0xc6, "available");
    throw(&mut code, class, "java/io/IOException");
    code.label("available");
    code.reference(0xbb, append_class(class, INPUT));
    code.emit(&[0x59, 0x2a]);
    // Capture before reading: a guest thread can yield between bootstrap calls.
    code.reference(
        0xb8,
        append_method_ref(class, CONNECTION, "revision0", "()J"),
    );
    code.emit(&[0x2a]);
    code.reference(
        0xb6,
        append_method_ref(class, CONNECTION, "__readSnapshot", "()[B"),
    );
    code.reference(
        0xb7,
        append_method_ref(
            class,
            INPUT,
            "<init>",
            "(Ljavax/microedition/io/file/FileConnectionImpl;J[B)V",
        ),
    );
    code.emit(&[0x4c, 0x2a, 0x2b]);
    code.reference(0xb5, active);
    code.emit(&[0x2b, 0xb0]);
    method(
        class,
        ACC_PUBLIC,
        "openInputStream",
        "()Ljava/io/InputStream;",
        2,
        code,
    );
}
