//! Bounded checked bytecodes which cannot call the host or allocate guest objects.
//! Invalid operands are left untouched for the ordinary interpreter to report.
use super::{
    ClassState, Constant, Handle, Heap, Method, Value, ValueKind, set_local, typed_array_access,
};

mod classification;

pub(super) use classification::{Opcode, is_readonly_call_tree, is_readonly_leaf};

type StringLookup<'a> = Option<(Handle, &'a [u16])>;

#[inline]
fn cached_string<'a>(
    strings: &'a super::StringValues,
    cache: &mut Option<[StringLookup<'a>; 16]>,
    handle: Handle,
) -> Option<&'a [u16]> {
    let cache = cache.get_or_insert_with(|| [None; 16]);
    let slot = ((handle.to_raw() >> 32) & 15) as usize;
    if let Some((cached, units)) = cache[slot]
        && cached == handle
    {
        return Some(units);
    }
    let units = strings.get(&handle)?.as_slice();
    cache[slot] = Some((handle, units));
    Some(units)
}

pub(super) struct BatchResult {
    pub pc: usize,
    pub last_pc: usize,
    pub instructions: u64,
    pub slots: usize,
}

impl BatchResult {
    // No Java method has a bytecode offset equal to usize::MAX. Reuse the
    // next-PC slot for terminal completion, keeping this hot result compact.
    pub(super) fn returned(&self) -> bool {
        self.instructions != 0 && self.pc == usize::MAX
    }
}

// Keep register allocation separate from the large exception/call dispatcher.
#[inline(never)]
pub(super) fn execute(
    method: &Method,
    locals: &mut [Option<Value>],
    stack: &mut Vec<Value>,
    pc: usize,
    slots: usize,
    slot_limit: usize,
    budget: u64,
    heap: &mut Heap,
    classes: &mut ClassState,
) -> BatchResult {
    execute_with_strings(
        method, locals, stack, pc, slots, slot_limit, budget, heap, classes, None,
    )
}

// Only the ordinary interpreter supplies Strings after checking the guest
// frame bound. Leaf memoization must keep observing its original read set.
#[inline(never)]
pub(super) fn execute_with_strings(
    method: &Method,
    locals: &mut [Option<Value>],
    stack: &mut Vec<Value>,
    pc: usize,
    slots: usize,
    slot_limit: usize,
    budget: u64,
    heap: &mut Heap,
    classes: &mut ClassState,
    strings: Option<&super::StringValues>,
) -> BatchResult {
    if slots == stack.len() {
        execute_inner::<true, false>(
            method,
            locals,
            stack,
            pc,
            slots,
            slot_limit,
            budget,
            heap,
            classes,
            &mut super::leaf_cache::Observation::default(),
            strings,
        )
    } else {
        execute_inner::<false, false>(
            method,
            locals,
            stack,
            pc,
            slots,
            slot_limit,
            budget,
            heap,
            classes,
            &mut super::leaf_cache::Observation::default(),
            strings,
        )
    }
}

#[inline(never)]
pub(super) fn execute_inner<const SINGLE_SLOT: bool, const OBSERVE: bool>(
    method: &Method,
    locals: &mut [Option<Value>],
    stack: &mut Vec<Value>,
    pc: usize,
    slots: usize,
    slot_limit: usize,
    budget: u64,
    heap: &mut Heap,
    classes: &mut ClassState,
    observation: &mut super::leaf_cache::Observation,
    strings: Option<&super::StringValues>,
) -> BatchResult {
    let empty = BatchResult {
        pc,
        last_pc: pc,
        instructions: 0,
        slots,
    };
    let Some(&initial) = method
        .instruction_index
        .get(pc)
        .filter(|&&index| index != u16::MAX)
    else {
        return empty;
    };
    // Own the Vec header while executing. Its allocation is retained, but its
    // length need not be published through a caller-owned reference per opcode.
    let caller_stack = stack;
    let mut stack = std::mem::take(caller_stack);
    let mut current_pc = pc;
    let mut last_pc = pc;
    let mut instructions = 0;
    let mut current_slots = slots;
    let mut index = usize::from(initial);
    let runtime_instructions = method.runtime_instructions.as_slice();
    // The String map is immutably borrowed for this batch. Borrowed payloads
    // cannot outlive the batch or survive constructors, GC, or host calls.
    let mut string_lookups = None;
    while instructions < budget {
        let Some(ins) = runtime_instructions.get(index) else {
            break;
        };
        let op = ins.opcode;
        let len = stack.len();
        if SINGLE_SLOT {
            current_slots = len;
        }
        let mut next_pc = ins.next_pc as usize;
        let mut next_index = index + 1;
        macro_rules! push {
            ($value:expr) => {{
                let value = $value;
                if SINGLE_SLOT && value.slots() != 1 {
                    break;
                }
                let next_slots = current_slots + value.slots();
                if next_slots > slot_limit {
                    break;
                }
                stack.push(value);
                current_slots = next_slots;
            }};
        }
        macro_rules! load_local {
            ($local:expr, $kind:expr) => {{
                let Some(value) = locals.get(usize::from($local)).copied().flatten() else {
                    break;
                };
                if value.kind() != $kind {
                    break;
                }
                push!(value);
            }};
        }
        macro_rules! store_local {
            ($local:expr, $kind:expr) => {{
                let Some(&value) = stack.last() else {
                    break;
                };
                // astore return addresses retain the ordinary checked path.
                if value.kind() != $kind {
                    break;
                }
                if set_local(locals, usize::from($local), value).is_err() {
                    break;
                }
                stack.pop();
                current_slots -= value.slots();
            }};
        }
        macro_rules! integer_binary {
            ($a:ident, $b:ident, $value:expr) => {{
                let [.., Value::Int($a), Value::Int($b)] = stack.as_slice() else {
                    break;
                };
                let ($a, $b) = (*$a, *$b);
                let value = $value;
                stack[len - 2] = Value::Int(value);
                stack.pop();
                current_slots -= 1;
            }};
        }
        macro_rules! branch {
            ($taken:expr, $removed:expr) => {{
                if $taken {
                    if ins.branch_index == u16::MAX {
                        break;
                    }
                    next_index = usize::from(ins.branch_index);
                    next_pc = current_pc.wrapping_add_signed(isize::from(i16::from_be_bytes([
                        ins.operands[0],
                        ins.operands[1],
                    ])));
                }
                stack.truncate(len - $removed);
                current_slots -= $removed;
            }};
        }
        match ins.batch_opcode {
            Opcode::Nop => {}
            Opcode::Null => push!(Value::Reference(None)),
            Opcode::IntConstant(value) => push!(Value::Int(i32::from(value))),
            Opcode::LongConstant(value) => push!(Value::Long(i64::from(value))),
            Opcode::FloatConstant(value) => push!(Value::Float(f32::from(value))),
            Opcode::DoubleConstant(value) => push!(Value::Double(f64::from(value))),
            Opcode::ShortConstant => push!(Value::Int(i32::from(i16::from_be_bytes([
                ins.operands[0],
                ins.operands[1]
            ])))),
            Opcode::Constant => {
                let constant = if op == 0x12 {
                    u16::from(ins.operands[0])
                } else {
                    u16::from_be_bytes([ins.operands[0], ins.operands[1]])
                };
                let value = match method
                    .constants
                    .get(usize::from(constant))
                    .and_then(Option::as_ref)
                {
                    Some(Constant::Integer(v)) if op != 0x14 => Value::Int(*v),
                    Some(Constant::Float(v)) if op != 0x14 => Value::Float(f32::from_bits(*v)),
                    Some(Constant::Long(v)) if op == 0x14 => Value::Long(*v),
                    Some(Constant::Double(v)) if op == 0x14 => Value::Double(f64::from_bits(*v)),
                    Some(Constant::String { string_index }) if op != 0x14 => {
                        let Some(handle) = classes
                            .constant_pool_cache(method)
                            .and_then(|cache| cache.string_literals.get(*string_index))
                        else {
                            break;
                        };
                        Value::Reference(Some(*handle))
                    }
                    _ => break,
                };
                push!(value);
            }
            Opcode::StringScan(local) => {
                if !OBSERVE
                    && current_slots.saturating_add(3) <= slot_limit
                    && let Some(strings) = strings
                    && let Some((backedge_pc, charged)) = super::string_array_scan::skip_misses(
                        method,
                        index,
                        local,
                        locals,
                        heap,
                        classes,
                        strings,
                        budget - instructions,
                    )
                {
                    instructions += charged;
                    // Every fused miss ends with the original backedge. Keep
                    // the loop head and stack intact for hits, limits and errors.
                    last_pc = backedge_pc;
                    continue;
                }
                load_local!(local, ValueKind::Int);
            }
            Opcode::LoadInt(local) => load_local!(local, ValueKind::Int),
            Opcode::LocalIntBinary(local) => {
                // Skip one dispatch and a temporary stack entry, but retain
                // the intermediate iload limit and both instruction charges.
                // At a poll boundary the ordinary interpreter executes iload.
                if budget - instructions < 2 || current_slots >= slot_limit {
                    break;
                }
                let Some(Some(Value::Int(right))) = locals.get(usize::from(local)) else {
                    break;
                };
                let Some(Value::Int(left)) = stack.last_mut() else {
                    break;
                };
                let Some(binary) = runtime_instructions.get(next_index) else {
                    break;
                };
                let value = match binary.opcode {
                    0x60 => left.wrapping_add(*right),
                    0x64 => left.wrapping_sub(*right),
                    0x68 => left.wrapping_mul(*right),
                    0x78 => left.wrapping_shl((*right & 31) as u32),
                    0x7a => left.wrapping_shr((*right & 31) as u32),
                    0x7c => ((*left as u32) >> (*right & 31)) as i32,
                    0x7e => *left & *right,
                    0x80 => *left | *right,
                    0x82 => *left ^ *right,
                    _ => break,
                };
                *left = value;
                current_pc = next_pc;
                next_pc = binary.next_pc as usize;
                next_index += 1;
                instructions += 1;
            }
            Opcode::LoadLong(local) => load_local!(local, ValueKind::Long),
            Opcode::LoadFloat(local) => load_local!(local, ValueKind::Float),
            Opcode::LoadDouble(local) => load_local!(local, ValueKind::Double),
            Opcode::LoadReference(local) => load_local!(local, ValueKind::Reference),
            Opcode::LoadArray => {
                if SINGLE_SLOT && matches!(op, 0x2f | 0x31) {
                    break;
                }
                let [.., Value::Reference(Some(array)), Value::Int(at)] = stack.as_slice() else {
                    break;
                };
                let Ok(access) = typed_array_access(op) else {
                    break;
                };
                let Ok(value) = heap.array_get_typed(*array, *at, access) else {
                    break;
                };

                if SINGLE_SLOT && value.slots() != 1 {
                    break;
                }
                let next_slots = current_slots - 2 + value.slots();
                if next_slots > slot_limit {
                    break;
                }
                if OBSERVE {
                    observation.record(super::leaf_cache::Read::Array(*array, *at, access, value));
                }
                stack[len - 2] = value;
                stack.pop();
                current_slots = next_slots;
            }
            Opcode::StoreInt(local) => store_local!(local, ValueKind::Int),
            Opcode::StoreLong(local) => store_local!(local, ValueKind::Long),
            Opcode::StoreFloat(local) => store_local!(local, ValueKind::Float),
            Opcode::StoreDouble(local) => store_local!(local, ValueKind::Double),
            Opcode::StoreReference(local) => store_local!(local, ValueKind::Reference),
            Opcode::StoreArray => {
                let [.., Value::Reference(Some(array)), Value::Int(at), value] = stack.as_slice()
                else {
                    break;
                };
                let Ok(access) = typed_array_access(op - 0x21) else {
                    break;
                };
                let slots = value.slots() + 2;
                if heap.array_set_typed(*array, *at, access, *value).is_err() {
                    break;
                }
                stack.truncate(len - 3);
                current_slots -= slots;
            }
            Opcode::Pop => {
                let Some(value) = stack.last() else {
                    break;
                };
                if value.slots() != 1 {
                    break;
                }
                stack.pop();
                current_slots -= 1;
            }
            Opcode::Duplicate => {
                let Some(&value) = stack.last() else {
                    break;
                };
                if value.slots() != 1 {
                    break;
                }
                push!(value);
            }
            Opcode::TwoSlots => {
                let Some(&top) = stack.last() else {
                    break;
                };
                let count = if top.slots() == 2 {
                    1
                } else if len >= 2 && stack[len - 2].slots() == 1 {
                    2
                } else {
                    break;
                };
                if op == 0x58 {
                    stack.truncate(len - count);
                    current_slots -= 2;
                } else {
                    if current_slots + 2 > slot_limit {
                        break;
                    }
                    if count == 2 {
                        stack.push(stack[len - 2]);
                    }
                    stack.push(top);
                    current_slots += 2;
                }
            }
            Opcode::Swap => {
                let [.., below, top] = stack.as_slice() else {
                    break;
                };
                let (below, top) = (*below, *top);
                if below.slots() != 1 || top.slots() != 1 {
                    break;
                }
                if op == 0x5a {
                    if current_slots + 1 > slot_limit {
                        break;
                    }
                    stack.push(top);
                    current_slots += 1;
                }
                stack[len - 2] = top;
                stack[len - 1] = below;
            }
            Opcode::IntAdd => integer_binary!(a, b, a.wrapping_add(b)),
            Opcode::IntSubtract => integer_binary!(a, b, a.wrapping_sub(b)),
            Opcode::IntMultiply => integer_binary!(a, b, a.wrapping_mul(b)),
            Opcode::IntDivide => integer_binary!(a, b, {
                if b == 0 {
                    break;
                }
                a.wrapping_div(b)
            }),
            Opcode::IntRemainder => integer_binary!(a, b, {
                if b == 0 {
                    break;
                }
                a.wrapping_rem(b)
            }),
            Opcode::IntShiftLeft => integer_binary!(a, b, a.wrapping_shl((b & 31) as u32)),
            Opcode::IntShiftRight => integer_binary!(a, b, a.wrapping_shr((b & 31) as u32)),
            Opcode::IntUnsignedShiftRight => integer_binary!(a, b, ((a as u32) >> (b & 31)) as i32),
            Opcode::IntAnd => integer_binary!(a, b, a & b),
            Opcode::IntOr => integer_binary!(a, b, a | b),
            Opcode::IntXor => integer_binary!(a, b, a ^ b),
            Opcode::LongBinary => {
                if SINGLE_SLOT {
                    break;
                }
                let [.., Value::Long(left), Value::Long(right)] = stack.as_slice() else {
                    break;
                };
                let value = match op {
                    0x61 => Value::Long(left.wrapping_add(*right)),
                    0x65 => Value::Long(left.wrapping_sub(*right)),
                    0x69 => Value::Long(left.wrapping_mul(*right)),
                    0x6d if *right != 0 => Value::Long(left.wrapping_div(*right)),
                    0x71 if *right != 0 => Value::Long(left.wrapping_rem(*right)),
                    0x7f => Value::Long(left & right),
                    0x81 => Value::Long(left | right),
                    0x83 => Value::Long(left ^ right),
                    0x94 => Value::Int(match left.cmp(right) {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    }),
                    _ => break,
                };
                current_slots -= 4 - value.slots();
                stack[len - 2] = value;
                stack.pop();
            }
            Opcode::FloatingBinary => {
                let [.., left, right] = stack.as_slice() else {
                    break;
                };
                if SINGLE_SLOT && (left.slots() != 1 || right.slots() != 1) {
                    break;
                }
                let value = if op >= 0x95 {
                    super::compare(op, *left, *right).map(Value::Int)
                } else {
                    super::binary(op, *left, *right)
                };
                let Ok(value) = value else { break };
                current_slots -= left.slots() + right.slots() - value.slots();
                stack[len - 2] = value;
                stack.pop();
            }
            Opcode::FloatingUnary => {
                let Some(&input) = stack.last() else {
                    break;
                };
                let value = if op <= 0x77 {
                    super::negate(op, input)
                } else {
                    super::convert(op, input)
                };
                let Ok(value) = value else { break };
                if SINGLE_SLOT && value.slots() != 1 {
                    break;
                }
                let next_slots = current_slots - input.slots() + value.slots();
                if next_slots > slot_limit {
                    break;
                }
                stack[len - 1] = value;
                current_slots = next_slots;
            }
            Opcode::LongShift => {
                if SINGLE_SLOT {
                    break;
                }
                let [.., Value::Long(left), Value::Int(right)] = stack.as_slice() else {
                    break;
                };
                let shift = (*right & 63) as u32;
                let value = match op {
                    0x79 => left.wrapping_shl(shift),
                    0x7b => left.wrapping_shr(shift),
                    _ => ((*left as u64) >> shift) as i64,
                };
                stack[len - 2] = Value::Long(value);
                stack.pop();
                current_slots -= 1;
            }
            Opcode::LongNegate => {
                if SINGLE_SLOT {
                    break;
                }
                let Some(Value::Long(value)) = stack.last_mut() else {
                    break;
                };
                *value = value.wrapping_neg();
            }
            Opcode::IntToLong => {
                if SINGLE_SLOT {
                    break;
                }
                let Some(&Value::Int(value)) = stack.last() else {
                    break;
                };
                if current_slots + 1 > slot_limit {
                    break;
                }
                stack[len - 1] = Value::Long(i64::from(value));
                current_slots += 1;
            }
            Opcode::LongToInt => {
                if SINGLE_SLOT {
                    break;
                }
                let Some(&Value::Long(value)) = stack.last() else {
                    break;
                };
                stack[len - 1] = Value::Int(value as i32);
                current_slots -= 1;
            }
            Opcode::IntUnary => {
                let Some(Value::Int(value)) = stack.last_mut() else {
                    break;
                };
                *value = match op {
                    0x74 => value.wrapping_neg(),
                    0x91 => i32::from(*value as i8),
                    0x92 => i32::from(*value as u16),
                    _ => i32::from(*value as i16),
                };
            }
            Opcode::Increment => {
                let Some(Some(Value::Int(value))) = locals.get_mut(usize::from(ins.operands[0]))
                else {
                    break;
                };
                *value = value.wrapping_add(i32::from(ins.operands[1] as i8));
            }
            Opcode::IfZero(condition) => {
                let Some(&Value::Int(value)) = stack.last() else {
                    break;
                };
                branch!(super::test_zero(condition, value), 1);
            }
            Opcode::IfIntPair(condition) => {
                let [.., Value::Int(left), Value::Int(right)] = stack.as_slice() else {
                    break;
                };
                branch!(super::test_pair(condition, *left, *right), 2);
            }
            Opcode::IfReferencePair(equal) => {
                let [.., Value::Reference(left), Value::Reference(right)] = stack.as_slice() else {
                    break;
                };
                branch!((left == right) == equal, 2);
            }
            Opcode::IfNull(is_null) => {
                let Some(Value::Reference(value)) = stack.last() else {
                    break;
                };
                branch!(value.is_none() == is_null, 1);
            }
            Opcode::Goto => branch!(true, 0),
            Opcode::StaticField => {
                let field_index = u16::from_be_bytes([ins.operands[0], ins.operands[1]]);
                let Some(access) = classes.static_field_fast_access(method, field_index) else {
                    break;
                };
                if !access
                    .slots
                    .declaring_class
                    .is_some_and(|slot| classes.initialized.contains_at(slot))
                {
                    break;
                }
                let Some(slot) = access.slots.static_field else {
                    break;
                };
                if op == 0xb2 {
                    let Some(value) = classes.static_fields.get_linked(slot).copied() else {
                        break;
                    };
                    if OBSERVE {
                        observation.record(super::leaf_cache::Read::Static(
                            slot,
                            access
                                .slots
                                .declaring_class
                                .expect("checked initialized class"),
                            value,
                        ));
                    }
                    push!(value);
                } else {
                    let Some(&value) = stack.last() else {
                        break;
                    };
                    if value.kind() != access.kind {
                        break;
                    }
                    let Some(target) = classes.static_fields.get_linked_mut(slot) else {
                        break;
                    };
                    *target = value;
                    stack.pop();
                    current_slots -= value.slots();
                }
            }
            Opcode::VirtualCall => {
                if OBSERVE {
                    break;
                }
                let Some(strings) = strings else { break };
                let constant = u16::from_be_bytes([ins.operands[0], ins.operands[1]]);
                let Some(cached) = classes
                    .constant_pool_cache(method)
                    .and_then(|cache| cache.virtual_method_targets.get(constant))
                    .filter(|cached| cached.string_equals_ignore_case)
                else {
                    break;
                };
                let [
                    ..,
                    Value::Reference(Some(receiver)),
                    Value::Reference(other),
                ] = stack.as_slice()
                else {
                    break;
                };
                let Ok(super::Allocation::Object { class, .. }) = heap.get(*receiver) else {
                    break;
                };
                if class.as_ref() != cached.receiver_class {
                    break;
                }
                let Some(left) = cached_string(strings, &mut string_lookups, *receiver) else {
                    break;
                };
                let Some(next_slots) = current_slots
                    .checked_sub(1)
                    .filter(|&slots| slots <= slot_limit)
                else {
                    break;
                };
                let equal = if let Some(other) = other {
                    let Some(right) = cached_string(strings, &mut string_lookups, *other) else {
                        break;
                    };
                    // Cap work per bytecode batch. Longer comparisons retain
                    // the ordinary intrinsic and its original call boundary.
                    if left.len().min(right.len()) > 256 {
                        break;
                    }
                    super::compatibility_strings::utf16_equals_ignore_case(left, right)
                } else {
                    false
                };
                stack.truncate(len - 2);
                stack.push(Value::Int(i32::from(equal)));
                current_slots = next_slots;
            }
            Opcode::InstanceField => {
                let field_index = u16::from_be_bytes([ins.operands[0], ins.operands[1]]);
                let Some(pool) = method.constant_pool_id else {
                    break;
                };
                let Some((field, _)) = classes.field_inline_cache.get_resolved(pool, field_index)
                else {
                    break;
                };
                if field.is_static {
                    break;
                }
                let Some(slot) = field.instance_slot else {
                    break;
                };
                if op == 0xb4 {
                    let Some(Value::Reference(Some(object))) = stack.last() else {
                        break;
                    };
                    let Ok(value) = heap.field_at(*object, slot, &field.field_token, &field.key)
                    else {
                        break;
                    };

                    if SINGLE_SLOT && value.slots() != 1 {
                        break;
                    }
                    let next_slots = current_slots - 1 + value.slots();
                    if next_slots > slot_limit {
                        break;
                    }
                    if OBSERVE {
                        observation.record(super::leaf_cache::Read::Field(
                            *object,
                            slot,
                            field.clone(),
                            value,
                        ));
                    }
                    stack[len - 1] = value;
                    current_slots = next_slots;
                } else {
                    let [.., Value::Reference(Some(object)), value] = stack.as_slice() else {
                        break;
                    };
                    if value.kind() != field.kind {
                        break;
                    }
                    let removed = 1 + value.slots();
                    if heap
                        .set_field_at(*object, slot, &field.field_token, &field.key, *value)
                        .is_err()
                    {
                        break;
                    }
                    stack.truncate(len - 2);
                    current_slots -= removed;
                }
            }
            Opcode::ArrayLength => {
                let Some(Value::Reference(Some(array))) = stack.last() else {
                    break;
                };
                let Ok(length) = heap.array_length(*array) else {
                    break;
                };
                if OBSERVE {
                    observation.record(super::leaf_cache::Read::Length(*array, length));
                }
                let Ok(length) = i32::try_from(length) else {
                    break;
                };
                stack[len - 1] = Value::Int(length);
            }
            Opcode::Return(opcode) => {
                if opcode == 0xb1 {
                    if method.descriptor.returns.is_some() {
                        break;
                    }
                } else {
                    let Ok(kind) = super::kind_for_return_opcode(opcode) else {
                        break;
                    };
                    if method.descriptor.returns != Some(kind)
                        || stack.last().is_none_or(|value| value.kind() != kind)
                    {
                        break;
                    }
                }
                // Keep the value rooted on the operand stack until the caller
                // accounts for this instruction and tears down the Java frame.
                last_pc = current_pc;
                current_pc = usize::MAX;
                instructions += 1;
                break;
            }
            Opcode::Fallback => break,
        }
        last_pc = current_pc;
        current_pc = next_pc;
        instructions += 1;
        index = next_index;
    }
    if SINGLE_SLOT {
        current_slots = stack.len();
    }
    *caller_stack = stack;
    BatchResult {
        pc: current_pc,
        last_pc,
        instructions,
        slots: current_slots,
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/interpreter_batch/mod.rs"]
mod string_lookup_tests;
