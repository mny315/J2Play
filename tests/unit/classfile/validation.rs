use super::*;
use crate::CodeAttribute;

#[test]
fn method_angle_brackets_are_limited_to_initializer_names() {
    for name in ["<init>", "<clinit>", "method"] {
        validate_unqualified_name(name, true, 12).unwrap();
    }
    for name in ["<", ">", "a<b>", "<other>"] {
        assert_eq!(
            validate_unqualified_name(name, true, 12)
                .unwrap_err()
                .code(),
            "invalid-member-name"
        );
    }
    for name in ["", "a.b", "a;b", "a[b", "a/b"] {
        for method in [false, true] {
            assert_eq!(
                validate_unqualified_name(name, method, 12)
                    .unwrap_err()
                    .code(),
                "invalid-member-name"
            );
        }
    }
}

#[test]
fn validates_field_and_method_descriptors() {
    for descriptor in ["I", "[[B", "Ljava/lang/String;"] {
        validate_field_descriptor(descriptor, 17).unwrap();
    }
    for descriptor in ["()V", "(IJLjava/lang/String;[B)Z"] {
        validate_method_descriptor(descriptor, 19).unwrap();
    }
    for descriptor in ["", "V", "Lbad.name;", "Ljava//lang/Object;", "[V"] {
        assert_eq!(
            validate_field_descriptor(descriptor, 17)
                .unwrap_err()
                .code(),
            "invalid-descriptor"
        );
    }
    for descriptor in ["V", "(V)V", "()", "([V)V"] {
        assert_eq!(
            validate_method_descriptor(descriptor, 19)
                .unwrap_err()
                .code(),
            "invalid-descriptor"
        );
    }
}

#[test]
fn array_descriptor_depth_is_iterative_and_limited_to_255() {
    let maximum = format!("{}I", "[".repeat(255));
    validate_field_descriptor(&maximum, 0).unwrap();

    for descriptor in [
        format!("{}I", "[".repeat(256)),
        format!("{}I", "[".repeat(65_536)),
    ] {
        assert_eq!(
            validate_field_descriptor(&descriptor, 0)
                .unwrap_err()
                .code(),
            "invalid-descriptor"
        );
    }
}

#[test]
fn rejects_invalid_flag_combinations() {
    assert_eq!(
        validate_class_flags(0x0010 | 0x0400, 8).unwrap_err().code(),
        "invalid-access-flags"
    );
    assert_eq!(
        validate_member_flags(MemberKind::Field, 0x0010 | 0x0040, 20)
            .unwrap_err()
            .code(),
        "invalid-access-flags"
    );
    assert_eq!(
        validate_member_flags(MemberKind::Method, 0x0400 | 0x0100, 20)
            .unwrap_err()
            .code(),
        "invalid-access-flags"
    );
}

#[test]
fn enforces_code_attribute_count() {
    assert_eq!(
        validate_member_attributes(MemberKind::Method, 0, &[], 30)
            .unwrap_err()
            .code(),
        "invalid-code-count"
    );
    let code = Attribute::Code(CodeAttribute {
        name_index: 1,
        max_stack: 0,
        max_locals: 0,
        code: vec![0xB1],
        exception_table: Vec::new(),
        attributes: Vec::new(),
    });
    assert_eq!(
        validate_member_attributes(MemberKind::Method, 0x0100, &[code], 30)
            .unwrap_err()
            .code(),
        "invalid-code-count"
    );
}
