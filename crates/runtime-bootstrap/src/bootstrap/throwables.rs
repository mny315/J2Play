//! Throwable state and Java exception constructors.

use crate::bytecode_builder::Code;

use super::{
    ACC_PRIVATE, ACC_PUBLIC, ACC_SUPER, ClassFile, Constant, ConstantPool, Member, code_method,
    field, forwarding_constructor, native_method,
};

pub(crate) fn java_lang_throwable() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/Throwable");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let fill_trace = pool.method_ref("java/lang/Throwable", "fillInStackTrace0", "()V");
    let to_string = pool.method_ref("java/lang/Throwable", "toString", "()Ljava/lang/String;");
    let print_trace = pool.method_ref(
        "java/lang/Throwable",
        "printStackTrace0",
        "(Ljava/lang/String;)V",
    );
    let detail_message =
        pool.field_ref("java/lang/Throwable", "detailMessage", "Ljava/lang/String;");

    let fields = vec![field(
        &mut pool,
        ACC_PRIVATE,
        "detailMessage",
        "Ljava/lang/String;",
    )];
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "<init>",
            "()V",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb7,
                (object_init >> 8) as u8,
                object_init as u8, // invokespecial Object.<init>:()V
                0x2a,              // aload_0
                0xb7,
                (fill_trace >> 8) as u8,
                fill_trace as u8, // invokespecial Throwable.fillInStackTrace0:()V
                0xb1,             // return
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "<init>",
            "(Ljava/lang/String;)V",
            2,
            2,
            vec![
                0x2a, // aload_0
                0xb7,
                (object_init >> 8) as u8,
                object_init as u8, // invokespecial Object.<init>:()V
                0x2a,              // aload_0
                0x2b,              // aload_1
                0xb5,
                (detail_message >> 8) as u8,
                detail_message as u8, // putfield Throwable.detailMessage:String
                0x2a,                 // aload_0
                0xb7,
                (fill_trace >> 8) as u8,
                fill_trace as u8, // invokespecial Throwable.fillInStackTrace0:()V
                0xb1,             // return
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "getMessage",
            "()Ljava/lang/String;",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb4,
                (detail_message >> 8) as u8,
                detail_message as u8, // getfield Throwable.detailMessage:String
                0xb0,                 // areturn
            ],
        ),
        throwable_to_string(&mut pool, code_name),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "printStackTrace",
            "()V",
            2,
            1,
            {
                let mut code = Code::default();
                code.emit(&[0x2a, 0x2a])
                    .reference(0xb6, to_string)
                    .reference(0xb7, print_trace)
                    .emit(&[0xb1]);
                code.finish()
            },
        ),
        native_method(
            &mut pool,
            ACC_PRIVATE,
            "printStackTrace0",
            "(Ljava/lang/String;)V",
        ),
        native_method(&mut pool, ACC_PRIVATE, "fillInStackTrace0", "()V"),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "fillInStackTrace",
            "()Ljava/lang/Throwable;",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb7,
                (fill_trace >> 8) as u8,
                fill_trace as u8, // invokespecial Throwable.fillInStackTrace0:()V
                0x2a,             // aload_0
                0xb0,             // areturn
            ],
        ),
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

fn throwable_to_string(pool: &mut ConstantPool, code_name: u16) -> Member {
    let get_class = pool.method_ref("java/lang/Object", "getClass", "()Ljava/lang/Class;");
    let get_name = pool.method_ref("java/lang/Class", "getName", "()Ljava/lang/String;");
    let get_message = pool.method_ref("java/lang/Throwable", "getMessage", "()Ljava/lang/String;");
    let concat = pool.method_ref(
        "java/lang/String",
        "concat",
        "(Ljava/lang/String;)Ljava/lang/String;",
    );
    let string_index = pool.utf8(": ");
    let separator = pool.push(Constant::String { string_index });
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb6, get_class)
        .reference(0xb6, get_name)
        .emit(&[0x4c, 0x2a])
        .reference(0xb6, get_message)
        .emit(&[0x4d, 0x2c])
        .jump(0xc6, "class_only")
        .emit(&[0x2b])
        .reference(0x13, separator)
        .reference(0xb6, concat)
        .emit(&[0x2c])
        .reference(0xb6, concat)
        .emit(&[0xb0]);
    code.label("class_only").emit(&[0x2b, 0xb0]);
    code_method(
        pool,
        code_name,
        ACC_PUBLIC,
        "toString",
        "()Ljava/lang/String;",
        2,
        3,
        code.finish(),
    )
}

pub(crate) fn java_lang_throwable_subclass(spec: (&str, &str, bool)) -> ClassFile {
    let (name, parent, has_index_constructor) = spec;
    let mut pool = ConstantPool::new();
    let this_class = pool.class(name);
    let super_class = pool.class(parent);
    let code_name = pool.utf8("Code");
    let parent_init = pool.method_ref(parent, "<init>", "()V");
    let parent_message_init = pool.method_ref(parent, "<init>", "(Ljava/lang/String;)V");

    let mut methods = vec![
        forwarding_constructor(
            &mut pool,
            code_name,
            "()V",
            1,
            vec![
                0x2a, // aload_0
                0xb7,
                (parent_init >> 8) as u8,
                parent_init as u8, // invokespecial parent.<init>:()V
                0xb1,              // return
            ],
        ),
        forwarding_constructor(
            &mut pool,
            code_name,
            "(Ljava/lang/String;)V",
            2,
            vec![
                0x2a, // aload_0
                0x2b, // aload_1
                0xb7,
                (parent_message_init >> 8) as u8,
                parent_message_init as u8, // invokespecial parent.<init>:(String)V
                0xb1,                      // return
            ],
        ),
    ];

    if has_index_constructor {
        let string_value_of =
            pool.method_ref("java/lang/String", "valueOf", "(I)Ljava/lang/String;");
        methods.insert(
            1,
            forwarding_constructor(
                &mut pool,
                code_name,
                "(I)V",
                2,
                vec![
                    0x2a, // aload_0
                    0x1b, // iload_1
                    0xb8,
                    (string_value_of >> 8) as u8,
                    string_value_of as u8, // invokestatic String.valueOf:(I)String
                    0xb7,
                    (parent_message_init >> 8) as u8,
                    parent_message_init as u8, // invokespecial parent.<init>:(String)V
                    0xb1,                      // return
                ],
            ),
        );
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
