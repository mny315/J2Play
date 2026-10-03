use super::*;

#[test]
fn local_integer_pairs_preserve_values_slots_and_instruction_boundaries() {
    for (opcode, left, right, expected) in [
        (0x60, i32::MAX, 1, i32::MIN),
        (0x64, i32::MIN, 1, i32::MAX),
        (0x68, i32::MAX, 2, -2),
        (0x78, 3, 33, 6),
        (0x7a, -16, 34, -4),
        (0x7c, -16, 34, 1_073_741_820),
        (0x7e, 0x35, 0x0f, 5),
        (0x80, 0x30, 5, 0x35),
        (0x82, 0x35, 0x0f, 0x3a),
    ] {
        let method = runtime_method(
            "IntegerPairs",
            "apply",
            "(II)I",
            &[0x1a, 0x1b, opcode, 0xac],
            4,
            2,
            vec![None],
            true,
        );
        let mut heap = Heap::new(4096);
        let mut classes = ClassState::default();
        let mut locals = [Some(Value::Int(left)), Some(Value::Int(right))];
        for wide_prefix in [false, true] {
            let prefix = if wide_prefix {
                vec![Value::Long(7)]
            } else {
                vec![]
            };
            let mut stack = prefix.clone();
            stack.push(Value::Int(left));
            let slots = if wide_prefix { 3 } else { 1 };
            let result = crate::machine::interpreter_batch::execute(
                &method,
                &mut locals,
                &mut stack,
                1,
                slots,
                slots + 1,
                2,
                &mut heap,
                &mut classes,
            );
            assert_eq!(
                (result.pc, result.last_pc, result.instructions, result.slots),
                (3, 2, 2, slots)
            );
            assert_eq!(stack.last(), Some(&Value::Int(expected)));
            assert_eq!(&stack[..stack.len() - 1], prefix);
        }
        // A jump directly to the arithmetic opcode still executes it alone.
        let mut stack = vec![Value::Int(left), Value::Int(right)];
        let result = crate::machine::interpreter_batch::execute(
            &method,
            &mut locals,
            &mut stack,
            2,
            2,
            2,
            1,
            &mut heap,
            &mut classes,
        );
        assert_eq!((result.pc, result.last_pc, result.instructions), (3, 2, 1));
        assert_eq!(stack, [Value::Int(expected)]);

        // A poll deadline, an intermediate stack limit or malformed operands
        // must leave the pair untouched for the ordinary checked interpreter.
        for (budget, limit, operand, local) in [
            (0, 2, Value::Int(left), Some(Value::Int(right))),
            (1, 2, Value::Int(left), Some(Value::Int(right))),
            (2, 1, Value::Int(left), Some(Value::Int(right))),
            (2, 2, Value::Float(1.0), Some(Value::Int(right))),
            (2, 2, Value::Int(left), Some(Value::Reference(None))),
            (2, 2, Value::Int(left), None),
        ] {
            let mut stack = vec![operand];
            locals[1] = local;
            let result = crate::machine::interpreter_batch::execute(
                &method,
                &mut locals,
                &mut stack,
                1,
                1,
                limit,
                budget,
                &mut heap,
                &mut classes,
            );
            assert_eq!((result.pc, result.instructions, result.slots), (1, 0, 1));
            assert_eq!(stack, [operand]);
            assert_eq!(locals[1], local);
        }
    }
}
