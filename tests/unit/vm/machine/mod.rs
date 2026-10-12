use super::*;

mod continuation;
mod instruction_metadata;
mod interpreter;
mod native_graphics;
#[path = "../../../support/process.rs"]
pub(crate) mod process;
mod runtime;
mod runtime_contracts;

pub(crate) use interpreter::{
    machine_thread_name, method, resumable_initializer_program, runtime_method, thread_class,
};
pub(crate) use runtime::{BoundedMmapiContext, program_with_exception};

struct CancellationContext {
    checks: std::rc::Rc<std::cell::Cell<usize>>,
    cancel_at: std::rc::Rc<std::cell::Cell<usize>>,
}

impl HostServices for CancellationContext {
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
    fn execution_cancelled(&self) -> bool {
        let checks = self.checks.get() + 1;
        self.checks.set(checks);
        checks >= self.cancel_at.get()
    }
}

pub(crate) fn test_class_definition(super_name: Option<&str>) -> Class {
    Class {
        super_name: super_name.map(str::to_owned),
        interfaces: Vec::new(),
        fields: Vec::new(),
        is_public: true,
        is_abstract: false,
        is_interface: false,
        no_arg_constructor: NoArgConstructor::Public,
    }
}

/// Minimal display storage for tests that need the complete bootstrap metadata.
fn program_with_bootstrap() -> Program {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(1, 1) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    program
}

fn program_with_core_natives() -> Program {
    let mut program = program_with_bootstrap();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    program
}

fn program_with_lcdui_natives() -> Program {
    let mut program = program_with_core_natives();
    program
        .native_registry_mut()
        .register(
            natives::NativeSignature::new(
                "javax/microedition/lcdui/Image",
                "createMutablePixels",
                "(II)[I",
            ),
            |context, arguments| {
                let [
                    natives::NativeValue::Int(width),
                    natives::NativeValue::Int(height),
                ] = arguments
                else {
                    panic!("invalid image dimensions");
                };
                assert!(*width > 0 && *height > 0);
                let length = usize::try_from(i64::from(*width) * i64::from(*height)).unwrap();
                assert!(length <= 4096);
                let pixels = context.allocate_java_int_array(&vec![-1; length])?;
                Ok(Some(natives::NativeValue::Reference(Some(pixels))))
            },
        )
        .unwrap();
    program
}
