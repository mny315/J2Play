use super::super::{Field, FieldRuntimeSlots, Handle, HashMap, ValueKind, VirtualMethodTarget};
use super::*;
use std::rc::Rc;

fn fixture() -> (
    Method,
    Heap,
    ClassState,
    super::super::StringValues,
    Handle,
    Handle,
) {
    // A project-owned case-insensitive index search, with an unrelated class,
    // constant-pool layout and result branch. Only its miss path is fused.
    let code = [
        0x03, 0x3c, 0x1b, 0xb2, 0, 1, 0xbe, 0xa2, 0, 23, 0x2a, 0xb2, 0, 1, 0x1b, 0x32, 0xb6, 0, 7,
        0x99, 0, 5, 0x1b, 0xac, 0x84, 1, 1, 0xa7, 0xff, 0xe7, 0x02, 0xac,
    ];
    let mut method = crate::machine::tests::runtime_method(
        "Inventory",
        "find",
        "(Ljava/lang/String;)I",
        &code,
        3,
        2,
        vec![None],
        true,
    );
    method.constant_pool_id = Some(0);
    assert!(matches!(
        method.runtime_instructions[2].batch_opcode,
        Opcode::StringScan(1)
    ));
    let mut heap = Heap::new(1_000_000);
    let query = heap
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    let mut strings = super::super::StringValues::from([(query, "MaTcH".encode_utf16().collect())]);
    let array = heap
        .allocate_array(ArrayKind::Reference("java/lang/String".into()), 64)
        .unwrap();
    for index in 0..64 {
        let handle = heap
            .allocate_object("java/lang/String", HashMap::new())
            .unwrap();
        strings.insert(
            handle,
            if index == 62 {
                "match".encode_utf16().collect()
            } else {
                format!("item{index}").encode_utf16().collect()
            },
        );
        heap.array_set(array, index, HeapValue::Reference(Some(handle)))
            .unwrap();
    }
    let mut classes = ClassState::default();
    classes
        .constant_pool_caches
        .push(super::super::ConstantPoolRuntimeCache::default());
    classes.initialized.linked.push(true);
    classes
        .static_fields
        .linked
        .push(Some(Value::Reference(Some(array))));
    let field = Rc::new(Field {
        key: "Inventory.names:[Ljava/lang/String;".into(),
        declaring_class: "Inventory".into(),
        kind: ValueKind::Reference,
        is_static: true,
        instance_slot: None,
        field_token: heap::FieldToken::new(),
        initial: Value::Reference(None),
        constant_string: None,
    });
    classes.field_inline_cache.insert_resolved(
        0,
        1,
        field,
        FieldRuntimeSlots {
            static_field: Some(0),
            declaring_class: Some(0),
        },
    );
    let callee = crate::machine::tests::runtime_method(
        "java/lang/String",
        "equalsIgnoreCase",
        "(Ljava/lang/String;)Z",
        &[0x03, 0xac],
        1,
        2,
        vec![None],
        false,
    );
    classes
        .constant_pool_cache_mut(&method)
        .unwrap()
        .virtual_method_targets
        .insert(
            7,
            VirtualMethodTarget {
                receiver_class: "java/lang/String".into(),
                callee: Rc::new(callee),
                string_equals_ignore_case: true,
            },
        );
    (method, heap, classes, strings, query, array)
}

fn compare_batches(
    method: &Method,
    heap: &mut Heap,
    classes: &mut ClassState,
    strings: &super::super::StringValues,
    query: Option<Handle>,
) {
    let mut plain = method.clone();
    for instruction in std::sync::Arc::make_mut(&mut plain.runtime_instructions) {
        if let Opcode::StringScan(index) = instruction.batch_opcode {
            instruction.batch_opcode = Opcode::LoadInt(index);
        }
    }
    for budget in 0..=1024 {
        let mut fast_locals = [Some(Value::Reference(query)), Some(Value::Int(0))];
        let mut slow_locals = fast_locals;
        let mut fast_stack = Vec::new();
        let mut slow_stack = Vec::new();
        let fast = super::super::interpreter_batch::execute_with_strings(
            method,
            &mut fast_locals,
            &mut fast_stack,
            2,
            0,
            3,
            budget,
            heap,
            classes,
            Some(strings),
        );
        let slow = super::super::interpreter_batch::execute_with_strings(
            &plain,
            &mut slow_locals,
            &mut slow_stack,
            2,
            0,
            3,
            budget,
            heap,
            classes,
            Some(strings),
        );
        assert_eq!(
            (fast.pc, fast.last_pc, fast.instructions, fast.slots),
            (slow.pc, slow.last_pc, slow.instructions, slow.slots),
            "budget {budget}"
        );
        assert_eq!(fast_locals, slow_locals, "budget {budget}");
        assert_eq!(fast_stack, slow_stack, "budget {budget}");
    }
}

#[test]
fn fused_string_scan_keeps_every_instruction_boundary_and_live_array_values() {
    let (method, mut heap, mut classes, mut strings, query, array) = fixture();
    let mut locals = [Some(Value::Reference(Some(query))), Some(Value::Int(0))];
    assert_eq!(
        skip_misses(&method, 2, 1, &mut locals, &heap, &classes, &strings, 1024),
        Some((27, 62 * 12))
    );
    assert_eq!(locals[1], Some(Value::Int(62)));
    compare_batches(&method, &mut heap, &mut classes, &strings, Some(query));
    heap.array_set(array, 1, HeapValue::Reference(Some(query)))
        .unwrap();
    compare_batches(&method, &mut heap, &mut classes, &strings, Some(query));
    heap.array_set(array, 1, HeapValue::Reference(None))
        .unwrap();
    strings.insert(query, "absent".encode_utf16().collect());
    compare_batches(&method, &mut heap, &mut classes, &strings, Some(query));
    compare_batches(&method, &mut heap, &mut classes, &strings, None);
    strings.insert(query, vec![65; 257]);
    compare_batches(&method, &mut heap, &mut classes, &strings, Some(query));
    classes.initialized.linked[0] = false;
    compare_batches(&method, &mut heap, &mut classes, &strings, Some(query));
    classes.initialized.linked[0] = true;
    classes.static_fields.linked[0] = Some(Value::Reference(None));
    compare_batches(&method, &mut heap, &mut classes, &strings, Some(query));
}

#[test]
fn string_scan_linker_requires_the_same_array_and_exact_miss_backedge() {
    let (method, ..) = fixture();
    for (instruction, byte, value) in [(7, 1, 2), (14, 1, 2)] {
        let mut code = method.runtime_instructions.as_ref().clone();
        code[2].batch_opcode = Opcode::LoadInt(1);
        code[instruction].operands[byte] = value;
        link(&mut code);
        assert!(matches!(code[2].batch_opcode, Opcode::LoadInt(1)));
    }
}
