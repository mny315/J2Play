use super::*;

#[test]
fn arithmetic_and_locals() {
    assert_eq!(
        run(&[0x10, 6, 0x3b, 0x1a, 0x07, 0x68, 0xac], 2, 1).unwrap(),
        Some(Value::Int(24))
    )
}

#[test]
fn loop_and_branch() {
    let code = [
        0x03, 0x3b, 0x04, 0x3c, 0x1b, 0x10, 6, 0xa2, 0, 13, 0x1a, 0x1b, 0x60, 0x3b, 0x84, 1, 1,
        0xa7, 0xff, 0xf3, 0x1a, 0xac,
    ];
    assert_eq!(run(&code, 2, 2).unwrap(), Some(Value::Int(15)))
}

#[test]
fn legacy_jsr_and_ret_execute_bounded_subroutine() {
    let code = [0xa8, 0x00, 0x06, 0x10, 7, 0xac, 0x4b, 0xa9, 0x00];
    assert_eq!(run(&code, 1, 1).unwrap(), Some(Value::Int(7)));
}

#[test]
fn integer_division_by_zero_is_vm_error() {
    assert_eq!(
        run(&[0x04, 0x03, 0x6c, 0xac], 2, 0).unwrap_err().code(),
        "arithmetic-exception"
    )
}

#[test]
fn instruction_limit_is_enforced() {
    let m = method(&[0xa7, 0, 0], 0, 0);
    let mut p = Program::new();
    p.methods.insert(m.key.clone(), m);
    let e = p
        .execute(
            "T",
            "main",
            "()I",
            Limits {
                max_instructions: 5,
                ..Limits::default()
            },
            false,
        )
        .unwrap_err();
    assert_eq!(e.code(), "instruction-limit")
}

#[test]
fn execution_memory_limit_is_enforced() {
    let m = method(&[0x03, 0xac], 1, 1);
    let mut p = Program::new();
    p.methods.insert(m.key.clone(), m);
    let error = p
        .execute(
            "T",
            "main",
            "()I",
            Limits {
                max_runtime_bytes: 1,
                ..Limits::default()
            },
            false,
        )
        .unwrap_err();
    assert_eq!(error.code(), "memory-limit");
}

#[test]
fn trace_is_optional_and_deterministic() {
    let m = method(&[0x05, 0xac], 1, 0);
    let mut p = Program::new();
    p.methods.insert(m.key.clone(), m);
    let e = p
        .execute("T", "main", "()I", Limits::default(), true)
        .unwrap();
    let replay = p
        .execute("T", "main", "()I", Limits::default(), true)
        .unwrap();
    assert_eq!(e.trace, replay.trace);
    assert_eq!(e.trace.len(), 4);
    assert!(e.trace[0].contains("iconst_2"));
    assert!(e.trace[1].contains("ireturn"));
    assert_eq!(e.trace[2], "method-profile:");
    assert_eq!(e.trace[3], "method-profile          2 T::main()I");
    let untraced = p
        .execute("T", "main", "()I", Limits::default(), false)
        .unwrap();
    assert_eq!(untraced.value, e.value);
    assert_eq!(untraced.instructions, e.instructions);
    assert!(untraced.trace.is_empty());
}
