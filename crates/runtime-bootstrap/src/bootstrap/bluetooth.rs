//! JSR-82 value objects and explicitly unavailable radio access.

use super::{
    ACC_FINAL, ACC_PRIVATE, ACC_PUBLIC, ACC_STATIC, ACC_SUPER, ClassFile, ConstantPool,
    code_method, field, java_lang_throwable_subclass, native_method,
};

#[allow(clippy::too_many_lines)]
pub(crate) fn javax_bluetooth_uuid() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("javax/bluetooth/UUID");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let [init_hi, init_lo] = pool
        .method_ref("java/lang/Object", "<init>", "()V")
        .to_be_bytes();
    let [value_hi, value_lo] = pool
        .field_ref("javax/bluetooth/UUID", "value", "Ljava/lang/String;")
        .to_be_bytes();
    let [equals_hi, equals_lo] = pool
        .method_ref("java/lang/String", "equals", "(Ljava/lang/Object;)Z")
        .to_be_bytes();
    let [hash_hi, hash_lo] = pool
        .method_ref("java/lang/String", "hashCode", "()I")
        .to_be_bytes();
    let [class_hi, class_lo] = this_class.to_be_bytes();
    let mut methods = Vec::new();
    for (descriptor, canonical_descriptor, arguments) in [
        ("(J)V", "(J)Ljava/lang/String;", &[0x1f][..]), // lload_1
        (
            "(Ljava/lang/String;Z)V",
            "(Ljava/lang/String;Z)Ljava/lang/String;",
            &[0x2b, 0x1c][..],
        ),
    ] {
        let [canonical_hi, canonical_lo] = pool
            .method_ref("javax/bluetooth/UUID", "canonical", canonical_descriptor)
            .to_be_bytes();
        let mut code = vec![0x2a, 0xb7, init_hi, init_lo, 0x2a];
        code.extend_from_slice(arguments);
        code.extend_from_slice(&[
            0xb8,
            canonical_hi,
            canonical_lo,
            0xb5,
            value_hi,
            value_lo,
            0xb1,
        ]);
        methods.push(code_method(
            &mut pool, code_name, ACC_PUBLIC, "<init>", descriptor, 3, 3, code,
        ));
        methods.push(native_method(
            &mut pool,
            ACC_PRIVATE | ACC_STATIC,
            "canonical",
            canonical_descriptor,
        ));
    }
    methods.extend([
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "toString",
            "()Ljava/lang/String;",
            1,
            1,
            vec![0x2a, 0xb4, value_hi, value_lo, 0xb0],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "hashCode",
            "()I",
            1,
            1,
            vec![0x2a, 0xb4, value_hi, value_lo, 0xb6, hash_hi, hash_lo, 0xac],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC,
            "equals",
            "(Ljava/lang/Object;)Z",
            2,
            2,
            vec![
                0x2b, 0xc1, class_hi, class_lo, // value instanceof UUID (false for null)
                0x9a, 0x00, 0x05, // ifne compare
                0x03, 0xac, 0x2a, 0xb4, value_hi, value_lo, 0x2b, 0xc0, class_hi, class_lo, 0xb4,
                value_hi, value_lo, 0xb6, equals_hi, equals_lo, 0xac,
            ],
        ),
    ]);
    let fields = vec![field(
        &mut pool,
        ACC_PRIVATE | ACC_FINAL,
        "value",
        "Ljava/lang/String;",
    )];
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

pub(crate) fn javax_bluetooth_bluetooth_state_exception() -> ClassFile {
    java_lang_throwable_subclass((
        "javax/bluetooth/BluetoothStateException",
        "java/io/IOException",
        false,
    ))
}

pub(crate) fn javax_bluetooth_local_device() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("javax/bluetooth/LocalDevice");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let unavailable = pool.class("javax/bluetooth/BluetoothStateException");
    let unavailable_init =
        pool.method_ref("javax/bluetooth/BluetoothStateException", "<init>", "()V");
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            "getLocalDevice",
            "()Ljavax/bluetooth/LocalDevice;",
            2,
            0,
            vec![
                0xbb,
                (unavailable >> 8) as u8,
                unavailable as u8,
                0x59,
                0xb7,
                (unavailable_init >> 8) as u8,
                unavailable_init as u8,
                0xbf,
            ],
        ),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "getProperty",
            "(Ljava/lang/String;)Ljava/lang/String;",
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
