//! Per-session class initialization, static storage and bounded linkage caches.

use super::{
    Field, Handle, Method, MethodDescriptor, MethodKey, Program, Value, ValueKind, value_reference,
};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

mod checkpoint;

#[derive(Debug, Default)]
pub(super) struct ConstantPoolRuntimeCache {
    // Only handles are cached; canonical payloads stay rooted by HeapState.
    pub(super) string_literals: SparseConstantCache<Handle>,
    pub(super) resolved_fields: SparseConstantCache<Rc<Field>>,
    pub(super) resolved_method_refs: SparseConstantCache<Rc<ResolvedMethodRef>>,
    pub(super) special_method_targets: SparseConstantCache<Rc<Method>>,
    pub(super) static_method_targets: SparseConstantCache<Rc<Method>>,
    pub(super) virtual_method_targets: SparseConstantCache<VirtualMethodTarget>,
}

const CONSTANT_CACHE_PAGE_ENTRIES: usize = 16;

#[derive(Debug)]
struct ConstantCachePage<T> {
    number: u16,
    slots: Box<[Option<T>; CONSTANT_CACHE_PAGE_ENTRIES]>,
}

#[derive(Debug)]
pub(super) struct SparseConstantCache<T> {
    pages: Vec<ConstantCachePage<T>>,
    // Interpreter loops usually revisit the same small constant-pool page.
    // Remembering it avoids a binary search on every resolved field or call
    // without making a hostile high constant-pool index allocate dense space.
    last_page: Cell<Option<usize>>,
}

impl<T> Default for SparseConstantCache<T> {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            last_page: Cell::new(None),
        }
    }
}

impl<T> SparseConstantCache<T> {
    pub(super) fn get(&self, index: u16) -> Option<&T> {
        let page_number = index / CONSTANT_CACHE_PAGE_ENTRIES as u16;
        if let Some(page) = self.last_page.get()
            && self
                .pages
                .get(page)
                .is_some_and(|cached| cached.number == page_number)
        {
            return self.pages[page].slots[usize::from(index) % CONSTANT_CACHE_PAGE_ENTRIES]
                .as_ref();
        }
        let page = self
            .pages
            .binary_search_by_key(&page_number, |page| page.number)
            .ok()?;
        self.last_page.set(Some(page));
        self.pages[page].slots[usize::from(index) % CONSTANT_CACHE_PAGE_ENTRIES].as_ref()
    }

    pub(super) fn insert(&mut self, index: u16, value: T) {
        let page_number = index / CONSTANT_CACHE_PAGE_ENTRIES as u16;
        let page = match self
            .pages
            .binary_search_by_key(&page_number, |page| page.number)
        {
            Ok(page) => page,
            Err(page) => {
                self.pages.insert(
                    page,
                    ConstantCachePage {
                        number: page_number,
                        slots: Box::new(std::array::from_fn(|_| None)),
                    },
                );
                page
            }
        };
        self.pages[page].slots[usize::from(index) % CONSTANT_CACHE_PAGE_ENTRIES] = Some(value);
        self.last_page.set(Some(page));
    }
}

const FIELD_INLINE_CACHE_ENTRIES: usize = 4_096;
const METHOD_REF_INLINE_CACHE_ENTRIES: usize = 2_048;
const FIXED_METHOD_INLINE_CACHE_ENTRIES: usize = 2_048;

#[derive(Debug)]
struct MethodRefInlineCacheEntry {
    constant_pool_id: usize,
    index: u16,
    reference: Rc<ResolvedMethodRef>,
}

/// Bounded direct-mapped front cache for symbolic invoke references.
///
/// The sparse constant-pool cache remains authoritative. This small front
/// cache only avoids its page search for recurring call sites; full keys make
/// collisions ordinary misses rather than observable VM state.
#[derive(Debug)]
pub(super) struct MethodRefInlineCache {
    entries: Box<[Option<MethodRefInlineCacheEntry>]>,
}

impl Default for MethodRefInlineCache {
    fn default() -> Self {
        Self {
            entries: (0..METHOD_REF_INLINE_CACHE_ENTRIES).map(|_| None).collect(),
        }
    }
}

impl MethodRefInlineCache {
    fn entry(constant_pool_id: usize, index: u16) -> usize {
        constant_pool_id
            .wrapping_mul(31)
            .wrapping_add(usize::from(index))
            % METHOD_REF_INLINE_CACHE_ENTRIES
    }

    pub(super) fn get(
        &self,
        constant_pool_id: usize,
        index: u16,
    ) -> Option<&Rc<ResolvedMethodRef>> {
        let cached = self.entries[Self::entry(constant_pool_id, index)].as_ref()?;
        (cached.constant_pool_id == constant_pool_id && cached.index == index)
            .then_some(&cached.reference)
    }

    pub(super) fn insert(
        &mut self,
        constant_pool_id: usize,
        index: u16,
        reference: Rc<ResolvedMethodRef>,
    ) {
        self.entries[Self::entry(constant_pool_id, index)] = Some(MethodRefInlineCacheEntry {
            constant_pool_id,
            index,
            reference,
        });
    }
}

#[derive(Debug)]
struct FixedMethodInlineCacheEntry {
    constant_pool_id: usize,
    index: u16,
    opcode: u8,
    method: Rc<Method>,
    declaring_class: Option<usize>,
}

/// Bounded direct-mapped front cache for already linked static and special
/// call targets. The sparse per-pool cache remains authoritative on misses.
#[derive(Debug)]
pub(super) struct FixedMethodInlineCache {
    entries: Box<[Option<FixedMethodInlineCacheEntry>]>,
}

impl Default for FixedMethodInlineCache {
    fn default() -> Self {
        Self {
            entries: (0..FIXED_METHOD_INLINE_CACHE_ENTRIES)
                .map(|_| None)
                .collect(),
        }
    }
}

impl FixedMethodInlineCache {
    fn entry(constant_pool_id: usize, index: u16, opcode: u8) -> usize {
        constant_pool_id
            .wrapping_mul(31)
            .wrapping_add(usize::from(index))
            .wrapping_add(usize::from(opcode).wrapping_mul(131))
            % FIXED_METHOD_INLINE_CACHE_ENTRIES
    }

    pub(super) fn get(
        &self,
        constant_pool_id: usize,
        index: u16,
        opcode: u8,
    ) -> Option<(&Rc<Method>, Option<usize>)> {
        let cached = self.entries[Self::entry(constant_pool_id, index, opcode)].as_ref()?;
        (cached.constant_pool_id == constant_pool_id
            && cached.index == index
            && cached.opcode == opcode)
            .then_some((&cached.method, cached.declaring_class))
    }

    pub(super) fn insert(
        &mut self,
        constant_pool_id: usize,
        index: u16,
        opcode: u8,
        method: Rc<Method>,
        declaring_class: Option<usize>,
    ) {
        self.entries[Self::entry(constant_pool_id, index, opcode)] =
            Some(FixedMethodInlineCacheEntry {
                constant_pool_id,
                index,
                opcode,
                method,
                declaring_class,
            });
    }
}

#[derive(Debug)]
struct FieldInlineCacheEntry {
    constant_pool_id: usize,
    index: u16,
    kind: ValueKind,
    is_static: bool,
    field: Rc<Field>,
    runtime_slots: FieldRuntimeSlots,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct FieldRuntimeSlots {
    pub(super) static_field: Option<usize>,
    pub(super) declaring_class: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct StaticFieldFastAccess {
    pub(super) kind: ValueKind,
    pub(super) slots: FieldRuntimeSlots,
}

#[derive(Debug)]
pub(super) struct FieldInlineCache {
    entries: Box<[Option<FieldInlineCacheEntry>]>,
}

impl Default for FieldInlineCache {
    fn default() -> Self {
        Self {
            entries: (0..FIELD_INLINE_CACHE_ENTRIES).map(|_| None).collect(),
        }
    }
}

impl FieldInlineCache {
    fn entry(constant_pool_id: usize, index: u16) -> usize {
        constant_pool_id
            .wrapping_mul(31)
            .wrapping_add(usize::from(index))
            % FIELD_INLINE_CACHE_ENTRIES
    }

    pub(super) fn get_resolved(
        &self,
        constant_pool_id: usize,
        index: u16,
    ) -> Option<(&Rc<Field>, FieldRuntimeSlots)> {
        let cached = self.entries[Self::entry(constant_pool_id, index)].as_ref()?;
        (cached.constant_pool_id == constant_pool_id && cached.index == index)
            .then_some((&cached.field, cached.runtime_slots))
    }

    pub(super) fn get_static_access(
        &self,
        constant_pool_id: usize,
        index: u16,
    ) -> Option<StaticFieldFastAccess> {
        let cached = self.entries[Self::entry(constant_pool_id, index)].as_ref()?;
        (cached.constant_pool_id == constant_pool_id && cached.index == index && cached.is_static)
            .then_some(StaticFieldFastAccess {
                kind: cached.kind,
                slots: cached.runtime_slots,
            })
    }

    pub(super) fn insert_resolved(
        &mut self,
        constant_pool_id: usize,
        index: u16,
        field: Rc<Field>,
        runtime_slots: FieldRuntimeSlots,
    ) {
        self.entries[Self::entry(constant_pool_id, index)] = Some(FieldInlineCacheEntry {
            constant_pool_id,
            index,
            kind: field.kind,
            is_static: field.is_static,
            field,
            runtime_slots,
        });
    }
}

#[derive(Debug, Default)]
pub(super) struct StaticFields {
    slots_by_key: HashMap<Arc<str>, usize>,
    pub(super) linked: Vec<Option<Value>>,
    synthetic: HashMap<String, Value>,
}

impl StaticFields {
    fn with_program(program: &Program) -> Self {
        let mut slots_by_key = HashMap::new();
        for field in program
            .classes
            .values()
            .flat_map(|class| class.fields.iter())
            .filter(|field| field.is_static)
        {
            let slot = slots_by_key.len();
            slots_by_key.insert(field.key.clone(), slot);
        }
        Self {
            linked: vec![None; slots_by_key.len()],
            slots_by_key,
            synthetic: HashMap::new(),
        }
    }

    fn slot(&self, key: &str) -> Option<usize> {
        self.slots_by_key.get(key).copied()
    }

    pub(super) fn get(&self, key: &str) -> Option<&Value> {
        if let Some(slot) = self.slot(key) {
            return self.linked.get(slot).and_then(Option::as_ref);
        }
        self.synthetic.get(key)
    }

    pub(super) fn get_at(&self, slot: usize, key: &str) -> Option<&Value> {
        self.linked
            .get(slot)
            .and_then(Option::as_ref)
            .or_else(|| self.get(key))
    }

    pub(super) fn get_linked(&self, slot: usize) -> Option<&Value> {
        self.linked.get(slot).and_then(Option::as_ref)
    }

    pub(super) fn get_linked_mut(&mut self, slot: usize) -> Option<&mut Value> {
        self.linked.get_mut(slot).and_then(Option::as_mut)
    }

    pub(super) fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        if let Some(slot) = self.slot(key) {
            return self.linked.get_mut(slot).and_then(Option::as_mut);
        }
        self.synthetic.get_mut(key)
    }

    pub(super) fn insert(&mut self, key: String, value: Value) -> Option<Value> {
        if let Some(slot) = self.slot(&key) {
            return self
                .linked
                .get_mut(slot)
                .and_then(|entry| entry.replace(value));
        }
        self.synthetic.insert(key, value)
    }

    pub(super) fn insert_at(&mut self, slot: usize, key: &str, value: Value) -> Option<Value> {
        if let Some(entry) = self.linked.get_mut(slot) {
            return entry.replace(value);
        }
        self.synthetic.insert(key.to_owned(), value)
    }

    fn values(&self) -> impl Iterator<Item = &Value> {
        self.linked
            .iter()
            .filter_map(Option::as_ref)
            .chain(self.synthetic.values())
    }
}

impl Extend<(String, Value)> for StaticFields {
    fn extend<T: IntoIterator<Item = (String, Value)>>(&mut self, iter: T) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct InitializedClasses {
    slots_by_name: HashMap<String, usize>,
    pub(super) linked: Vec<bool>,
    synthetic: HashSet<String>,
}

impl InitializedClasses {
    fn with_program(program: &Program) -> Self {
        let slots_by_name = program
            .classes
            .keys()
            .enumerate()
            .map(|(slot, name)| (name.clone(), slot))
            .collect::<HashMap<_, _>>();
        Self {
            linked: vec![false; slots_by_name.len()],
            slots_by_name,
            synthetic: HashSet::new(),
        }
    }

    pub(super) fn slot(&self, name: &str) -> Option<usize> {
        self.slots_by_name.get(name).copied()
    }

    pub(super) fn contains(&self, name: &str) -> bool {
        self.slot(name)
            .and_then(|slot| self.linked.get(slot))
            .copied()
            .unwrap_or_else(|| self.synthetic.contains(name))
    }

    pub(super) fn contains_at(&self, slot: usize) -> bool {
        self.linked.get(slot).copied().unwrap_or(false)
    }

    pub(super) fn insert(&mut self, name: String) -> bool {
        if let Some(slot) = self.slot(&name) {
            let was_initialized = self.linked[slot];
            self.linked[slot] = true;
            return !was_initialized;
        }
        self.synthetic.insert(name)
    }
}

#[derive(Debug, Default)]
pub(super) struct ClassState {
    // Constant-pool linking is immutable after Program construction. A compact
    // pool ID and sparse cp-indexed pages keep hot field/invoke bytecodes out
    // of SipHash and tuple-key HashMap lookups. Pages are allocated only for
    // used indexes, preventing a hostile high cp index from creating a large
    // empty vector.
    pub(super) constant_pool_caches: Vec<ConstantPoolRuntimeCache>,
    // A small bounded front cache keeps repeated field bytecodes out of the
    // sparse-page search. Full pool ID and index checks make collisions a
    // performance miss only.
    pub(super) field_inline_cache: FieldInlineCache,
    pub(super) method_ref_inline_cache: MethodRefInlineCache,
    pub(super) fixed_method_inline_cache: FixedMethodInlineCache,
    pub(super) static_fields: StaticFields,
    pub(super) initialized: InitializedClasses,
    pub(super) initializing: HashMap<String, u64>,
    pub(super) failed_initialization: HashMap<String, String>,
    pub(super) initialization_waiters: HashMap<String, Vec<Handle>>,
    pub(super) objects: HashMap<String, Handle>,
}

impl ClassState {
    pub(super) fn with_program(program: &Program) -> Self {
        Self {
            constant_pool_caches: (0..program.constant_pool_count)
                .map(|_| ConstantPoolRuntimeCache::default())
                .collect(),
            static_fields: StaticFields::with_program(program),
            initialized: InitializedClasses::with_program(program),
            ..Self::default()
        }
    }

    pub(super) fn runtime_slots(&self, field: &Field) -> FieldRuntimeSlots {
        FieldRuntimeSlots {
            static_field: field
                .is_static
                .then(|| self.static_fields.slot(&field.key))
                .flatten(),
            declaring_class: self.initialized.slot(&field.declaring_class),
        }
    }

    pub(super) fn static_field_fast_access(
        &self,
        method: &Method,
        index: u16,
    ) -> Option<StaticFieldFastAccess> {
        self.field_inline_cache
            .get_static_access(method.constant_pool_id?, index)
    }

    pub(super) fn constant_pool_cache(&self, method: &Method) -> Option<&ConstantPoolRuntimeCache> {
        self.constant_pool_caches.get(method.constant_pool_id?)
    }

    pub(super) fn constant_pool_cache_mut(
        &mut self,
        method: &Method,
    ) -> Option<&mut ConstantPoolRuntimeCache> {
        self.constant_pool_caches.get_mut(method.constant_pool_id?)
    }

    pub(super) fn append_roots(&self, roots: &mut Vec<Handle>) {
        roots.extend(self.static_fields.values().filter_map(value_reference));
        roots.extend(self.objects.values().copied());
    }
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedMethodRef {
    pub(super) symbolic: MethodKey,
    pub(super) descriptor: MethodDescriptor,
    pub(super) is_static: bool,
}

#[derive(Clone, Debug)]
pub(super) struct VirtualMethodTarget {
    pub(super) receiver_class: String,
    pub(super) callee: Rc<Method>,
    pub(super) string_equals_ignore_case: bool,
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/class_state.rs"]
mod tests;
