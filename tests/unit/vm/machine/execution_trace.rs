use super::*;
use crate::machine::diagnostic_helpers::MAX_DIAGNOSTIC_BYTES;
use std::cell::Cell;

#[test]
fn trace_bounds_large_stack_formatting() {
    let mut trace = ExecutionTrace::default();
    let stack = vec![crate::machine::Value::Int(i32::MIN); 16_384];
    trace.record(format_args!("stack={stack:?}"));
    let lines = trace.into_lines();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].len() <= MAX_DIAGNOSTIC_BYTES);
    assert!(lines[0].starts_with("stack=[Int(-2147483648)"));
    assert!(lines[0].ends_with("..."));
}

#[test]
fn exhausted_trace_skips_formatting_and_adds_only_one_marker() {
    struct Counted<'a>(&'a Cell<usize>);
    impl fmt::Display for Counted<'_> {
        fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.set(self.0.get() + 1);
            output.write_str("must not be formatted")
        }
    }
    for byte_limit in [false, true] {
        let mut trace = ExecutionTrace::default();
        if byte_limit {
            let line = "界".repeat(MAX_DIAGNOSTIC_BYTES);
            for _ in 0..=MAX_TRACE_BYTES / MAX_DIAGNOSTIC_BYTES {
                trace.record(format_args!("{line}"));
            }
        } else {
            for _ in 0..=MAX_TRACE_LINES {
                trace.record(format_args!(""));
            }
        }
        assert!(trace.is_truncated());
        let calls = Cell::new(0);
        for _ in 0..100 {
            trace.record(format_args!("{}", Counted(&calls)));
        }
        assert_eq!(calls.get(), 0);
        let lines = trace.into_lines();
        assert!(lines.len() <= MAX_TRACE_LINES + 1);
        assert!(
            lines.iter().map(String::len).sum::<usize>() <= MAX_TRACE_BYTES + MAX_DIAGNOSTIC_BYTES
        );
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.starts_with("trace truncated"))
                .count(),
            1
        );
    }
}
