//! Audited boundary for locally generated, verified numeric functions. This
//! module owns RX memory until the last callable is dropped. No raw pointers,
//! functions, or binary-deserialization APIs escape the crate.
#![allow(unsafe_code)]

use crate::compiler::RunStats;
use cranelift_jit::JITModule;
use cranelift_module::FuncId;
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use vm::acceleration::{CompiledIntegerMethod, IntegerMethodResult};

pub(crate) struct Image {
    module: Option<JITModule>,
    pub stats: Rc<Cell<RunStats>>,
}

impl Image {
    pub fn new(module: JITModule, stats: Rc<Cell<RunStats>>) -> Self {
        Self {
            module: Some(module),
            stats,
        }
    }

    pub fn module_mut(&mut self) -> &mut JITModule {
        self.module.as_mut().expect("live executable image")
    }

    pub fn method(self: &Rc<Self>, id: FuncId, parameters: usize) -> Rc<dyn CompiledIntegerMethod> {
        let pointer = self
            .module
            .as_ref()
            .expect("live executable image")
            .get_finalized_function(id);
        // SAFETY: the private compiler emits exactly this System V C signature,
        // verifies every path, and finalizes the module before calling this.
        // The retained Rc owns all referenced executable pages.
        let function = unsafe { std::mem::transmute::<*const u8, NumericFunction>(pointer) };
        Rc::new(NumericMethod {
            image: Rc::clone(self),
            function,
            parameters,
        })
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if let Some(module) = self.module.take() {
            // SAFETY: a call borrows NumericMethod, which retains this Image.
            // No function pointer escapes; the last Rc cannot die during a call.
            unsafe {
                module.free_memory();
            }
        }
    }
}

type NumericFunction = unsafe extern "C" fn(*const i32, u32) -> u64;

struct NumericMethod {
    image: Rc<Image>,
    function: NumericFunction,
    parameters: usize,
}

impl fmt::Debug for NumericMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledIntegerMethod")
            .field("parameters", &self.parameters)
            .finish_non_exhaustive()
    }
}

impl CompiledIntegerMethod for NumericMethod {
    fn execute(&self, arguments: &[i32], budget: u32) -> Option<IntegerMethodResult> {
        if arguments.len() != self.parameters || budget == 0 {
            return None;
        }
        let budget = budget.min(1_023);
        // SAFETY: verified code only loads these initialized parameter slots.
        // It performs no stores, calls, allocations, or guest memory access.
        // Every basic block consumes fuel before execution, including backedges.
        let encoded = unsafe { (self.function)(arguments.as_ptr(), budget) };
        let mut stats = self.image.stats.get();
        stats.attempts = stats.attempts.saturating_add(1);
        let completed = u32::try_from(encoded >> 32).ok()?.checked_sub(1);
        let result = completed
            .filter(|count| *count != 0 && *count <= budget)
            .map(|instructions| {
                stats.calls = stats.calls.saturating_add(1);
                stats.instructions = stats.instructions.saturating_add(u64::from(instructions));
                IntegerMethodResult {
                    value: u32::try_from(encoded & u64::from(u32::MAX))
                        .expect("masked result")
                        .cast_signed(),
                    instructions,
                }
            });
        if result.is_none() {
            stats.bailouts = stats.bailouts.saturating_add(1);
        }
        self.image.stats.set(stats);
        if stats.attempts >= 1_024 && stats.attempts.is_power_of_two() {
            eprintln!(
                "J2Play AOT: calls={} bytecodes={} bailouts={}",
                stats.calls, stats.instructions, stats.bailouts
            );
        }
        result
    }
}
