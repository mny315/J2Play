//! Bounded memoization of deterministic read-only bytecode leaves. Every heap
//! dependency is checked again; cached handles never become managed GC roots.
use super::{ClassState, Field, Heap, Method, Value};
use heap::{ArrayAccessKind, Handle};
use std::rc::Rc;

const ENTRIES: usize = 1024;
const MAX_READS: usize = 64;
const MAX_ARGS: usize = 8;
pub(super) const RESERVED_BYTES: usize = ENTRIES
    * (std::mem::size_of::<Entry>()
        + MAX_READS * std::mem::size_of::<Read>()
        + MAX_ARGS * std::mem::size_of::<Value>())
    + MAX_READS * std::mem::size_of::<Read>()
    + 4096;

#[derive(Debug)]
pub(super) enum Read {
    Array(Handle, i32, ArrayAccessKind, Value),
    Length(Handle, usize),
    Field(Handle, usize, Rc<Field>, Value),
    Static(usize, usize, Value),
    Initialized(usize),
}

impl Read {
    fn same_location(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Array(left, index, kind, value),
                Self::Array(right, other_index, other_kind, other_value),
            ) => {
                left == right
                    && index == other_index
                    && kind == other_kind
                    && same(*value, *other_value)
            }
            (Self::Length(left, length), Self::Length(right, other_length)) => {
                left == right && length == other_length
            }
            (
                Self::Field(left, slot, field, value),
                Self::Field(right, other_slot, other_field, other_value),
            ) => {
                left == right
                    && slot == other_slot
                    && Rc::ptr_eq(field, other_field)
                    && same(*value, *other_value)
            }
            (Self::Static(left, class, value), Self::Static(right, other_class, other_value)) => {
                left == right && class == other_class && same(*value, *other_value)
            }
            (Self::Initialized(left), Self::Initialized(right)) => left == right,
            _ => false,
        }
    }

    fn unchanged(&self, heap: &Heap, classes: &ClassState) -> bool {
        match self {
            Self::Initialized(slot) => classes.initialized.contains_at(*slot),
            Self::Array(handle, index, kind, expected) => heap
                .array_get_typed(*handle, *index, *kind)
                .is_ok_and(|value| same(value, *expected)),
            Self::Length(handle, expected) => heap.array_length(*handle) == Ok(*expected),
            Self::Field(handle, slot, field, expected) => heap
                .field_at(*handle, *slot, &field.field_token, &field.key)
                .is_ok_and(|value| same(value, *expected)),
            Self::Static(slot, class, expected) => {
                classes.initialized.contains_at(*class)
                    && classes
                        .static_fields
                        .get_linked(*slot)
                        .is_some_and(|value| same(*value, *expected))
            }
        }
    }
}

fn bits(value: Value) -> (u8, u64) {
    match value {
        Value::Int(value) => (0, u64::from(value as u32)),
        Value::Long(value) => (1, value as u64),
        Value::Float(value) => (2, u64::from(value.to_bits())),
        Value::Double(value) => (3, value.to_bits()),
        Value::Reference(None) => (4, 0),
        Value::Reference(Some(handle)) => (5, handle.to_raw()),
    }
}

fn same(left: Value, right: Value) -> bool {
    bits(left) == bits(right)
}

#[derive(Debug, Default)]
pub(super) struct Observation {
    reads: Vec<Read>,
    overflow: bool,
    pub(super) unsupported: bool,
}

impl Observation {
    pub(super) fn reset(&mut self) {
        self.reads.clear();
        self.overflow = false;
        self.unsupported = false;
    }

    pub(super) fn record(&mut self, read: Read) {
        if self.overflow
            || self
                .reads
                .iter()
                .any(|previous| previous.same_location(&read))
        {
            return;
        }
        if self.reads.len() == MAX_READS {
            self.overflow = true;
        } else {
            self.reads.push(read);
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Entry {
    method: usize,
    context: [usize; 4],
    args: Vec<Value>,
    observation: Observation,
    instructions: u64,
    value: i32,
}

impl Entry {
    pub(super) fn lookup(
        &self,
        method: &Method,
        args: &[Value],
        context: [usize; 4],
        heap: &Heap,
        classes: &ClassState,
    ) -> Option<(i32, u64)> {
        (self.instructions != 0
            && Some(self.method) == method.stack_key_id
            && self
                .context
                .iter()
                .zip(context)
                .all(|(&cached, current)| current <= cached)
            && self.args.len() == args.len()
            && self
                .args
                .iter()
                .zip(args)
                .all(|(&left, &right)| same(left, right))
            && self
                .observation
                .reads
                .iter()
                .all(|read| read.unchanged(heap, classes)))
        .then_some((self.value, self.instructions))
    }

    pub(super) fn finish(
        &mut self,
        method: &Method,
        args: &[Value],
        context: [usize; 4],
        value: i32,
        instructions: u64,
        observation: &mut Observation,
    ) {
        // An incomplete read set cannot validate a future hit. Keep the old
        // entry while the caller still uses this invocation's computed result.
        if observation.overflow {
            return;
        }
        std::mem::swap(&mut self.observation, observation);
        self.method = method
            .stack_key_id
            .expect("cache admission requires a linked method");
        self.context = context;
        self.args.clear();
        self.args.reserve_exact(args.len());
        self.args.extend_from_slice(args);
        self.value = value;
        self.instructions = instructions;
    }
}

#[derive(Debug, Default)]
pub(super) struct LeafCache {
    entries: Vec<Entry>,
    scratch: Observation,
    rejected: Vec<usize>,
    #[cfg(test)]
    pub(super) hits: usize,
}

impl LeafCache {
    pub(super) fn rejected(&self, method: &Method) -> bool {
        method
            .stack_key_id
            .is_some_and(|id| self.rejected.get(id & 255) == Some(&id))
    }

    pub(super) fn reject(&mut self, method: &Method) {
        if let Some(id) = method.stack_key_id {
            if self.rejected.is_empty() {
                self.rejected.resize(256, usize::MAX);
            }
            self.rejected[id & 255] = id;
        }
    }

    pub(super) fn take_scratch(&mut self) -> Observation {
        std::mem::take(&mut self.scratch)
    }
    pub(super) fn put_scratch(&mut self, mut observation: Observation) {
        observation.reset();
        self.scratch = observation;
    }

    pub(super) fn index(method: &Method, args: &[Value]) -> Option<usize> {
        if args.len() > MAX_ARGS || method.instructions.len() < 16 {
            return None;
        }
        let mut hash = method.stack_key_id? as u64;
        for &arg in args {
            let (tag, value) = bits(arg);
            hash =
                (hash.rotate_left(9) ^ value ^ u64::from(tag)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        }
        Some((hash ^ (hash >> 32)) as usize & (ENTRIES - 1))
    }

    pub(super) fn get(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)
    }

    pub(super) fn take(&mut self, index: usize) -> Entry {
        if self.entries.is_empty() {
            self.entries.resize_with(ENTRIES, Entry::default);
        }
        std::mem::take(&mut self.entries[index])
    }

    pub(super) fn put(&mut self, index: usize, entry: Entry) {
        self.entries[index] = entry;
    }
}

impl super::Machine<'_, '_> {
    /// Speculatively evaluates at most four frames and one interpreter quantum.
    /// Unresolved calls, initializers, writes, exceptions and instrumentation
    /// always use the ordinary interpreter. No partial result is committed.
    pub(super) fn evaluate_readonly_tree(
        &mut self,
        method: &Method,
        args: &[Value],
        budget: u64,
        context: [usize; 4],
        nesting: usize,
        observation: &mut Observation,
    ) -> Option<(i32, u64)> {
        let [depth, frames, frame_slots, stack_slots] = context;
        if (!method.readonly_leaf && !method.readonly_call_tree)
            || method.is_native
            || method.is_synchronized
            || !method.exception_table.is_empty()
            || method.descriptor.returns != Some(super::ValueKind::Int)
            || method.max_stack > 32
            || method.max_locals > 32
            || super::compatibility::compatibility_intrinsic_candidate(method)
        {
            observation.unsupported = true;
            return None;
        }
        if nesting == 4
            || budget == 0
            || depth > self.limits.max_frames
            || frames >= self.limits.max_frames
        {
            return None;
        }
        let nested_frame_slots = frame_slots.checked_add(method.max_locals)?;
        let nested_stack_slots = stack_slots.checked_add(method.max_stack)?;
        if nested_stack_slots > self.limits.max_stack_slots
            || nested_frame_slots
                .checked_mul(std::mem::size_of::<Option<Value>>())?
                .checked_add(self.program.runtime_bytes)?
                .checked_add(RESERVED_BYTES)?
                > self.limits.max_runtime_bytes
        {
            return None;
        }
        super::validate_call_arguments(method, args).ok()?;
        let mut locals = [None; 32];
        let mut local = 0;
        for &arg in args {
            super::set_local(&mut locals[..method.max_locals], local, arg).ok()?;
            local += arg.slots();
        }
        let mut stack = self
            .execution
            .frame_storage_pool
            .borrow_mut()
            .take_operand_stack(method.max_stack);
        let outcome = (|| {
            let mut pc = 0;
            let mut used = 0;
            let mut slots = 0;
            let mut wide = false;
            loop {
                let execute = if wide {
                    super::interpreter_batch::execute_inner::<false, true>
                } else {
                    super::interpreter_batch::execute_inner::<true, true>
                };
                let result = execute(
                    method,
                    &mut locals[..method.max_locals],
                    &mut stack,
                    pc,
                    slots,
                    method.max_stack,
                    budget.checked_sub(used)?,
                    &mut self.heap.managed,
                    &mut self.classes,
                    observation,
                    None,
                );
                used += result.instructions;
                slots = result.slots;
                if result.returned() {
                    return match stack.last() {
                        Some(Value::Int(value)) => Some((*value, used)),
                        _ => None,
                    };
                }
                if used >= budget {
                    return None;
                }
                let instruction_index = usize::from(*method.instruction_index.get(result.pc)?);
                let ins = method.runtime_instructions.get(instruction_index)?;
                if ins.opcode != 0xb8 {
                    if !wide {
                        wide = true;
                        pc = result.pc;
                        continue;
                    }
                    return None;
                }
                let index = u16::from_be_bytes([ins.operands[0], ins.operands[1]]);
                let (callee, class) = self.fixed_method_inline_cached(method, index, 0xb8)?;
                let class = class?;
                if !self.classes.initialized.contains_at(class) {
                    return None;
                }
                if !callee.is_static {
                    return None;
                }
                let start = stack
                    .len()
                    .checked_sub(callee.descriptor.parameters.len())?;
                observation.record(Read::Initialized(class));
                let (value, instructions) = self.evaluate_readonly_tree(
                    &callee,
                    &stack[start..],
                    budget.checked_sub(used + 1)?,
                    [
                        depth + 1,
                        frames + 1,
                        nested_frame_slots,
                        nested_stack_slots,
                    ],
                    nesting + 1,
                    observation,
                )?;
                used += 1 + instructions;
                stack.truncate(start);
                slots = stack.iter().map(|value| value.slots()).sum::<usize>();
                if slots >= method.max_stack {
                    return None;
                }
                stack.push(Value::Int(value));
                slots += 1;
                pc = ins.next_pc as usize;
            }
        })();
        self.execution
            .frame_storage_pool
            .borrow_mut()
            .put_operand_stack(stack);
        outcome
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/leaf_cache/mod.rs"]
mod tests;
