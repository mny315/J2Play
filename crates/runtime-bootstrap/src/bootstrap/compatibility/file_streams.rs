//! `FileConnection` stream ownership and common bytecode helpers.

use crate::bytecode_builder::Code;

use super::{
    ACC_FINAL, ACC_PRIVATE, ACC_PUBLIC, ACC_STATIC, ClassFile, Constant, Member, append_class,
    append_code_method, append_constant, append_field_ref, append_method_ref, append_native_method,
};

mod connection;
mod input;
mod output;

pub(crate) use connection::repair_connection;
pub(crate) use input::input_class;
pub(crate) use output::repair_output;

const CONNECTION: &str = "javax/microedition/io/file/FileConnectionImpl";
const OUTPUT: &str = "javax/microedition/io/file/FileConnectionImpl$FileOutput";
const OUTPUT_DESCRIPTOR: &str = "Ljavax/microedition/io/file/FileConnectionImpl$FileOutput;";
const BUFFER: &str = "java/io/ByteArrayOutputStream";
const INPUT: &str = "javax/microedition/io/file/FileConnectionImpl$FileInput";
const INPUT_DESCRIPTOR: &str = "Ljavax/microedition/io/file/FileConnectionImpl$FileInput;";
const INPUT_BUFFER: &str = "java/io/ByteArrayInputStream";

fn throw(code: &mut Code, class: &mut ClassFile, name: &str) {
    code.reference(0xbb, append_class(class, name));
    code.emit(&[0x59]);
    code.reference(0xb7, append_method_ref(class, name, "<init>", "()V"));
    code.emit(&[0xbf]);
}

fn method(
    class: &mut ClassFile,
    flags: u16,
    name: &str,
    descriptor: &str,
    locals: u16,
    code: Code,
) {
    if let Some(index) = class.methods.iter().position(|member| {
        class.utf8(member.name_index) == Some(name)
            && class.utf8(member.descriptor_index) == Some(descriptor)
    }) {
        class.methods.remove(index);
    }
    append_code_method(class, flags, name, descriptor, 6, locals, code.finish());
}

fn nonnegative(code: &mut Code, class: &mut ClassFile) {
    code.emit(&[0x1f, 0x09, 0x94]); // lload_1, lconst_0, lcmp
    code.jump(0x9c, "valid");
    throw(code, class, "java/lang/IllegalArgumentException");
    code.label("valid");
}

fn require_connection_open(code: &mut Code, class: &mut ClassFile, exception: &str) {
    code.emit(&[0x2a]);
    code.reference(0xb4, append_field_ref(class, CONNECTION, "open", "Z"));
    code.jump(0x9a, "open");
    throw(code, class, exception);
    code.label("open");
}

fn require_stream_access(code: &mut Code, class: &mut ClassFile, access: &str) {
    // StreamConnection methods use IOException for a closed connection;
    // FileConnection's own methods instead use ConnectionClosedException.
    require_connection_open(code, class, "java/io/IOException");
    code.emit(&[0x2a]);
    code.reference(0xb7, append_method_ref(class, CONNECTION, access, "()V"));
}
