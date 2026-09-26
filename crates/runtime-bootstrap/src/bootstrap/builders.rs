//! Shared constant-pool, field, method and class declaration builders.

use super::{
    ACC_ABSTRACT, ACC_FINAL, ACC_INTERFACE, ACC_NATIVE, ACC_PUBLIC, ACC_STATIC, ACC_SUPER,
    Attribute, ClassFile, CodeAttribute, Constant, Member,
};

pub(crate) struct ConstantPool {
    entries: Vec<Option<Constant>>,
}
impl ConstantPool {
    pub(crate) fn new() -> Self {
        Self {
            entries: vec![None],
        }
    }

    pub(crate) fn push(&mut self, constant: Constant) -> u16 {
        let index = u16::try_from(self.entries.len())
            .expect("Rust bootstrap constant pool must fit in a JVM u16 index");
        self.entries.push(Some(constant));
        index
    }

    pub(crate) fn utf8(&mut self, value: &str) -> u16 {
        self.push(Constant::Utf8(value.to_owned()))
    }

    pub(crate) fn class(&mut self, name: &str) -> u16 {
        let name_index = self.utf8(name);
        self.push(Constant::Class { name_index })
    }

    fn name_and_type(&mut self, name: &str, descriptor: &str) -> u16 {
        let name_index = self.utf8(name);
        let descriptor_index = self.utf8(descriptor);
        self.push(Constant::NameAndType {
            name_index,
            descriptor_index,
        })
    }

    pub(crate) fn method_ref(&mut self, class: &str, name: &str, descriptor: &str) -> u16 {
        let class_index = self.class(class);
        let name_and_type_index = self.name_and_type(name, descriptor);
        self.push(Constant::Methodref {
            class_index,
            name_and_type_index,
        })
    }

    pub(crate) fn interface_method_ref(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
    ) -> u16 {
        let class_index = self.class(class);
        let name_and_type_index = self.name_and_type(name, descriptor);
        self.push(Constant::InterfaceMethodref {
            class_index,
            name_and_type_index,
        })
    }

    pub(crate) fn field_ref(&mut self, class: &str, name: &str, descriptor: &str) -> u16 {
        let class_index = self.class(class);
        let name_and_type_index = self.name_and_type(name, descriptor);
        self.push(Constant::Fieldref {
            class_index,
            name_and_type_index,
        })
    }

    pub(crate) fn finish(self) -> Vec<Option<Constant>> {
        self.entries
    }
}

pub(crate) fn forwarding_constructor(
    pool: &mut ConstantPool,
    code_name: u16,
    descriptor: &str,
    max_locals: u16,
    code: Vec<u8>,
) -> Member {
    code_method(
        pool, code_name, ACC_PUBLIC, "<init>", descriptor, 2, max_locals, code,
    )
}

pub(crate) fn field(pool: &mut ConstantPool, flags: u16, name: &str, descriptor: &str) -> Member {
    Member {
        access_flags: flags,
        name_index: pool.utf8(name),
        descriptor_index: pool.utf8(descriptor),
        attributes: Vec::new(),
    }
}

pub(crate) fn abstract_method(
    pool: &mut ConstantPool,
    flags: u16,
    name: &str,
    descriptor: &str,
) -> Member {
    Member {
        access_flags: flags | ACC_ABSTRACT,
        name_index: pool.utf8(name),
        descriptor_index: pool.utf8(descriptor),
        attributes: Vec::new(),
    }
}

pub(crate) fn native_method(
    pool: &mut ConstantPool,
    flags: u16,
    name: &str,
    descriptor: &str,
) -> Member {
    Member {
        access_flags: flags | ACC_NATIVE,
        name_index: pool.utf8(name),
        descriptor_index: pool.utf8(descriptor),
        attributes: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn code_method(
    pool: &mut ConstantPool,
    code_name: u16,
    flags: u16,
    name: &str,
    descriptor: &str,
    max_stack: u16,
    max_locals: u16,
    code: Vec<u8>,
) -> Member {
    Member {
        access_flags: flags,
        name_index: pool.utf8(name),
        descriptor_index: pool.utf8(descriptor),
        attributes: vec![Attribute::Code(CodeAttribute {
            name_index: code_name,
            max_stack,
            max_locals,
            code,
            exception_table: Vec::new(),
            attributes: Vec::new(),
        })],
    }
}

pub(crate) fn interface_class(name: &str, parents: &[&str], methods: &[(&str, &str)]) -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class(name);
    let super_class = pool.class("java/lang/Object");
    let interfaces = parents.iter().map(|parent| pool.class(parent)).collect();
    let methods = methods
        .iter()
        .map(|(name, descriptor)| abstract_method(&mut pool, ACC_PUBLIC, name, descriptor))
        .collect();
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_INTERFACE | ACC_ABSTRACT,
        this_class,
        super_class,
        interfaces,
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn native_class(
    name: &str,
    super_name: &str,
    interfaces: &[&str],
    fields: &[(u16, &str, &str)],
    methods: &[(u16, &str, &str)],
) -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class(name);
    let super_class = pool.class(super_name);
    let interfaces = interfaces
        .iter()
        .map(|interface| pool.class(interface))
        .collect();
    let fields = fields
        .iter()
        .map(|(flags, name, descriptor)| field(&mut pool, *flags, name, descriptor))
        .collect();
    let methods = methods
        .iter()
        .map(|(flags, name, descriptor)| native_method(&mut pool, *flags, name, descriptor))
        .collect();
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces,
        fields,
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn interface_class_with_fields(
    name: &str,
    parents: &[&str],
    fields: &[(&str, &str)],
    methods: &[(&str, &str)],
) -> ClassFile {
    let mut class = interface_class(name, parents, methods);
    for (name, descriptor) in fields {
        let name_index = append_constant(&mut class, Constant::Utf8((*name).to_owned()));
        let descriptor_index =
            append_constant(&mut class, Constant::Utf8((*descriptor).to_owned()));
        class.fields.push(Member {
            access_flags: ACC_PUBLIC | ACC_STATIC | ACC_FINAL,
            name_index,
            descriptor_index,
            attributes: Vec::new(),
        });
    }
    class
}

pub(crate) fn interface_class_with_string_fields(
    name: &str,
    parents: &[&str],
    fields: &[(&str, &str)],
    methods: &[(&str, &str)],
) -> ClassFile {
    let mut class = interface_class(name, parents, methods);
    let constant_value_name =
        append_constant(&mut class, Constant::Utf8("ConstantValue".to_owned()));
    for (name, value) in fields {
        let name_index = append_constant(&mut class, Constant::Utf8((*name).to_owned()));
        let descriptor_index =
            append_constant(&mut class, Constant::Utf8("Ljava/lang/String;".to_owned()));
        let string_index = append_constant(&mut class, Constant::Utf8((*value).to_owned()));
        let value_index = append_constant(&mut class, Constant::String { string_index });
        class.fields.push(Member {
            access_flags: ACC_PUBLIC | ACC_STATIC | ACC_FINAL,
            name_index,
            descriptor_index,
            attributes: vec![Attribute::Raw {
                name_index: constant_value_name,
                name: "ConstantValue".to_owned(),
                bytes: value_index.to_be_bytes().to_vec(),
            }],
        });
    }
    class
}

pub(crate) fn append_constant(class: &mut ClassFile, constant: Constant) -> u16 {
    let index = u16::try_from(class.constant_pool.len())
        .expect("Rust bootstrap constant pool must fit in a JVM u16 index");
    class.constant_pool.push(Some(constant));
    index
}
