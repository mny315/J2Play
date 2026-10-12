//! Console `PrintStream` overloads share one conversion and output contract.

use crate::bootstrap::{ConstantPool, code_method, native_method};
use crate::bytecode_builder::Code;
use crate::{ACC_PRIVATE, ACC_PUBLIC, ACC_SUPER};
use classfile::{ClassFile, Constant};

pub(super) fn class() -> ClassFile {
    const OWNER: &str = "java/io/PrintStream";
    let mut pool = ConstantPool::new();
    let this_class = pool.class(OWNER);
    let super_class = pool.class("java/io/OutputStream");
    let code_name = pool.utf8("Code");
    let init = pool.method_ref("java/io/OutputStream", "<init>", "()V");
    let write = pool.method_ref(OWNER, "write0", "(Ljava/lang/String;Z)V");
    let newline = pool.method_ref(OWNER, "println", "()V");
    let empty_text = pool.utf8("");
    let empty = pool.push(Constant::String {
        string_index: empty_text,
    });
    let mut constructor = Code::default();
    constructor
        .emit(&[0x2a])
        .reference(0xb7, init)
        .emit(&[0xb1]);
    let mut line = Code::default();
    line.emit(&[0x2a])
        .reference(0x13, empty)
        .emit(&[0x04])
        .reference(0xb7, write)
        .emit(&[0xb1]);
    let mut methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "<init>",
            "()V",
            1,
            1,
            constructor.finish(),
        ),
        native_method(&mut pool, ACC_PRIVATE, "write0", "(Ljava/lang/String;Z)V"),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "println",
            "()V",
            3,
            1,
            line.finish(),
        ),
    ];
    for (argument, load, locals) in [
        ("Z", 0x1b, 2),
        ("C", 0x1b, 2),
        ("I", 0x1b, 2),
        ("J", 0x1f, 3),
        ("F", 0x23, 2),
        ("D", 0x27, 3),
        ("[C", 0x2b, 2),
        ("Ljava/lang/Object;", 0x2b, 2),
        ("Ljava/lang/String;", 0x2b, 2),
    ] {
        let conversion = if argument == "Ljava/lang/String;" {
            "Ljava/lang/Object;"
        } else {
            argument
        };
        let value_of = pool.method_ref(
            "java/lang/String",
            "valueOf",
            &format!("({conversion})Ljava/lang/String;"),
        );
        let descriptor = format!("({argument})V");
        let print = pool.method_ref(OWNER, "print", &descriptor);
        let mut code = Code::default();
        code.emit(&[0x2a, load])
            .reference(0xb8, value_of)
            .emit(&[0x03])
            .reference(0xb7, write)
            .emit(&[0xb1]);
        methods.push(code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "print",
            &descriptor,
            3,
            locals,
            code.finish(),
        ));
        // CLDC println(value) dispatches through print(value), then println().
        let mut code = Code::default();
        code.emit(&[0x2a, load])
            .reference(0xb6, print)
            .emit(&[0x2a])
            .reference(0xb6, newline)
            .emit(&[0xb1]);
        methods.push(code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "println",
            &descriptor,
            3,
            locals,
            code.finish(),
        ));
    }
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}
