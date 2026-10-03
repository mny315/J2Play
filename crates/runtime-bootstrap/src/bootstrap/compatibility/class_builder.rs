use super::{
    ACC_ABSTRACT, ACC_NATIVE, ACC_STATIC, Attribute, ClassFile, CodeAttribute, Constant, Member,
    append_constant,
};

pub(crate) fn append_native_method(
    class: &mut ClassFile,
    flags: u16,
    name: &str,
    descriptor: &str,
) {
    let name_index = append_constant(class, Constant::Utf8(name.to_owned()));
    let descriptor_index = append_constant(class, Constant::Utf8(descriptor.to_owned()));
    class.methods.push(Member {
        access_flags: flags | ACC_NATIVE,
        name_index,
        descriptor_index,
        attributes: Vec::new(),
    });
}

pub(crate) fn method_mut<'a>(
    class: &'a mut ClassFile,
    name: &str,
    descriptor: &str,
) -> &'a mut Member {
    let method_index = class
        .methods
        .iter()
        .position(|method| {
            class.utf8(method.name_index) == Some(name)
                && class.utf8(method.descriptor_index) == Some(descriptor)
        })
        .unwrap_or_else(|| panic!("bootstrap method {name}{descriptor} must exist"));
    &mut class.methods[method_index]
}

pub(crate) fn method_code_mut<'a>(
    class: &'a mut ClassFile,
    name: &str,
    descriptor: &str,
) -> &'a mut CodeAttribute {
    method_mut(class, name, descriptor)
        .attributes
        .iter_mut()
        .find_map(|attribute| match attribute {
            Attribute::Code(code) => Some(code),
            Attribute::Raw { .. } => None,
        })
        .unwrap_or_else(|| panic!("bootstrap method {name}{descriptor} must have bytecode"))
}

pub(crate) fn replace_method_with_native(class: &mut ClassFile, name: &str, descriptor: &str) {
    let method = method_mut(class, name, descriptor);
    method.access_flags = (method.access_flags & !ACC_ABSTRACT) | ACC_NATIVE;
    method.attributes.clear();
}

pub(crate) fn delegate_data_input_stream_read_utf(class: &mut ClassFile) {
    let read_utf = append_method_ref(
        class,
        "java/io/DataInputStream",
        "readUTF",
        "(Ljava/io/DataInput;)Ljava/lang/String;",
    )
    .to_be_bytes();
    let code_name = append_constant(class, Constant::Utf8("Code".to_owned()));
    let method = method_mut(class, "readUTF", "()Ljava/lang/String;");
    method.access_flags &= !(ACC_ABSTRACT | ACC_NATIVE | ACC_STATIC);
    method.attributes = vec![Attribute::Code(CodeAttribute {
        name_index: code_name,
        max_stack: 1,
        max_locals: 1,
        code: vec![
            0x2a, // aload_0
            0xb8,
            read_utf[0],
            read_utf[1], // invokestatic DataInputStream.readUTF(DataInput)
            0xb0,        // areturn
        ],
        exception_table: Vec::new(),
        attributes: Vec::new(),
    })];
}

pub(crate) fn append_method_ref(
    class: &mut ClassFile,
    owner: &str,
    name: &str,
    descriptor: &str,
) -> u16 {
    let owner_class = append_class(class, owner);
    let name_index = append_constant(class, Constant::Utf8(name.to_owned()));
    let descriptor_index = append_constant(class, Constant::Utf8(descriptor.to_owned()));
    let name_and_type_index = append_constant(
        class,
        Constant::NameAndType {
            name_index,
            descriptor_index,
        },
    );
    append_constant(
        class,
        Constant::Methodref {
            class_index: owner_class,
            name_and_type_index,
        },
    )
}

pub(crate) fn find_method_ref(
    class: &ClassFile,
    owner: &str,
    name: &str,
    descriptor: &str,
) -> Option<u16> {
    class
        .constant_pool
        .iter()
        .enumerate()
        .find_map(|(index, constant)| {
            let Constant::Methodref {
                class_index,
                name_and_type_index,
            } = constant.as_ref()?
            else {
                return None;
            };
            let Constant::NameAndType {
                name_index,
                descriptor_index,
            } = class
                .constant_pool
                .get(usize::from(*name_and_type_index))?
                .as_ref()?
            else {
                return None;
            };
            (class.class_name(*class_index) == Some(owner)
                && class.utf8(*name_index) == Some(name)
                && class.utf8(*descriptor_index) == Some(descriptor))
            .then(|| u16::try_from(index).expect("bootstrap constant index fits u16"))
        })
}

pub(crate) fn append_field_ref(
    class: &mut ClassFile,
    owner: &str,
    name: &str,
    descriptor: &str,
) -> u16 {
    let owner_class = append_class(class, owner);
    let name_index = append_constant(class, Constant::Utf8(name.to_owned()));
    let descriptor_index = append_constant(class, Constant::Utf8(descriptor.to_owned()));
    let name_and_type_index = append_constant(
        class,
        Constant::NameAndType {
            name_index,
            descriptor_index,
        },
    );
    append_constant(
        class,
        Constant::Fieldref {
            class_index: owner_class,
            name_and_type_index,
        },
    )
}

pub(crate) fn find_field_ref(
    class: &ClassFile,
    owner: &str,
    name: &str,
    descriptor: &str,
) -> Option<u16> {
    class
        .constant_pool
        .iter()
        .enumerate()
        .find_map(|(index, constant)| {
            let Constant::Fieldref {
                class_index,
                name_and_type_index,
            } = constant.as_ref()?
            else {
                return None;
            };
            let Constant::NameAndType {
                name_index,
                descriptor_index,
            } = class
                .constant_pool
                .get(usize::from(*name_and_type_index))?
                .as_ref()?
            else {
                return None;
            };
            (class.class_name(*class_index) == Some(owner)
                && class.utf8(*name_index) == Some(name)
                && class.utf8(*descriptor_index) == Some(descriptor))
            .then(|| u16::try_from(index).expect("bootstrap constant index fits u16"))
        })
}

pub(crate) fn append_class(class: &mut ClassFile, name: &str) -> u16 {
    let name_index = append_constant(class, Constant::Utf8(name.to_owned()));
    append_constant(class, Constant::Class { name_index })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn append_code_method(
    class: &mut ClassFile,
    flags: u16,
    name: &str,
    descriptor: &str,
    max_stack: u16,
    max_locals: u16,
    code: Vec<u8>,
) {
    let code_name = append_constant(class, Constant::Utf8("Code".to_owned()));
    let name_index = append_constant(class, Constant::Utf8(name.to_owned()));
    let descriptor_index = append_constant(class, Constant::Utf8(descriptor.to_owned()));
    class.methods.push(Member {
        access_flags: flags,
        name_index,
        descriptor_index,
        attributes: vec![Attribute::Code(CodeAttribute {
            name_index: code_name,
            max_stack,
            max_locals,
            code,
            exception_table: Vec::new(),
            attributes: Vec::new(),
        })],
    });
}
