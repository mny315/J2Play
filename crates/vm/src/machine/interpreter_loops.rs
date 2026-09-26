use super::{
    Allocation, ArrayAccessKind, ArrayKind, Handle, HeapValue, MAIN_THREAD_ID, Machine, Method,
    Value,
};

mod recognition;
pub(super) use recognition::link_counted_array_fills;

/// Checked bulk execution of counted array fills and reverse copies.
///
/// The recognizer is entirely structural: it records no class, method, JAR or
/// archive identity. Only an exact control-flow and typed-field contract is
/// accepted, and execution falls back to the ordinary bytecodes whenever the
/// live state cannot be proven safe for a bulk write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CountedArrayFill {
    field_index: u16,
    index_local: u16,
    kind: ArrayFillLoop,
    start_pc: u32,
    exit_pc: u32,
    access: ArrayAccessKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArrayFillLoop {
    CountDown {
        count_local: u16,
        value_local: u16,
    },
    ToLength {
        value: i32,
    },
    ToLimit {
        limit_local: u16,
        value_local: u16,
    },
    ReverseCopy {
        counter_local: u16,
        limit_local: u16,
        source_local: u16,
        source_index_local: u16,
    },
}

impl Machine<'_, '_> {
    fn array_loop_budget(&self) -> u64 {
        // The first load was already counted; stop before the next host poll.
        (1_023 - self.execution.instructions % 1_024).min(
            self.limits
                .max_instructions
                .saturating_sub(self.execution.instructions),
        )
    }

    fn array_loop_target(&self, method: &Method, field_index: u16) -> Option<Handle> {
        let access = self.classes.static_field_fast_access(method, field_index)?;
        if !self
            .classes
            .initialized
            .contains_at(access.slots.declaring_class?)
        {
            return None;
        }
        match self
            .classes
            .static_fields
            .get_linked(access.slots.static_field?)?
        {
            Value::Reference(Some(array)) => Some(*array),
            _ => None,
        }
    }

    pub(in crate::machine) fn try_counted_array_fill(
        &mut self,
        method: &Method,
        encoded_fill: u16,
        locals: &mut [Option<Value>],
        stack_is_empty: bool,
    ) -> Option<usize> {
        if encoded_fill == 0
            || !stack_is_empty
            || method.max_stack < 3
            || self.execution.stack_slots.saturating_add(3) > self.limits.max_stack_slots
            || self.execution.tracing
            || self.execution.profiling
            || self.scheduler.current_thread != MAIN_THREAD_ID
            || !self.scheduler.scheduled_tasks.is_empty()
        {
            return None;
        }
        let fill = *self
            .program
            .counted_array_fills
            .get(usize::from(encoded_fill - 1))?;
        let (count_local, value_local) = match fill.kind {
            ArrayFillLoop::ToLimit { .. } => {
                return self.try_limit_array_fill(method, fill, locals);
            }
            ArrayFillLoop::ReverseCopy { .. } => {
                return self.try_reverse_array_copy(method, fill, locals);
            }
            ArrayFillLoop::ToLength { .. } => {
                return self.try_length_array_fill(method, fill, locals);
            }
            ArrayFillLoop::CountDown {
                count_local,
                value_local,
            } => (count_local, value_local),
        };
        let count_slot = usize::from(count_local);
        let index_slot = usize::from(fill.index_local);
        let value_slot = usize::from(value_local);
        let Value::Int(count) = locals.get(count_slot).copied().flatten()? else {
            return None;
        };

        let budget = self.array_loop_budget();
        if count < 0 {
            // The first iload has already been counted by the ordinary loop;
            // five more bytecodes perform the decrement and exit branch.
            if budget < 5 {
                return None;
            }
            locals[count_slot] = Some(Value::Int(count.wrapping_sub(1)));
            self.execution.instructions += 5;
            return usize::try_from(fill.exit_pc).ok();
        }

        let iterations_left = u64::try_from(count).ok()?.saturating_add(1);
        let chunk = iterations_left.min((budget.saturating_add(1)) / 12);
        let chunk = usize::try_from(chunk).ok()?;
        if chunk == 0 {
            return None;
        }
        let Value::Int(index) = locals.get(index_slot).copied().flatten()? else {
            return None;
        };
        let Value::Int(value) = locals.get(value_slot).copied().flatten()? else {
            return None;
        };
        let array = self.array_loop_target(method, fill.field_index)?;
        if self
            .heap
            .managed
            .array_fill_typed(array, index, chunk, fill.access, HeapValue::Int(value))
            .is_err()
        {
            return None;
        }

        let chunk_i32 = i32::try_from(chunk).ok()?;
        let mut next_count = count.wrapping_sub(chunk_i32);
        locals[index_slot] = Some(Value::Int(index.wrapping_add(chunk_i32)));
        let mut additional_instructions = u64::try_from(chunk).ok()? * 12 - 1;
        let completed_values = u64::try_from(chunk).ok()? == iterations_left;
        let finish_exit = completed_values && budget.saturating_sub(additional_instructions) >= 6;
        let next_pc = if finish_exit {
            next_count = next_count.wrapping_sub(1);
            additional_instructions += 6;
            usize::try_from(fill.exit_pc).ok()?
        } else {
            usize::try_from(fill.start_pc).ok()?
        };
        locals[count_slot] = Some(Value::Int(next_count));
        self.execution.instructions += additional_instructions;
        Some(next_pc)
    }

    fn try_limit_array_fill(
        &mut self,
        method: &Method,
        fill: CountedArrayFill,
        locals: &mut [Option<Value>],
    ) -> Option<usize> {
        let ArrayFillLoop::ToLimit {
            limit_local,
            value_local,
        } = fill.kind
        else {
            return None;
        };
        let index_slot = usize::from(fill.index_local);
        let Value::Int(index) = locals.get(index_slot).copied().flatten()? else {
            return None;
        };
        let Value::Int(limit) = locals.get(usize::from(limit_local)).copied().flatten()? else {
            return None;
        };
        // The dispatcher has already accounted for the initial iload.
        let budget = self.array_loop_budget();
        if index >= limit {
            if budget < 2 {
                return None;
            }
            self.execution.instructions += 2;
            return usize::try_from(fill.exit_pc).ok();
        }
        let remaining = usize::try_from(limit.checked_sub(index)?).ok()?;
        let chunk = remaining.min(usize::try_from((budget + 1) / 9).ok()?);
        if chunk == 0 {
            return None;
        }
        let Value::Int(value) = locals.get(usize::from(value_local)).copied().flatten()? else {
            return None;
        };
        let array = self.array_loop_target(method, fill.field_index)?;
        self.heap
            .managed
            .array_fill_typed(array, index, chunk, fill.access, HeapValue::Int(value))
            .ok()?;
        locals[index_slot] = Some(Value::Int(index.checked_add(i32::try_from(chunk).ok()?)?));
        let mut additional = u64::try_from(chunk).ok()? * 9 - 1;
        let next_pc = if chunk == remaining && budget - additional >= 3 {
            additional += 3;
            fill.exit_pc
        } else {
            fill.start_pc
        };
        self.execution.instructions += additional;
        usize::try_from(next_pc).ok()
    }

    fn try_reverse_array_copy(
        &mut self,
        method: &Method,
        fill: CountedArrayFill,
        locals: &mut [Option<Value>],
    ) -> Option<usize> {
        let ArrayFillLoop::ReverseCopy {
            counter_local,
            limit_local,
            source_local,
            source_index_local,
        } = fill.kind
        else {
            return None;
        };
        if method.max_stack < 4
            || self.execution.stack_slots.saturating_add(4) > self.limits.max_stack_slots
        {
            return None;
        }
        let int = |slot: u16| match locals.get(usize::from(slot)).copied().flatten()? {
            Value::Int(value) => Some(value),
            _ => None,
        };
        let counter = int(counter_local)?;
        let limit = int(limit_local)?;
        let budget = self.array_loop_budget();
        if counter >= limit {
            if budget < 2 {
                return None;
            }
            self.execution.instructions += 2;
            return usize::try_from(fill.exit_pc).ok();
        }
        let remaining = usize::try_from(limit.checked_sub(counter)?).ok()?;
        let chunk = remaining.min(usize::try_from((budget + 1) / 13).ok()?);
        if chunk == 0 {
            return None;
        }
        let destination_index = int(fill.index_local)?;
        let source_index = int(source_index_local)?;
        let chunk_i32 = i32::try_from(chunk).ok()?;
        let next_destination = destination_index.checked_add(chunk_i32)?;
        let next_source = source_index.checked_sub(chunk_i32)?;
        let next_counter = counter.checked_add(chunk_i32)?;
        let Value::Reference(Some(source)) =
            locals.get(usize::from(source_local)).copied().flatten()?
        else {
            return None;
        };
        let destination = self.array_loop_target(method, fill.field_index)?;
        // A reverse copy can observe its own writes when the arrays alias.
        if source == destination {
            return None;
        }
        let (
            Allocation::Array {
                kind: ArrayKind::Int,
                elements: source,
            },
            Allocation::Array {
                kind: ArrayKind::Int,
                elements: destination,
            },
        ) = self.heap.managed.get_pair_mut(source, destination).ok()?
        else {
            return None;
        };
        let source_values =
            source.get(usize::try_from(next_source).ok()?..usize::try_from(source_index).ok()?)?;
        let destination_values = destination.get_mut(
            usize::try_from(destination_index).ok()?..usize::try_from(next_destination).ok()?,
        )?;
        if source_values
            .iter()
            .any(|value| !matches!(value, HeapValue::Int(_)))
        {
            return None;
        }
        for (destination, source) in destination_values
            .iter_mut()
            .zip(source_values.iter().rev())
        {
            *destination = *source;
        }
        locals[usize::from(counter_local)] = Some(Value::Int(next_counter));
        locals[usize::from(fill.index_local)] = Some(Value::Int(next_destination));
        locals[usize::from(source_index_local)] = Some(Value::Int(next_source));
        let mut additional = u64::try_from(chunk).ok()? * 13 - 1;
        let next_pc = if chunk == remaining && budget - additional >= 3 {
            additional += 3;
            fill.exit_pc
        } else {
            fill.start_pc
        };
        self.execution.instructions += additional;
        usize::try_from(next_pc).ok()
    }

    fn try_length_array_fill(
        &mut self,
        method: &Method,
        fill: CountedArrayFill,
        locals: &mut [Option<Value>],
    ) -> Option<usize> {
        let ArrayFillLoop::ToLength { value } = fill.kind else {
            return None;
        };
        let index_slot = usize::from(fill.index_local);
        let Value::Int(index) = locals.get(index_slot).copied().flatten()? else {
            return None;
        };
        let array = self.array_loop_target(method, fill.field_index)?;
        let length = i32::try_from(self.heap.managed.array_length(array).ok()?).ok()?;
        let budget = self.array_loop_budget();
        if index >= length {
            if budget < 3 {
                return None;
            }
            self.execution.instructions += 3;
            return usize::try_from(fill.exit_pc).ok();
        }
        let remaining = usize::try_from(length.checked_sub(index)?).ok()?;
        let chunk = remaining.min(usize::try_from((budget + 1) / 10).ok()?);
        if chunk == 0 {
            return None;
        }
        self.heap
            .managed
            .array_fill_typed(array, index, chunk, fill.access, HeapValue::Int(value))
            .ok()?;
        locals[index_slot] = Some(Value::Int(index.checked_add(i32::try_from(chunk).ok()?)?));
        let mut additional = u64::try_from(chunk).ok()? * 10 - 1;
        let next_pc = if chunk == remaining && budget - additional >= 4 {
            additional += 4;
            fill.exit_pc
        } else {
            fill.start_pc
        };
        self.execution.instructions += additional;
        usize::try_from(next_pc).ok()
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/interpreter_loops/mod.rs"]
mod tests;
