use crate::support::guest::{Pool, emit_reference as instruction};
use classfile::{Attribute, ClassFile, CodeAttribute, Constant, Member};
use diagnostics::{Category, EmuError};

mod input;
mod visibility;

const CONNECTION: &str = "javax/microedition/io/file/FileConnectionImpl";

#[derive(Clone, Copy, PartialEq)]
enum Mutation {
    None,
    Truncate,
    Rename,
    Delete,
}

fn expect_int(code: &mut Vec<u8>, expected: u8) {
    code.extend_from_slice(&[0x10, expected, 0x9f, 0, 5, 0x02, 0xac]);
}

fn rejects_io(
    code: &mut Vec<u8>,
    handlers: &mut Vec<classfile::ExceptionHandler>,
    catch_type: u16,
    operation: impl FnOnce(&mut Vec<u8>),
) {
    let start_pc = u16::try_from(code.len()).unwrap();
    operation(code);
    let end_pc = u16::try_from(code.len()).unwrap();
    code.extend_from_slice(&[0x02, 0xac, 0x57]);
    handlers.push(classfile::ExceptionHandler {
        start_pc,
        end_pc,
        handler_pc: end_pc + 2,
        catch_type,
    });
}

#[allow(clippy::too_many_lines)]
fn fixture(offset: u8, mutation: Mutation, retry: bool) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/FileOutputChecks");
    let super_class = pool.class("java/lang/Object");
    let connection = pool.class(CONNECTION);
    let constructor = pool.method(CONNECTION, "<init>", "(Ljava/lang/String;I)V");
    let open = pool.method(CONNECTION, "openOutputStream", "(J)Ljava/io/OutputStream;");
    let write = pool.method("java/io/OutputStream", "write", "(I)V");
    let flush = pool.method("java/io/OutputStream", "flush", "()V");
    let close = pool.method("java/io/OutputStream", "close", "()V");
    let trim = pool.method(CONNECTION, "truncate", "(J)V");
    let exception = pool.class("java/io/IOException");
    let mut handlers = Vec::new();
    let string_index = pool.utf8("file:///owned");
    let url = pool.push(Constant::String { string_index });
    let mut code = Vec::new();
    instruction(&mut code, 0xbb, connection);
    code.extend_from_slice(&[0x59, 0x12, u8::try_from(url).unwrap(), 0x05]);
    instruction(&mut code, 0xb7, constructor);
    code.extend_from_slice(&[0x4b, 0x2a, 0x10, offset, 0x85]);
    instruction(&mut code, 0xb6, open);
    code.push(0x4c);
    rejects_io(&mut code, &mut handlers, exception, |code| {
        code.extend_from_slice(&[0x2a, 0x09]);
        instruction(code, 0xb6, open);
        code.push(0x57);
    });
    code.extend_from_slice(&[0x2b, 0x10, b'X']);
    instruction(&mut code, 0xb6, write);
    let start = u16::try_from(code.len()).unwrap();
    code.push(0x2b);
    instruction(&mut code, 0xb6, flush);
    let end = u16::try_from(code.len()).unwrap();
    if retry {
        code.extend_from_slice(&[0xa7, 0, 4, 0x57]);
        handlers.push(classfile::ExceptionHandler {
            start_pc: start,
            end_pc: end,
            handler_pc: end + 3,
            catch_type: pool.class("java/io/IOException"),
        });
    }
    code.push(0x2b);
    instruction(&mut code, 0xb6, flush);
    code.extend_from_slice(&[0x2b, 0x10, b'Y']);
    instruction(&mut code, 0xb6, write);
    if mutation == Mutation::Truncate {
        code.extend_from_slice(&[0x2a, 0x0a]);
        instruction(&mut code, 0xb6, trim);
    }
    if matches!(mutation, Mutation::Rename | Mutation::Delete) {
        code.push(0x2a);
        if mutation == Mutation::Rename {
            let string_index = pool.utf8("moved");
            let name = pool.push(Constant::String { string_index });
            code.extend_from_slice(&[0x12, u8::try_from(name).unwrap()]);
            instruction(
                &mut code,
                0xb6,
                pool.method(CONNECTION, "rename", "(Ljava/lang/String;)V"),
            );
        } else {
            instruction(&mut code, 0xb6, pool.method(CONNECTION, "delete", "()V"));
        }
        rejects_io(&mut code, &mut handlers, exception, |code| {
            code.extend_from_slice(&[0x2b, 0x10, b'!']);
            instruction(code, 0xb6, write);
        });
        if mutation == Mutation::Rename {
            code.extend_from_slice(&[0x2a, 0x09]);
            instruction(&mut code, 0xb6, open);
            code.push(0x4c);
        }
    }
    if mutation != Mutation::Delete {
        code.extend_from_slice(&[0x2b, 0x10, b'Z']);
        instruction(&mut code, 0xb6, write);
    }
    for _ in 0..2 {
        code.push(0x2b);
        instruction(&mut code, 0xb6, close);
    }
    rejects_io(&mut code, &mut handlers, exception, |code| {
        code.extend_from_slice(&[0x2b, 0x10, b'?']);
        instruction(code, 0xb6, write);
    });
    rejects_io(&mut code, &mut handlers, exception, |code| {
        code.push(0x2b);
        instruction(code, 0xb6, flush);
    });
    if mutation != Mutation::Delete {
        code.extend_from_slice(&[0x2a, 0x09]);
        instruction(&mut code, 0xb6, open);
        instruction(&mut code, 0xb6, close);
    }
    code.extend_from_slice(&[0x03, 0xac]);
    owned_class(pool, this_class, super_class, code, handlers)
}

fn owned_class(
    mut pool: Pool,
    this_class: u16,
    super_class: u16,
    code: Vec<u8>,
    handlers: Vec<classfile::ExceptionHandler>,
) -> ClassFile {
    let name_index = pool.utf8("run");
    let descriptor_index = pool.utf8("()I");
    let code_name = pool.utf8("Code");
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x21,
        this_class,
        super_class,
        interfaces: vec![],
        fields: vec![],
        methods: vec![Member {
            access_flags: 9,
            name_index,
            descriptor_index,
            attributes: vec![Attribute::Code(CodeAttribute {
                name_index: code_name,
                max_stack: 5,
                max_locals: 4,
                code,
                exception_table: handlers,
                attributes: vec![],
            })],
        }],
        attributes: vec![],
    }
}

struct Context {
    url: String,
    data: Vec<u8>,
    snapshots: Vec<Vec<u8>>,
    fail_next: bool,
    revision: u64,
    reads: usize,
    fail_read: Option<usize>,
}

impl natives::HostServices for Context {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
    fn gcf_file_read(&mut self, url: &str) -> Result<Vec<u8>, EmuError> {
        assert_eq!(url, self.url);
        self.reads += 1;
        if self.fail_read == Some(self.reads) {
            return Err(EmuError::new(
                Category::Api,
                "file-io",
                "owned injected read failure",
            ));
        }
        Ok(self.data.clone())
    }
    fn gcf_file_revision(&self) -> Result<u64, EmuError> {
        Ok(self.revision)
    }
    fn gcf_file_output_offset(&mut self, url: &str, offset: u64) -> Result<u64, EmuError> {
        assert_eq!(url, self.url);
        Ok(offset.min(u64::try_from(self.data.len()).unwrap()))
    }
    fn gcf_file_write_at(&mut self, url: &str, data: &[u8], offset: u64) -> Result<(), EmuError> {
        self.revision = self.revision.wrapping_add(1);
        assert_eq!(url, self.url);
        if std::mem::take(&mut self.fail_next) {
            return Err(EmuError::new(
                Category::Api,
                "file-io",
                "owned injected write failure",
            ));
        }
        let offset = usize::try_from(offset).unwrap();
        assert!(offset <= self.data.len());
        self.data
            .resize(self.data.len().max(offset + data.len()), 0);
        self.data[offset..offset + data.len()].copy_from_slice(data);
        self.snapshots.push(self.data.clone());
        Ok(())
    }
    fn gcf_file_write(&mut self, _: &str, data: &[u8], _: bool) -> Result<(), EmuError> {
        self.revision = self.revision.wrapping_add(1);
        self.data = data.to_vec();
        self.snapshots.push(self.data.clone());
        Ok(())
    }
    fn gcf_file_truncate(&mut self, _: &str, size: u64) -> Result<(), EmuError> {
        self.revision = self.revision.wrapping_add(1);
        self.data.truncate(usize::try_from(size).unwrap());
        self.snapshots.push(self.data.clone());
        Ok(())
    }
    fn gcf_file_rename(&mut self, url: &str, name: &str) -> Result<String, EmuError> {
        self.revision = self.revision.wrapping_add(1);
        assert_eq!(url, self.url);
        self.url = format!("file:///{name}");
        Ok(self.url.clone())
    }
    fn gcf_file_delete(&mut self, url: &str) -> Result<(), EmuError> {
        self.revision = self.revision.wrapping_add(1);
        assert_eq!(url, self.url);
        self.data.clear();
        self.snapshots.push(self.data.clone());
        Ok(())
    }
}

#[test]
fn file_output_flush_preserves_tail_and_advances_only_after_success() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "file_output_flush_preserves_tail_and_advances_only_after_success"
    )) {
        return;
    }
    for (offset, mutation, retry, expected) in [
        (
            0,
            Mutation::None,
            false,
            vec![b"Xbcdef".to_vec(), b"XYZdef".to_vec()],
        ),
        (
            0,
            Mutation::None,
            true,
            vec![b"Xbcdef".to_vec(), b"XYZdef".to_vec()],
        ),
        (
            100,
            Mutation::None,
            false,
            vec![b"abcdefX".to_vec(), b"abcdefXYZ".to_vec()],
        ),
        (
            0,
            Mutation::Truncate,
            false,
            vec![
                b"Xbcdef".to_vec(),
                b"XYcdef".to_vec(),
                b"X".to_vec(),
                b"XZ".to_vec(),
            ],
        ),
        (
            0,
            Mutation::Rename,
            false,
            vec![b"Xbcdef".to_vec(), b"XYcdef".to_vec(), b"ZYcdef".to_vec()],
        ),
        (
            0,
            Mutation::Delete,
            false,
            vec![b"Xbcdef".to_vec(), b"XYcdef".to_vec(), vec![]],
        ),
    ] {
        let limits = vm::Limits::default();
        let mut program = vm::Program::new();
        for class in runtime_bootstrap::production_bootstrap_classes() {
            program.add_class(&class, &limits).unwrap();
        }
        program
            .add_class(&fixture(offset, mutation, retry), &limits)
            .unwrap();
        cldc::register_core_natives(program.native_registry_mut()).unwrap();
        gcf::register_natives(program.native_registry_mut()).unwrap();
        let mut context = Context {
            url: "file:///owned".into(),
            data: b"abcdef".to_vec(),
            snapshots: vec![],
            fail_next: retry,
            revision: 0,
            reads: 0,
            fail_read: None,
        };
        let execution = program
            .execute_with_context(
                "fixtures/FileOutputChecks",
                "run",
                "()I",
                limits,
                false,
                &mut context,
            )
            .unwrap();
        assert_eq!(execution.value, Some(vm::Value::Int(0)));
        assert_eq!(context.snapshots, expected);
    }
}
