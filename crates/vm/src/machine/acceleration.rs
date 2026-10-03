use super::{CallOutcome, Constant, Machine, Method, Program, Value, ValueKind};
use crate::acceleration::{IntegerMethodCompiler, IntegerMethodSpec};

#[derive(Debug)]
pub(super) struct PreparedIntegerMethod {
    compiled: std::rc::Rc<dyn crate::acceleration::CompiledIntegerMethod>,
    static_fields: Vec<u16>,
}

impl Program {
    /// Prepares a bounded set of numeric leaf candidates before guest execution.
    /// Every rejected or cancelled candidate retains its interpreter path.
    pub fn prepare_integer_methods(
        &mut self,
        compiler: &mut dyn IntegerMethodCompiler,
        cancelled: &dyn Fn() -> bool,
    ) -> usize {
        self.integer_methods.clear();
        for method in self.methods.values_mut() {
            method.compiled_integer = None;
        }
        let mut candidates = self.methods.iter().collect::<Vec<_>>();
        // Stable order also makes a bounded preparation reproducible.
        candidates.sort_unstable_by_key(|(key, _)| *key);
        let mut keys = Vec::new();
        let mut specs = Vec::new();
        for (key, method) in candidates {
            if cancelled() || specs.len() == 256 {
                break;
            }
            if let Some(spec) = integer_spec(method) {
                keys.push(key.clone());
                specs.push(spec);
            }
        }
        let compiled = compiler.compile(&specs, cancelled);
        for ((key, compiled), spec) in keys.into_iter().zip(compiled).zip(specs) {
            if let Some(compiled) = compiled
                && let Some(method) = self.methods.get_mut(&key)
            {
                method.compiled_integer = Some(self.integer_methods.len());
                self.integer_methods.push(PreparedIntegerMethod {
                    compiled,
                    static_fields: spec
                        .static_integer_parameters
                        .iter()
                        .map(|&(field, _)| field)
                        .collect(),
                });
            }
        }
        self.integer_methods.len()
    }
}

fn integer_spec(method: &Method) -> Option<IntegerMethodSpec> {
    if method.is_native
        || method.is_synchronized
        || !method.exception_table.is_empty()
        || method.descriptor.returns != Some(ValueKind::Int)
        || method.descriptor.parameters.len() > 8
        || method
            .descriptor
            .parameters
            .iter()
            .any(|kind| *kind != ValueKind::Int)
        || !(8..=512).contains(&method.instructions.len())
        || method.code.len() > 2_048
        || method.max_locals > 32
        || method.max_stack > 32
        || method.instructions.iter().any(|instruction| {
            !matches!(instruction.opcode,
                0x00 | 0x02..=0x08 | 0x10..=0x13 | 0x15 | 0x1a..=0x1d |
                0x36 | 0x3b..=0x3e | 0x57..=0x60 | 0x64 | 0x68 | 0x6c |
                0x70 | 0x74 | 0x78 | 0x7a | 0x7c | 0x7e | 0x80 | 0x82 |
                0x84 | 0x91..=0x93 | 0x99..=0xa4 | 0xa7 | 0xac | 0xb2)
        })
    {
        return None;
    }
    let first = usize::from(!method.is_static);
    let mut parameter_slots = (first..first + method.descriptor.parameters.len())
        .map(u8::try_from)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let mut integer_constants = Vec::new();
    let mut static_integer_parameters = Vec::new();
    for instruction in method.instructions.iter() {
        if instruction.opcode == 0xb2 {
            let field = u16::from_be_bytes(instruction.operands.get(..2)?.try_into().ok()?);
            if super::resolve_field(&method.constants, field)
                .ok()?
                .descriptor
                != "I"
            {
                return None;
            }
            if !static_integer_parameters
                .iter()
                .any(|&(index, _)| index == field)
            {
                let slot =
                    u8::try_from(method.max_locals + static_integer_parameters.len()).ok()?;
                if slot >= 32 || parameter_slots.len() == 8 {
                    return None;
                }
                static_integer_parameters.push((field, slot));
                parameter_slots.push(slot);
            }
        }
        let index = match instruction.opcode {
            0x12 => u16::from(*instruction.operands.first()?),
            0x13 => u16::from_be_bytes(instruction.operands.get(..2)?.try_into().ok()?),
            _ => continue,
        };
        let Constant::Integer(value) = method.constants.get(usize::from(index))?.as_ref()? else {
            return None;
        };
        if !integer_constants.iter().any(|(other, _)| *other == index) {
            integer_constants.push((index, *value));
        }
    }
    let max_locals = u8::try_from(method.max_locals + static_integer_parameters.len()).ok()?;
    Some(IntegerMethodSpec {
        code: method.code.as_ref().clone(),
        integer_constants,
        static_integer_parameters,
        parameter_slots,
        max_locals,
        max_stack: u8::try_from(method.max_stack).ok()?,
    })
}

impl Machine<'_, '_> {
    fn leaf_instruction_budget(&self, method: &Method) -> Option<u64> {
        if self.execution.tracing
            || self.execution.profiling
            || self.scheduler.suspend_requested
            || self.scheduler.driver_host_poll_yield
            || self.native_context.execution_cancelled()
            || self.native_context.execution_suspended()
            || self.execution.stack_slots.saturating_add(method.max_stack)
                > self.limits.max_stack_slots
        {
            return None;
        }
        // Never cross the interpreter's cancellation, timer, or thread quantum
        // boundary. Bailout is side-effect-free, so interpretation restarts at PC 0.
        let mut budget = (1_023 - (self.execution.instructions & 1_023)).min(
            self.limits
                .max_instructions
                .saturating_sub(self.execution.instructions),
        );
        if self.scheduler.current_thread != super::MAIN_THREAD_ID {
            budget = budget.min(self.scheduler.quantum_remaining.saturating_sub(1));
        }
        if !self.scheduler.scheduled_tasks.is_empty() {
            if self.scheduler.timer_poll_requested {
                return None;
            }
            budget = budget.min(self.scheduler.timer_poll_countdown);
        }
        (budget != 0).then_some(budget)
    }

    fn account_leaf_instructions(&mut self, instructions: u64) {
        self.execution.instructions += instructions;
        if self.scheduler.current_thread != super::MAIN_THREAD_ID {
            self.scheduler.quantum_remaining -= instructions;
        }
        if !self.scheduler.scheduled_tasks.is_empty() {
            self.scheduler.timer_poll_countdown -= instructions;
        }
    }

    pub(super) fn try_readonly_leaf(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
    ) -> Option<CallOutcome> {
        if (!method.readonly_leaf && !method.readonly_call_tree)
            || method.is_native
            || method.is_synchronized
            || method.max_locals > 32
            || method.max_stack > 32
            || depth > self.limits.max_frames
            || self.execution.call_stack.len() >= self.limits.max_frames
            || !method.exception_table.is_empty()
            || self.native_context.vm_flight_recorder_enabled()
            || super::compatibility::compatibility_intrinsic_candidate(method)
        {
            return None;
        }
        let bytes = self
            .execution
            .frame_slots
            .checked_add(method.max_locals)?
            .checked_mul(std::mem::size_of::<Option<Value>>())?
            .checked_add(self.program.runtime_bytes)?;
        if bytes > self.limits.max_runtime_bytes {
            return None;
        }
        if method.compiled_integer.is_some() {
            return self.try_compiled_integer(method, args);
        }
        if method.readonly_call_tree || method.instructions.len() >= 16 {
            if method.descriptor.returns == Some(ValueKind::Int) {
                return self.try_cached_readonly_tree(method, args, depth, bytes);
            }
            if !method.readonly_leaf {
                return None;
            }
        }
        let budget = self.leaf_instruction_budget(method)?;
        if budget < method.instructions.len() as u64 {
            return None;
        }
        // This method cannot write guest state, allocate, or call another
        // method. A partial batch can therefore be discarded and replayed by
        // the ordinary interpreter, including its exact exception/limit path.
        let mut locals = [None; 32];
        let mut local = 0;
        for &argument in args {
            super::set_local(&mut locals[..method.max_locals], local, argument).ok()?;
            local += argument.slots();
        }
        let mut stack = self
            .execution
            .frame_storage_pool
            .borrow_mut()
            .take_operand_stack(method.max_stack);
        let result = super::interpreter_batch::execute(
            method,
            &mut locals[..method.max_locals],
            &mut stack,
            0,
            0,
            method.max_stack,
            budget,
            &mut self.heap.managed,
            &mut self.classes,
        );
        let value = if result.returned() {
            match method.descriptor.returns {
                Some(kind) => stack
                    .last()
                    .copied()
                    .filter(|value| value.kind() == kind)
                    .map(Some),
                None => Some(None),
            }
        } else {
            None
        };
        self.execution
            .frame_storage_pool
            .borrow_mut()
            .put_operand_stack(stack);
        let value = value?;
        self.account_leaf_instructions(result.instructions);
        Some(CallOutcome::Return(value))
    }

    fn try_cached_readonly_tree(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
        bytes: usize,
    ) -> Option<CallOutcome> {
        if self.execution.leaf_cache.rejected(method)
            || bytes.checked_add(super::leaf_cache::RESERVED_BYTES)? > self.limits.max_runtime_bytes
        {
            return None;
        }
        let budget = self.leaf_instruction_budget(method)?;
        let index = super::leaf_cache::LeafCache::index(method, args)?;
        let context = [
            depth,
            self.execution.call_stack.len(),
            self.execution.frame_slots,
            self.execution.stack_slots,
        ];
        if let Some((value, instructions)) =
            self.execution.leaf_cache.get(index).and_then(|entry| {
                entry.lookup(method, args, context, &self.heap.managed, &self.classes)
            })
        {
            if instructions > budget {
                return None;
            }
            #[cfg(test)]
            {
                self.execution.leaf_cache.hits += 1;
            }
            self.account_leaf_instructions(instructions);
            return Some(CallOutcome::Return(Some(Value::Int(value))));
        }
        let mut entry = self.execution.leaf_cache.take(index);
        // A poll boundary or an unresolved callee must not evict a useful old
        // result. Collect a replacement separately and commit only on success.
        let mut observation = self.execution.leaf_cache.take_scratch();
        let evaluated =
            self.evaluate_readonly_tree(method, args, budget, context, 0, &mut observation);
        if let Some((value, instructions)) = evaluated {
            entry.finish(method, args, context, value, instructions, &mut observation);
        } else if observation.unsupported {
            self.execution.leaf_cache.reject(method);
        }
        self.execution.leaf_cache.put_scratch(observation);
        self.execution.leaf_cache.put(index, entry);
        let (value, instructions) = evaluated?;
        self.account_leaf_instructions(instructions);
        Some(CallOutcome::Return(Some(Value::Int(value))))
    }

    pub(super) fn try_compiled_integer(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Option<CallOutcome> {
        let prepared = self.program.integer_methods.get(method.compiled_integer?)?;
        let budget = self.leaf_instruction_budget(method)?;
        let args = args.get(usize::from(!method.is_static)..)?;
        let integer_count = args.len() + prepared.static_fields.len();
        if args.len() != method.descriptor.parameters.len() || integer_count > 8 {
            return None;
        }
        let mut integers = [0; 8];
        for (destination, argument) in integers.iter_mut().zip(args) {
            let Value::Int(value) = argument else {
                return None;
            };
            *destination = *value;
        }
        // No host call or scheduler boundary can occur in the compiled leaf.
        // Re-read mutable fields on every invocation; decline unresolved or
        // uninitialized fields so the bytecodes retain initialization/errors.
        for (index, &field) in prepared.static_fields.iter().enumerate() {
            let access = self.classes.static_field_fast_access(method, field)?;
            if !access
                .slots
                .declaring_class
                .is_some_and(|slot| self.classes.initialized.contains_at(slot))
            {
                return None;
            }
            let Value::Int(value) = access
                .slots
                .static_field
                .and_then(|slot| self.classes.static_fields.get_linked(slot))
                .copied()?
            else {
                return None;
            };
            integers[args.len() + index] = value;
        }
        let result = prepared
            .compiled
            .execute(&integers[..integer_count], u32::try_from(budget).ok()?)?;
        let instructions = u64::from(result.instructions);
        if instructions == 0 || instructions > budget {
            return None;
        }
        self.account_leaf_instructions(instructions);
        Some(CallOutcome::Return(Some(Value::Int(result.value))))
    }
}
