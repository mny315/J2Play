//! Bounded reuse of cleared locals and operand stacks between guest calls.

use super::Value;
use std::cell::RefCell;
use std::rc::Rc;

const FRAME_STORAGE_POOL_MAX_VECTORS: usize = 64;
const FRAME_STORAGE_POOL_MAX_SLOTS: usize = 131_072;

#[derive(Debug, Default)]
pub(super) struct FrameStoragePool {
    locals: Vec<Vec<Option<Value>>>,
    operand_stacks: Vec<Vec<Value>>,
    retained_slots: usize,
}

impl FrameStoragePool {
    pub(super) fn take_locals(&mut self, len: usize) -> Vec<Option<Value>> {
        let mut locals = self.locals.pop().unwrap_or_default();
        self.retained_slots = self.retained_slots.saturating_sub(locals.capacity());
        locals.resize(len, None);
        locals
    }

    pub(super) fn take_operand_stack(&mut self, capacity: usize) -> Vec<Value> {
        let mut stack = self.operand_stacks.pop().unwrap_or_default();
        self.retained_slots = self.retained_slots.saturating_sub(stack.capacity());
        if stack.capacity() < capacity {
            stack.reserve_exact(capacity - stack.len());
        }
        stack
    }

    pub(super) fn put_locals(&mut self, mut locals: Vec<Option<Value>>) {
        locals.clear();
        self.put_slots(locals.capacity(), |pool| pool.locals.push(locals));
    }

    pub(super) fn put_operand_stack(&mut self, mut stack: Vec<Value>) {
        stack.clear();
        self.put_slots(stack.capacity(), |pool| pool.operand_stacks.push(stack));
    }

    fn put_slots(&mut self, capacity: usize, put: impl FnOnce(&mut Self)) {
        let pooled_vectors = self.locals.len().saturating_add(self.operand_stacks.len());
        let Some(retained_slots) = self.retained_slots.checked_add(capacity) else {
            return;
        };
        if capacity == 0
            || pooled_vectors >= FRAME_STORAGE_POOL_MAX_VECTORS
            || retained_slots > FRAME_STORAGE_POOL_MAX_SLOTS
        {
            return;
        }
        self.retained_slots = retained_slots;
        put(self);
    }
}

#[derive(Debug)]
pub(super) struct PooledFrameStorage {
    pub(super) locals: Vec<Option<Value>>,
    pub(super) stack: Vec<Value>,
    pub(super) pool: Rc<RefCell<FrameStoragePool>>,
}

impl PooledFrameStorage {
    pub(super) fn take(
        pool: Rc<RefCell<FrameStoragePool>>,
        locals_len: usize,
        stack_capacity: usize,
    ) -> Self {
        let (locals, stack) = {
            let mut storage = pool.borrow_mut();
            (
                storage.take_locals(locals_len),
                storage.take_operand_stack(stack_capacity),
            )
        };
        Self {
            locals,
            stack,
            pool,
        }
    }
}

impl Drop for PooledFrameStorage {
    fn drop(&mut self) {
        let mut pool = self.pool.borrow_mut();
        pool.put_operand_stack(std::mem::take(&mut self.stack));
        pool.put_locals(std::mem::take(&mut self.locals));
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/frame_storage.rs"]
mod tests;
