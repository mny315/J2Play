//! Character `Reader` and `InputStreamReader` declarations.

use crate::bytecode_builder::Code;

use super::{
    ACC_ABSTRACT, ACC_PRIVATE, ACC_PROTECTED, ACC_PUBLIC, ACC_SUPER, ClassFile, ConstantPool,
    Member, abstract_method, code_method, native_method,
};

pub(crate) fn java_io_reader() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/io/Reader");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let lock = pool.field_ref("java/io/Reader", "lock", "Ljava/lang/Object;");
    let reader_read = pool.method_ref("java/io/Reader", "read", "([CII)I");
    let illegal_argument = pool.class("java/lang/IllegalArgumentException");
    let illegal_argument_init =
        pool.method_ref("java/lang/IllegalArgumentException", "<init>", "()V");
    let [init_high, init_low] = object_init.to_be_bytes();
    let [lock_high, lock_low] = lock.to_be_bytes();
    let [read_high, read_low] = reader_read.to_be_bytes();
    let lock_name = pool.utf8("lock");
    let object_descriptor = pool.utf8("Ljava/lang/Object;");
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PROTECTED,
            "<init>",
            "()V",
            2,
            1,
            vec![
                0x2a, 0xb7, init_high, init_low, // Object.<init>
                0x2a, 0x2a, 0xb5, lock_high, lock_low, // lock = this
                0xb1,
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "markSupported",
            "()Z",
            1,
            1,
            vec![0x03, 0xac], // return false
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "read",
            "()I",
            4,
            2,
            vec![
                0x04, 0xbc, 0x05, 0x4c, // char[] one = new char[1]
                0x2a, 0x2b, 0x03, 0x04, 0xb6, read_high, read_low, // read(one, 0, 1)
                0x02, 0xa0, 0x00, 0x05, // if result != -1, return the char
                0x02, 0xac, // return -1
                0x2b, 0x03, 0x34, 0xac, // return one[0]
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "read",
            "([C)I",
            4,
            2,
            vec![
                0x2a, 0x2b, 0x03, 0x2b, 0xbe, // this, chars, 0, chars.length
                0xb6, read_high, read_low, // read(chars, 0, chars.length)
                0xac,
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "skip",
            "(J)J",
            6,
            7,
            reader_skip_code(reader_read, illegal_argument, illegal_argument_init),
        ),
        abstract_method(&mut pool, ACC_PUBLIC, "read", "([CII)I"),
        abstract_method(&mut pool, ACC_PUBLIC, "close", "()V"),
    ];
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER | ACC_ABSTRACT,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: vec![Member {
            access_flags: ACC_PROTECTED,
            name_index: lock_name,
            descriptor_index: object_descriptor,
            attributes: Vec::new(),
        }],
        methods,
        attributes: Vec::new(),
    }
}

fn reader_skip_code(read: u16, illegal: u16, illegal_init: u16) -> Vec<u8> {
    // Locals: this, requested (long), buffer, skipped (long), chunk/read count.
    // The buffer is local to this invocation and never exceeds 256 characters.
    let mut code = Code::default();
    code.emit(&[0x1f, 0x09, 0x94, 0x59])
        .jump(0x9c, "nonnegative");
    code.emit(&[0x57])
        .reference(0xbb, illegal)
        .emit(&[0x59])
        .reference(0xb7, illegal_init)
        .emit(&[0xbf]);
    code.label("nonnegative")
        .jump(0x9a, "allocate")
        .emit(&[0x09, 0xad]);
    code.label("allocate")
        .emit(&[0x1f, 0x5c, 0x11, 0x01, 0x00, 0x85, 0x94])
        .jump(0x9e, "allocate_buffer")
        .emit(&[0x58, 0x11, 0x01, 0x00, 0x85]);
    code.label("allocate_buffer")
        .emit(&[0x88, 0x59, 0x36, 6, 0xbc, 0x05, 0x4e, 0x09, 0x37, 4]);
    code.label("read")
        .emit(&[0x2a, 0x2d, 0x03, 0x15, 6])
        .reference(0xb6, read)
        .emit(&[0x36, 6, 0x15, 6])
        .jump(0x9e, "done");
    // Respect short reads and stop on EOF or a non-progressing override.
    code.emit(&[0x16, 4, 0x15, 6, 0x85, 0x61, 0x37, 4]);
    code.emit(&[0x16, 4, 0x1f, 0x94]).jump(0x9c, "done");
    // The next read is bounded by both the remaining count and the buffer.
    code.emit(&[0x1f, 0x16, 4, 0x65, 0x5c, 0x2d, 0xbe, 0x85, 0x94])
        .jump(0x9e, "remaining")
        .emit(&[0x58, 0x2d, 0xbe, 0x85]);
    code.label("remaining")
        .emit(&[0x88, 0x36, 6])
        .jump(0xa7, "read");
    code.label("done").emit(&[0x16, 4, 0xad]);
    code.finish()
}

pub(crate) fn java_io_input_stream_reader() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/io/InputStreamReader");
    let super_class = pool.class("java/io/Reader");
    let fields = [
        ("in", "Ljava/io/InputStream;"),
        ("encoding", "I"),
        ("pending", "I"),
        ("closed", "Z"),
    ]
    .into_iter()
    .map(|(name, descriptor)| Member {
        access_flags: ACC_PRIVATE,
        name_index: pool.utf8(name),
        descriptor_index: pool.utf8(descriptor),
        attributes: Vec::new(),
    })
    .collect();
    let methods = vec![
        native_method(&mut pool, ACC_PUBLIC, "<init>", "(Ljava/io/InputStream;)V"),
        native_method(
            &mut pool,
            ACC_PUBLIC,
            "<init>",
            "(Ljava/io/InputStream;Ljava/lang/String;)V",
        ),
        native_method(&mut pool, ACC_PUBLIC, "read", "()I"),
        native_method(&mut pool, ACC_PUBLIC, "read", "([CII)I"),
        native_method(&mut pool, ACC_PUBLIC, "close", "()V"),
    ];
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}
