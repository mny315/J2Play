//! Object identity, reflection, runtime services and reference declarations.

use crate::bytecode_builder::Code;

use super::{
    ACC_ABSTRACT, ACC_FINAL, ACC_INTERFACE, ACC_PRIVATE, ACC_PROTECTED, ACC_PUBLIC, ACC_STATIC,
    ACC_SUPER, ClassFile, ConstantPool, abstract_method, code_method, field, native_method,
};

pub(crate) fn java_lang_object() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let wait_long = pool.method_ref("java/lang/Object", "wait", "(J)V");
    let hash_code = pool.method_ref("java/lang/Object", "hashCode", "()I");
    let to_string = pool.method_ref("java/lang/Object", "__toString", "(I)Ljava/lang/String;");

    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "<init>",
            "()V",
            0,
            1,
            vec![0xb1], // return
        ),
        native_method(&mut pool, ACC_PUBLIC, "hashCode", "()I"),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "equals",
            "(Ljava/lang/Object;)Z",
            2,
            2,
            {
                let mut code = Code::default();
                code.emit(&[0x2a, 0x2b])
                    .jump(0xa6, "different")
                    .emit(&[0x04, 0xac]);
                code.label("different").emit(&[0x03, 0xac]);
                code.finish()
            },
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "toString",
            "()Ljava/lang/String;",
            2,
            1,
            {
                let mut code = Code::default();
                code.emit(&[0x2a, 0x2a])
                    .reference(0xb6, hash_code)
                    .reference(0xb7, to_string)
                    .emit(&[0xb0]);
                code.finish()
            },
        ),
        native_method(
            &mut pool,
            ACC_PRIVATE,
            "__toString",
            "(I)Ljava/lang/String;",
        ),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_FINAL,
            "getClass",
            "()Ljava/lang/Class;",
        ),
        native_method(&mut pool, ACC_PUBLIC | ACC_FINAL, "wait", "(J)V"),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_FINAL,
            "wait",
            "()V",
            3,
            1,
            {
                let mut code = Code::default();
                code.emit(&[0x2a, 0x09])
                    .reference(0xb6, wait_long)
                    .emit(&[0xb1]);
                code.finish()
            },
        ),
        native_method(&mut pool, ACC_PUBLIC | ACC_FINAL, "wait", "(JI)V"),
        native_method(&mut pool, ACC_PUBLIC | ACC_FINAL, "notify", "()V"),
        native_method(&mut pool, ACC_PUBLIC | ACC_FINAL, "notifyAll", "()V"),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class: 0,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn java_lang_class() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/Class");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let null_pointer_init = pool.method_ref("java/lang/NullPointerException", "<init>", "()V");
    let null_pointer_class = pool.class("java/lang/NullPointerException");
    let resource_bytes = pool.method_ref(
        "java/lang/Class",
        "getResourceBytes",
        "(Ljava/lang/String;)[B",
    );
    let byte_stream_class = pool.class("java/io/ByteArrayInputStream");
    let byte_stream_init = pool.method_ref("java/io/ByteArrayInputStream", "<init>", "([B)V");

    let methods = vec![
        code_method(&mut pool, code_name, ACC_PRIVATE, "<init>", "()V", 1, 1, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb7, object_init)
                .emit(&[0xb1]);
            code.finish()
        }),
        native_method(&mut pool, ACC_PUBLIC, "getName", "()Ljava/lang/String;"),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "forName",
            "(Ljava/lang/String;)Ljava/lang/Class;",
        ),
        native_method(&mut pool, ACC_PUBLIC, "newInstance", "()Ljava/lang/Object;"),
        native_method(
            &mut pool,
            ACC_PUBLIC,
            "isAssignableFrom",
            "(Ljava/lang/Class;)Z",
        ),
        native_method(&mut pool, ACC_PUBLIC, "isArray", "()Z"),
        native_method(&mut pool, ACC_PUBLIC, "isInterface", "()Z"),
        native_method(&mut pool, ACC_PUBLIC, "toString", "()Ljava/lang/String;"),
        native_method(&mut pool, ACC_PUBLIC, "isInstance", "(Ljava/lang/Object;)Z"),
        native_method(
            &mut pool,
            ACC_PRIVATE,
            "getResourceBytes",
            "(Ljava/lang/String;)[B",
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "getResourceAsStream",
            "(Ljava/lang/String;)Ljava/io/InputStream;",
            3,
            3,
            {
                let mut code = Code::default();
                code.emit(&[0x2b]).jump(0xc7, "load");
                code.reference(0xbb, null_pointer_class)
                    .emit(&[0x59])
                    .reference(0xb7, null_pointer_init)
                    .emit(&[0xbf]);
                code.label("load")
                    .emit(&[0x2a, 0x2b])
                    .reference(0xb7, resource_bytes)
                    .emit(&[0x4d, 0x2c])
                    .jump(0xc7, "stream")
                    .emit(&[0x01, 0xb0]);
                code.label("stream")
                    .reference(0xbb, byte_stream_class)
                    .emit(&[0x59, 0x2c])
                    .reference(0xb7, byte_stream_init)
                    .emit(&[0xb0]);
                code.finish()
            },
        ),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_FINAL | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn java_lang_runtime() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/Runtime");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let runtime_class = pool.class("java/lang/Runtime");
    let runtime_init = pool.method_ref("java/lang/Runtime", "<init>", "()V");
    let instance = pool.field_ref("java/lang/Runtime", "INSTANCE", "Ljava/lang/Runtime;");
    let system_gc = pool.method_ref("java/lang/System", "gc", "()V");

    let fields = vec![field(
        &mut pool,
        ACC_PRIVATE | ACC_STATIC | ACC_FINAL,
        "INSTANCE",
        "Ljava/lang/Runtime;",
    )];
    let methods = vec![
        code_method(&mut pool, code_name, ACC_PRIVATE, "<init>", "()V", 1, 1, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb7, object_init)
                .emit(&[0xb1]);
            code.finish()
        }),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            "getRuntime",
            "()Ljava/lang/Runtime;",
            1,
            0,
            {
                let mut code = Code::default();
                code.reference(0xb2, instance).emit(&[0xb0]);
                code.finish()
            },
        ),
        native_method(&mut pool, ACC_PUBLIC, "freeMemory", "()J"),
        native_method(&mut pool, ACC_PUBLIC, "totalMemory", "()J"),
        code_method(&mut pool, code_name, ACC_PUBLIC, "gc", "()V", 0, 1, {
            let mut code = Code::default();
            code.reference(0xb8, system_gc).emit(&[0xb1]);
            code.finish()
        }),
        code_method(&mut pool, code_name, ACC_STATIC, "<clinit>", "()V", 2, 0, {
            let mut code = Code::default();
            code.reference(0xbb, runtime_class)
                .emit(&[0x59])
                .reference(0xb7, runtime_init)
                .reference(0xb3, instance)
                .emit(&[0xb1]);
            code.finish()
        }),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_FINAL | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn java_lang_system() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/System");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let print_stream_class = pool.class("java/io/PrintStream");
    let print_stream_init = pool.method_ref("java/io/PrintStream", "<init>", "()V");
    let out = pool.field_ref("java/lang/System", "out", "Ljava/io/PrintStream;");
    let err = pool.field_ref("java/lang/System", "err", "Ljava/io/PrintStream;");
    let null_pointer_class = pool.class("java/lang/NullPointerException");
    let null_pointer_init = pool.method_ref("java/lang/NullPointerException", "<init>", "()V");
    let illegal_argument_class = pool.class("java/lang/IllegalArgumentException");
    let illegal_argument_init =
        pool.method_ref("java/lang/IllegalArgumentException", "<init>", "()V");
    let string_length = pool.method_ref("java/lang/String", "length", "()I");
    let get_property = pool.method_ref(
        "java/lang/System",
        "getProperty0",
        "(Ljava/lang/String;)Ljava/lang/String;",
    );

    let fields = vec![
        field(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC | ACC_FINAL,
            "out",
            "Ljava/io/PrintStream;",
        ),
        field(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC | ACC_FINAL,
            "err",
            "Ljava/io/PrintStream;",
        ),
    ];
    let methods = vec![
        code_method(&mut pool, code_name, ACC_PRIVATE, "<init>", "()V", 1, 1, {
            let mut code = Code::default();
            code.emit(&[0x2a])
                .reference(0xb7, object_init)
                .emit(&[0xb1]);
            code.finish()
        }),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "currentTimeMillis",
            "()J",
        ),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "gc", "()V"),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "identityHashCode",
            "(Ljava/lang/Object;)I",
        ),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "arraycopy",
            "(Ljava/lang/Object;ILjava/lang/Object;II)V",
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            "getProperty",
            "(Ljava/lang/String;)Ljava/lang/String;",
            2,
            1,
            {
                let mut code = Code::default();
                code.emit(&[0x2a]).jump(0xc7, "check_empty");
                code.reference(0xbb, null_pointer_class)
                    .emit(&[0x59])
                    .reference(0xb7, null_pointer_init)
                    .emit(&[0xbf]);
                code.label("check_empty")
                    .emit(&[0x2a])
                    .reference(0xb6, string_length)
                    .jump(0x9a, "property");
                code.reference(0xbb, illegal_argument_class)
                    .emit(&[0x59])
                    .reference(0xb7, illegal_argument_init)
                    .emit(&[0xbf]);
                code.label("property")
                    .emit(&[0x2a])
                    .reference(0xb8, get_property)
                    .emit(&[0xb0]);
                code.finish()
            },
        ),
        native_method(
            &mut pool,
            ACC_PRIVATE | ACC_STATIC,
            "getProperty0",
            "(Ljava/lang/String;)Ljava/lang/String;",
        ),
        code_method(&mut pool, code_name, ACC_STATIC, "<clinit>", "()V", 2, 0, {
            let mut code = Code::default();
            for stream in [out, err] {
                code.reference(0xbb, print_stream_class)
                    .emit(&[0x59])
                    .reference(0xb7, print_stream_init)
                    .reference(0xb3, stream);
            }
            code.emit(&[0xb1]);
            code.finish()
        }),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_FINAL | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn java_lang_runnable() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/Runnable");
    let super_class = pool.class("java/lang/Object");
    let methods = vec![abstract_method(&mut pool, ACC_PUBLIC, "run", "()V")];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_INTERFACE | ACC_ABSTRACT,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn java_lang_ref_reference() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/ref/Reference");
    let super_class = pool.class("java/lang/Object");
    let methods = vec![
        native_method(&mut pool, ACC_PROTECTED, "<init>", "(Ljava/lang/Object;)V"),
        native_method(&mut pool, ACC_PUBLIC, "get", "()Ljava/lang/Object;"),
        native_method(&mut pool, ACC_PUBLIC, "clear", "()V"),
    ];
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER | ACC_ABSTRACT,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn java_lang_ref_weak_reference() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/ref/WeakReference");
    let super_class = pool.class("java/lang/ref/Reference");
    let code_name = pool.utf8("Code");
    let reference_init =
        pool.method_ref("java/lang/ref/Reference", "<init>", "(Ljava/lang/Object;)V");
    let methods = vec![code_method(
        &mut pool,
        code_name,
        ACC_PUBLIC,
        "<init>",
        "(Ljava/lang/Object;)V",
        2,
        2,
        {
            let mut code = Code::default();
            code.emit(&[0x2a, 0x2b])
                .reference(0xb7, reference_init)
                .emit(&[0xb1]);
            code.finish()
        },
    )];
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
