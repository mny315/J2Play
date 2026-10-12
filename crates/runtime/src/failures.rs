use diagnostics::{Category, EmuError};
use std::fmt::Write as _;

#[must_use]
pub fn thread_failure_message(failures: &[vm::ThreadFailure], total: u64) -> Option<String> {
    let Some(failure) = failures
        .iter()
        .find(|failure| failure.is_managed_heap_limit())
        .or_else(|| failures.first())
    else {
        return (total != 0).then(|| format!("{total} worker failures; details were not retained"));
    };
    let mut message = format!(
        "thread {} terminated by uncaught {}",
        failure.thread_id, failure.exception_class
    );
    if let Some(detail) = &failure.exception_message {
        message.push_str(": ");
        message.push_str(detail);
    }
    if !failure.stack_trace.is_empty() {
        message.push_str(if failure.exception_message.is_some() {
            "; java-stack: "
        } else {
            ": "
        });
        message.push_str(&failure.stack_trace.join(" <- "));
    }
    if total > 1 {
        let _ = write!(message, "; {total} worker failures total");
    }
    Some(message)
}

/// Turns authoritative VM worker failure state into a frontend diagnostic.
#[must_use]
pub fn thread_failure_error(failures: &[vm::ThreadFailure], total: u64) -> Option<EmuError> {
    let message = thread_failure_message(failures, total)?;
    let code = if failures
        .iter()
        .any(vm::ThreadFailure::is_managed_heap_limit)
    {
        vm::MANAGED_HEAP_LIMIT_CODE
    } else {
        "uncaught-thread-exception"
    };
    Some(EmuError::new(Category::Vm, code, message))
}
