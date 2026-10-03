use super::*;
use crate::*;

#[test]
fn data_input_stream_constructor_saves_a_null_source_for_later_use() {
    let classes = production_bootstrap_classes();
    let stream = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("java/io/DataInputStream"))
        .unwrap();
    let constructor = stream
        .methods
        .iter()
        .find(|method| {
            stream.utf8(method.name_index) == Some("<init>")
                && stream.utf8(method.descriptor_index) == Some("(Ljava/io/InputStream;)V")
        })
        .unwrap();
    let Attribute::Code(code) = &constructor.attributes[0] else {
        panic!("DataInputStream constructor must have bytecode");
    };

    // CLDC 1.1 specifies that the constructor saves its argument for later
    // use. In particular, it does not eagerly reject null: the first
    // operation that dereferences the saved stream raises the NPE.
    assert_eq!(
        code.code,
        [0x2a, 0xb7, 0x00, 0x01, 0x2a, 0x2b, 0xb5, 0x00, 0x0a, 0xb1]
    );
}

#[test]
fn byte_string_constructor_defers_to_the_platform_default_encoding() {
    let classes = production_bootstrap_classes();
    let string = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("java/lang/String"))
        .unwrap();
    let constructor = string
        .methods
        .iter()
        .find(|method| {
            string.utf8(method.name_index) == Some("<init>")
                && string.utf8(method.descriptor_index) == Some("([BII)V")
        })
        .unwrap();
    let Attribute::Code(code) = &constructor.attributes[0] else {
        panic!("String byte constructor must have bytecode");
    };

    // A null internal encoding marker asks String.initBytes to resolve the
    // CLDC-mandated microedition.encoding property. The trailing nop keeps
    // the migrated constructor's branch offsets stable.
    assert!(code.code.ends_with(&[
        0x2a, 0x2b, 0x1c, 0x1d, 0x01, 0x00, 0xb7, 0x00, 0x1e, 0x57, 0xb1,
    ]));
}

#[test]
fn reader_reports_mark_as_unsupported_by_default() {
    let classes = production_bootstrap_classes();
    let reader = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("java/io/Reader"))
        .unwrap();
    let mark_supported = reader
        .methods
        .iter()
        .find(|method| {
            reader.utf8(method.name_index) == Some("markSupported")
                && reader.utf8(method.descriptor_index) == Some("()Z")
        })
        .unwrap();
    assert_eq!(mark_supported.access_flags, ACC_PUBLIC);
    let Attribute::Code(code) = &mark_supported.attributes[0] else {
        panic!("Reader.markSupported() must have bytecode");
    };
    assert_eq!(code.code, [0x03, 0xac]);
}

#[test]
fn hashtable_enumeration_snapshots_entries_before_mutation() {
    let classes = production_bootstrap_classes();
    let enumeration = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("java/util/ArrayEnumeration"))
        .unwrap();
    let constructor = enumeration
        .methods
        .iter()
        .find(|method| {
            enumeration.utf8(method.name_index) == Some("<init>")
                && enumeration.utf8(method.descriptor_index) == Some("([Ljava/lang/Object;I)V")
        })
        .unwrap();
    let Attribute::Code(code) = &constructor.attributes[0] else {
        panic!("ArrayEnumeration constructor must have bytecode");
    };

    assert!(
        code.code
            .windows(4)
            .any(|bytes| bytes == [0x1c, 0xbd, 0x00, 0x02]),
        "the enumeration must allocate a private snapshot"
    );
    assert!(
        code.code
            .windows(3)
            .any(|bytes| bytes == [0xb8, 0x00, 0x25]),
        "the enumeration must copy entries into its snapshot"
    );
    let Some(Constant::Methodref {
        class_index,
        name_and_type_index,
    }) = enumeration.constant_pool.get(37).and_then(Option::as_ref)
    else {
        panic!("ArrayEnumeration must reference System.arraycopy");
    };
    assert_eq!(
        enumeration.class_name(*class_index),
        Some("java/lang/System")
    );
    let Some(Constant::NameAndType {
        name_index,
        descriptor_index,
    }) = enumeration
        .constant_pool
        .get(usize::from(*name_and_type_index))
        .and_then(Option::as_ref)
    else {
        panic!("ArrayEnumeration arraycopy reference must have a name and type");
    };
    assert_eq!(enumeration.utf8(*name_index), Some("arraycopy"));
    assert_eq!(
        enumeration.utf8(*descriptor_index),
        Some("(Ljava/lang/Object;ILjava/lang/Object;II)V")
    );
}
