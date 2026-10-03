//! CLDC Number conversions and Math declarations.

use super::{
    ACC_ABSTRACT, ACC_FINAL, ACC_PRIVATE, ACC_PUBLIC, ACC_STATIC, ACC_SUPER, ClassFile,
    ConstantPool, Member, abstract_method, code_method, native_method,
};

pub(crate) fn java_lang_number() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/Number");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let int_value = pool.method_ref("java/lang/Number", "intValue", "()I");

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
                0xb1,              // return
            ],
        ),
        abstract_method(&mut pool, ACC_PUBLIC, "intValue", "()I"),
        abstract_method(&mut pool, ACC_PUBLIC, "longValue", "()J"),
        abstract_method(&mut pool, ACC_PUBLIC, "floatValue", "()F"),
        abstract_method(&mut pool, ACC_PUBLIC, "doubleValue", "()D"),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "byteValue",
            "()B",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb6,
                (int_value >> 8) as u8,
                int_value as u8, // invokevirtual Number.intValue:()I
                0x91,            // i2b
                0xac,            // ireturn
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "shortValue",
            "()S",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb6,
                (int_value >> 8) as u8,
                int_value as u8, // invokevirtual Number.intValue:()I
                0x93,            // i2s
                0xac,            // ireturn
            ],
        ),
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

pub(crate) fn java_lang_math() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/lang/Math");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");

    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PRIVATE,
            "<init>",
            "()V",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb7,
                (object_init >> 8) as u8,
                object_init as u8, // invokespecial Object.<init>:()V
                0xb1,              // return
            ],
        ),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "sin", "(D)D"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "cos", "(D)D"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "tan", "(D)D"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "sqrt", "(D)D"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "ceil", "(D)D"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "floor", "(D)D"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "toRadians", "(D)D"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "toDegrees", "(D)D"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "min", "(FF)F"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "max", "(FF)F"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "min", "(DD)D"),
        native_method(&mut pool, ACC_PUBLIC | ACC_STATIC, "max", "(DD)D"),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            "abs",
            "(I)I",
            1,
            1,
            vec![
                0x1a, // iload_0
                0x9c, 0x00, 0x06, // ifge positive
                0x1a, // iload_0
                0x74, // ineg
                0xac, // ireturn
                0x1a, // iload_0
                0xac, // ireturn
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            "abs",
            "(J)J",
            4,
            2,
            vec![
                0x1e, // lload_0
                0x09, // lconst_0
                0x94, // lcmp
                0x9c, 0x00, 0x06, // ifge positive
                0x1e, // lload_0
                0x75, // lneg
                0xad, // lreturn
                0x1e, // lload_0
                0xad, // lreturn
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            "abs",
            "(F)F",
            2,
            1,
            vec![
                0x22, // fload_0
                0x0b, // fconst_0
                0x96, // fcmpg
                0x9e, 0x00, 0x05, // ifle non-positive
                0x22, // fload_0
                0xae, // freturn
                0x0b, // fconst_0
                0x22, // fload_0
                0x66, // fsub
                0xae, // freturn
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            "abs",
            "(D)D",
            4,
            2,
            vec![
                0x26, // dload_0
                0x0e, // dconst_0
                0x98, // dcmpg
                0x9e, 0x00, 0x05, // ifle non-positive
                0x26, // dload_0
                0xaf, // dreturn
                0x0e, // dconst_0
                0x26, // dload_0
                0x67, // dsub
                0xaf, // dreturn
            ],
        ),
        comparison_method(
            &mut pool,
            code_name,
            "min",
            "(II)I",
            0xa2,
            [0x1a, 0x1b],
            0xac,
        ),
        comparison_method(
            &mut pool,
            code_name,
            "min",
            "(JJ)J",
            0x9c,
            [0x1e, 0x20],
            0xad,
        ),
        comparison_method(
            &mut pool,
            code_name,
            "max",
            "(II)I",
            0xa4,
            [0x1a, 0x1b],
            0xac,
        ),
        comparison_method(
            &mut pool,
            code_name,
            "max",
            "(JJ)J",
            0x9e,
            [0x1e, 0x20],
            0xad,
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

pub(crate) fn comparison_method(
    pool: &mut ConstantPool,
    code_name: u16,
    name: &str,
    descriptor: &str,
    branch: u8,
    loads: [u8; 2],
    return_opcode: u8,
) -> Member {
    let is_long = descriptor == "(JJ)J";
    let [load_left, load_right] = loads;
    let mut code = vec![load_left, load_right];
    if is_long {
        code.push(0x94); // lcmp
    }
    code.extend_from_slice(&[
        branch,
        0x00,
        0x05, // choose right
        load_left,
        return_opcode,
        load_right,
        return_opcode,
    ]);
    code_method(
        pool,
        code_name,
        ACC_PUBLIC | ACC_STATIC,
        name,
        descriptor,
        if is_long { 4 } else { 2 },
        if is_long { 4 } else { 2 },
        code,
    )
}
