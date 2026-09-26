//! Optional, host-owned acceleration of verified integer-only leaf methods.
//! The VM keeps instruction budgets, scheduling, and all guest effects.

use std::fmt::Debug;
use std::rc::Rc;

/// Bounded input to an optional compiler. No guest names or host paths are used.
#[derive(Clone, Debug)]
pub struct IntegerMethodSpec {
    pub code: Vec<u8>,
    pub integer_constants: Vec<(u16, i32)>,
    /// CP field index and reserved local slot for a VM-supplied integer.
    /// The compiler only reads the argument; it never accesses guest fields.
    pub static_integer_parameters: Vec<(u16, u8)>,
    pub parameter_slots: Vec<u8>,
    pub max_locals: u8,
    pub max_stack: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegerMethodResult {
    pub value: i32,
    /// Exact number of guest bytecodes performed, including the return.
    pub instructions: u32,
}

/// An implementation may only inspect the supplied integers. It must terminate
/// within `budget` bytecodes and return `None` without guest effects on bailout.
pub trait CompiledIntegerMethod: Debug {
    fn execute(&self, arguments: &[i32], budget: u32) -> Option<IntegerMethodResult>;
}

/// The platform owns compilation, executable memory, and any private cache.
pub trait IntegerMethodCompiler {
    fn compile(
        &mut self,
        methods: &[IntegerMethodSpec],
        cancelled: &dyn Fn() -> bool,
    ) -> Vec<Option<Rc<dyn CompiledIntegerMethod>>>;
}
