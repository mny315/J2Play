use classfile::{Attribute, CodeAttribute, Constant, Member};

pub(crate) struct Pool {
    pub(crate) entries: Vec<Option<Constant>>,
}

impl Pool {
    pub(crate) fn new() -> Self {
        Self {
            entries: vec![None],
        }
    }

    pub(crate) fn push(&mut self, constant: Constant) -> u16 {
        let index = u16::try_from(self.entries.len()).unwrap();
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

    pub(crate) fn string(&mut self, value: &str) -> u16 {
        let string_index = self.utf8(value);
        self.push(Constant::String { string_index })
    }

    fn member_ref(&mut self, owner: &str, name: &str, descriptor: &str, field: bool) -> u16 {
        let class_index = self.class(owner);
        let name_index = self.utf8(name);
        let descriptor_index = self.utf8(descriptor);
        let name_and_type_index = self.push(Constant::NameAndType {
            name_index,
            descriptor_index,
        });
        self.push(if field {
            Constant::Fieldref {
                class_index,
                name_and_type_index,
            }
        } else {
            Constant::Methodref {
                class_index,
                name_and_type_index,
            }
        })
    }

    pub(crate) fn method(&mut self, owner: &str, name: &str, descriptor: &str) -> u16 {
        self.member_ref(owner, name, descriptor, false)
    }

    pub(crate) fn field(&mut self, owner: &str, name: &str, descriptor: &str) -> u16 {
        self.member_ref(owner, name, descriptor, true)
    }
}

pub(crate) struct MethodCode<'a> {
    pub(crate) flags: u16,
    pub(crate) name: &'a str,
    pub(crate) descriptor: &'a str,
    pub(crate) max_stack: u16,
    pub(crate) max_locals: u16,
    pub(crate) code: Vec<u8>,
}

pub(crate) fn code_method(pool: &mut Pool, code_name: u16, method: MethodCode<'_>) -> Member {
    Member {
        access_flags: method.flags,
        name_index: pool.utf8(method.name),
        descriptor_index: pool.utf8(method.descriptor),
        attributes: vec![Attribute::Code(CodeAttribute {
            name_index: code_name,
            max_stack: method.max_stack,
            max_locals: method.max_locals,
            code: method.code,
            exception_table: Vec::new(),
            attributes: Vec::new(),
        })],
    }
}

pub(crate) fn emit_reference(code: &mut Vec<u8>, opcode: u8, index: u16) {
    code.push(opcode);
    code.extend_from_slice(&index.to_be_bytes());
}

pub(crate) fn write_fixture(
    path: &std::path::Path,
    manifest: &[u8],
    class_name: &str,
    class: &[u8],
) {
    use std::io::Write;

    let file = std::fs::File::create(path).unwrap();
    let mut jar = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    jar.start_file("META-INF/MANIFEST.MF", options).unwrap();
    jar.write_all(manifest).unwrap();
    jar.start_file(class_name, options).unwrap();
    jar.write_all(class).unwrap();
    jar.finish().unwrap();
}

pub(crate) fn utf8(bytes: &mut Vec<u8>, value: &str) {
    bytes.push(1);
    bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}
