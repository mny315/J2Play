//! Bounded diagnostic trace storage and the final method profile.

use super::Machine;
use super::diagnostic_helpers::diagnostic_message;
use std::fmt;

const MAX_TRACE_BYTES: usize = 4 * 1_024 * 1_024;
const MAX_TRACE_LINES: usize = 100_000;

#[derive(Debug, Default)]
pub(super) struct ExecutionTrace {
    lines: Vec<String>,
    bytes: usize,
    truncated: bool,
}

impl ExecutionTrace {
    pub(super) fn is_truncated(&self) -> bool {
        self.truncated
    }

    pub(super) fn record(&mut self, message: fmt::Arguments<'_>) {
        if self.truncated {
            return;
        }
        if self.lines.len() == MAX_TRACE_LINES || self.bytes == MAX_TRACE_BYTES {
            self.truncated = true;
            return;
        }
        // Formatting itself stops at the diagnostic limit, including Debug
        // output for a large operand stack. No unbounded temporary is built.
        let line = diagnostic_message(|output| output.write_fmt(message));
        if line.len() > MAX_TRACE_BYTES - self.bytes {
            self.truncated = true;
            return;
        }
        self.bytes += line.len();
        self.lines.push(line);
    }

    fn into_lines(mut self) -> Vec<String> {
        if self.truncated {
            self.lines
                .push("trace truncated: diagnostic limit reached".into());
        }
        self.lines
    }
}

impl Machine<'_, '_> {
    pub(super) fn finish_trace(&mut self) -> Vec<String> {
        let mut lines = std::mem::take(&mut self.execution.trace).into_lines();
        if !self.execution.profiling {
            return lines;
        }
        let mut methods = self
            .execution
            .method_instructions
            .drain()
            .collect::<Vec<_>>();
        methods.sort_unstable_by(|left, right| {
            right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0))
        });
        // Keep the summary even after instruction tracing fills its budget.
        // These final 41 lines each retain the ordinary diagnostic byte bound.
        lines.push("method-profile:".into());
        lines.extend(methods.into_iter().take(40).map(|(method, instructions)| {
            diagnostic_message(|output| {
                write!(
                    output,
                    "method-profile {instructions:>10} {}::{}{}",
                    method.class, method.name, method.descriptor
                )
            })
        }));
        lines
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/execution_trace.rs"]
mod tests;
