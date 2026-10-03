use super::{
    CallOutcome, Category, EmuError, Handle, HeapValue, Machine, Method, MethodKey, ThreadFailure,
    Value,
};
use std::fmt;

pub(super) const MAX_DIAGNOSTIC_BYTES: usize = 8 * 1024;

struct DiagnosticBuffer {
    text: String,
    maximum: usize,
}

impl fmt::Write for DiagnosticBuffer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let remaining = self.maximum - self.text.len();
        if text.len() <= remaining {
            self.text.push_str(text);
            return Ok(());
        }
        self.text
            .push_str(&text[..text.floor_char_boundary(remaining)]);
        Err(fmt::Error)
    }
}

pub(super) fn diagnostic_message(write: impl FnOnce(&mut dyn fmt::Write) -> fmt::Result) -> String {
    limited_message(MAX_DIAGNOSTIC_BYTES, write).0
}

fn limited_message(
    maximum: usize,
    write: impl FnOnce(&mut dyn fmt::Write) -> fmt::Result,
) -> (String, bool) {
    let mut output = DiagnosticBuffer {
        text: String::new(),
        maximum,
    };
    let truncated = write(&mut output).is_err();
    if truncated {
        let ellipsis = &"..."[..maximum.min(3)];
        let end = output.text.floor_char_boundary(maximum - ellipsis.len());
        output.text.truncate(end);
        output.text.push_str(ellipsis);
    }
    (output.text, truncated)
}

pub(super) fn diagnostic_stack<'a>(
    frames: impl Iterator<Item = (&'a MethodKey, usize)>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut remaining = MAX_DIAGNOSTIC_BYTES;
    for frame in frames {
        // Reserve a final marker if a complete line exhausts the remaining budget.
        if remaining < 6 {
            lines.push("...".to_owned());
            break;
        }
        let (line, truncated) = limited_message(remaining - 3, |output| {
            append_java_stack(output, std::iter::once(frame))
        });
        remaining -= line.len();
        lines.push(line);
        if truncated {
            break;
        }
    }
    lines
}

pub(super) fn append_java_stack<'a>(
    output: &mut dyn fmt::Write,
    frames: impl Iterator<Item = (&'a MethodKey, usize)>,
) -> fmt::Result {
    for (index, (key, pc)) in frames.enumerate() {
        if index != 0 {
            output.write_str(" <- ")?;
        }
        write!(
            output,
            "{}::{}{} pc={pc}",
            key.class, key.name, key.descriptor
        )?;
    }
    Ok(())
}

impl Machine<'_, '_> {
    pub(super) fn thread_exception(
        &mut self,
        class: &str,
        message: Option<&str>,
    ) -> Result<CallOutcome, EmuError> {
        if !self.program.classes.contains_key(class) {
            return Err(vm_error("vm-exception", message.unwrap_or(class)));
        }
        Ok(CallOutcome::Throw(self.allocate_exception(
            class,
            message,
            &[],
            &[],
        )?))
    }

    pub(super) fn error_as_call_outcome(
        &mut self,
        error: EmuError,
        roots: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let Some(class) = java_error_class(&error) else {
            return Err(error);
        };
        if !self.program.classes.contains_key(class) {
            return Err(error);
        }
        let handle = self.allocate_error_exception(class, &error, &[], roots)?;
        Ok(CallOutcome::Throw(handle))
    }

    pub(super) fn contextual_error(&self, error: EmuError) -> EmuError {
        let append_stack = !error.has_java_stack() && !self.execution.call_stack.is_empty();
        if !append_stack && error.message().len() <= MAX_DIAGNOSTIC_BYTES {
            return error;
        }
        let message = diagnostic_message(|output| {
            output.write_str(error.message())?;
            if append_stack {
                output.write_str("; java-stack: ")?;
                append_java_stack(
                    output,
                    self.execution.call_stack.iter().rev().map(|frame| {
                        (
                            self.program.active_stack_key(frame).as_ref(),
                            frame.bytecode_pc,
                        )
                    }),
                )?;
            }
            Ok(())
        });
        let error = error.with_message(message);
        if append_stack {
            error.with_java_stack()
        } else {
            error
        }
    }

    pub(super) fn record_thread_failure(&mut self, failure: ThreadFailure) {
        self.execution.thread_failure_count = self.execution.thread_failure_count.saturating_add(1);
        let capacity = self.limits.max_threads;
        if capacity == 0 {
            return;
        }
        if self.execution.thread_failures.len() == capacity {
            self.execution.thread_failures.remove(0);
        }
        self.execution.thread_failures.push(failure);
    }

    pub(super) fn report_uncaught_thread_exception(
        &mut self,
        thread: Handle,
        throwable: Handle,
    ) -> Result<(), EmuError> {
        let exception_class =
            diagnostics::bounded_text(&self.object_class(throwable)?, MAX_DIAGNOSTIC_BYTES);
        let exception_message = self.throwable_diagnostic_message(throwable);
        let stack_trace = diagnostic_stack(self.throwable_stack_frames(throwable));
        if self.execution.tracing {
            self.execution.trace.record(format_args!(
                "thread {} terminated by uncaught {exception_class}: {stack_trace:?}",
                thread.to_raw()
            ));
        }
        let managed_heap_limit = self.heap.managed_heap_limit_throwables.contains(&throwable);
        self.native_context.uncaught_thread_exception(
            thread.to_raw(),
            &exception_class,
            exception_message.as_deref(),
            &stack_trace,
            managed_heap_limit,
        );
        self.record_thread_failure(ThreadFailure {
            thread_id: thread.to_raw(),
            exception_class,
            exception_message,
            stack_trace,
            managed_heap_limit,
        });
        Ok(())
    }

    pub(super) fn throwable_stack_frames(
        &self,
        handle: Handle,
    ) -> impl Iterator<Item = (&MethodKey, usize)> {
        self.heap
            .throwable_traces
            .get(&handle)
            .into_iter()
            .flatten()
            .map(|frame| (frame.key.as_ref(), frame.bytecode_pc))
    }

    pub(super) fn throwable_diagnostic_message(&self, handle: Handle) -> Option<String> {
        let HeapValue::Reference(Some(message)) = self
            .heap
            .managed
            .field(
                handle,
                "java/lang/Throwable.detailMessage:Ljava/lang/String;",
            )
            .ok()?
        else {
            return None;
        };
        let units = self.heap.string_values.get(&message)?;
        let message = diagnostic_message(|output| {
            for character in char::decode_utf16(units.iter().copied()) {
                output.write_char(character.unwrap_or(char::REPLACEMENT_CHARACTER))?;
            }
            Ok(())
        });
        (!message.is_empty()).then_some(message)
    }

    pub(super) fn throwable_error(
        &self,
        handle: Handle,
        code: &'static str,
        operation: &str,
    ) -> Result<EmuError, EmuError> {
        let class = self.object_class(handle)?;
        let message = self.throwable_diagnostic_message(handle);
        let mut frames = self.throwable_stack_frames(handle).peekable();
        let has_stack = frames.peek().is_some();
        let description = diagnostic_message(|output| {
            write!(output, "{operation}: {class}")?;
            if let Some(message) = &message {
                write!(output, ": {message}")?;
            }
            if has_stack {
                output.write_str(if message.is_some() {
                    "; java-stack: "
                } else {
                    ": "
                })?;
                append_java_stack(output, frames)?;
            }
            Ok(())
        });
        let error = vm_error(code, description);
        Ok(if has_stack {
            error.with_java_stack()
        } else {
            error
        })
    }
}

pub(super) fn display_key(k: &MethodKey) -> String {
    k.display()
}
pub(super) fn opcode_name(opcode: u8) -> &'static str {
    match opcode {
        0xb2 => "getstatic",
        0xb3 => "putstatic",
        0xb4 => "getfield",
        0xb5 => "putfield",
        0xb6 => "invokevirtual",
        0xb7 => "invokespecial",
        0xb8 => "invokestatic",
        0xb9 => "invokeinterface",
        0xbb => "new",
        0xbc => "newarray",
        0xbd => "anewarray",
        0xc0 => "checkcast",
        0xc1 => "instanceof",
        0xc5 => "multianewarray",
        _ => "unknown-opcode",
    }
}
pub(super) fn annotate_resolution(
    unresolved: impl std::fmt::Display,
    method: &Method,
    pc: usize,
    opcode: u8,
    error: EmuError,
) -> EmuError {
    if matches!(
        error.code(),
        "class-not-found"
            | "method-not-found"
            | "field-not-found"
            | "abstract-method"
            | "incompatible-class-change"
    ) {
        let message = diagnostic_message(|output| {
            write!(
                output,
                "{}; unresolved={unresolved}; call-site={}::{}{} pc={pc} opcode={}",
                error.message(),
                method.key.class,
                method.key.name,
                method.key.descriptor,
                opcode_name(opcode)
            )
        });
        return error.with_message(message);
    }
    error
}
pub(super) fn type_error() -> EmuError {
    vm_error("type-mismatch", "operand has unexpected primitive type")
}
pub(super) fn vm_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Vm, code, message)
}

pub(super) fn java_error_class(error: &EmuError) -> Option<&'static str> {
    if error.category() == Category::M3g {
        return Some(match error.code() {
            "resource-limit" | "render-budget" | "memory-limit" => "java/lang/OutOfMemoryError",
            "singular-transform" => "java/lang/ArithmeticException",
            "stale-handle"
            | "unbound-guest-object"
            | "immutable-image"
            | "component-type"
            | "missing-positions"
            | "invalid-object-graph" => "java/lang/IllegalStateException",
            "vertex-bounds" | "index-out-of-bounds" => "java/lang/IndexOutOfBoundsException",
            _ => "java/lang/IllegalArgumentException",
        });
    }
    if error.category() == Category::Jar {
        return Some("java/io/IOException");
    }
    match error.code() {
        "number-format" => Some("java/lang/NumberFormatException"),
        "illegal-argument" | "illegal-argument-exception" | "calendar-overflow" | "store-name" => {
            Some("java/lang/IllegalArgumentException")
        }
        "illegal-state" | "illegal-state-exception" | "player-closed" => {
            Some("java/lang/IllegalStateException")
        }
        "class-not-found" => Some("java/lang/NoClassDefFoundError"),
        "class-for-name" => Some("java/lang/ClassNotFoundException"),
        "instantiation-exception" => Some("java/lang/InstantiationException"),
        "illegal-access-exception" => Some("java/lang/IllegalAccessException"),
        "class-circularity" => Some("java/lang/ClassCircularityError"),
        "exception-in-initializer" => Some("java/lang/ExceptionInInitializerError"),
        "method-not-found" => Some("java/lang/NoSuchMethodError"),
        "field-not-found" => Some("java/lang/NoSuchFieldError"),
        "incompatible-class-change" => Some("java/lang/IncompatibleClassChangeError"),
        "abstract-method" => Some("java/lang/AbstractMethodError"),
        "unsupported-native" => Some("java/lang/UnsatisfiedLinkError"),
        "null-pointer-exception" => Some("java/lang/NullPointerException"),
        "string-index" => Some("java/lang/StringIndexOutOfBoundsException"),
        "unsupported-encoding" => Some("java/io/UnsupportedEncodingException"),
        "array-index-out-of-bounds-exception" => Some("java/lang/ArrayIndexOutOfBoundsException"),
        "negative-array-size-exception" => Some("java/lang/NegativeArraySizeException"),
        "index-out-of-bounds-exception" => Some("java/lang/IndexOutOfBoundsException"),
        "buffer-overflow" => Some("java/nio/BufferOverflowException"),
        "array-store-exception" => Some("java/lang/ArrayStoreException"),
        "heap-limit"
        | "heap-metadata-limit"
        | super::MANAGED_HEAP_LIMIT_CODE
        | "out-of-memory-error"
        | "memory-limit"
        | "array-limit"
        | "string-limit"
        | "timer-limit"
        | "thread-limit" => Some("java/lang/OutOfMemoryError"),
        "stack-overflow" | "frame-limit" => Some("java/lang/StackOverflowError"),
        "illegal-monitor-state" => Some("java/lang/IllegalMonitorStateException"),
        "connection-not-found" => Some("javax/microedition/io/ConnectionNotFoundException"),
        "reader-io" | "media-io" | "network-io" | "network-dns" | "network-offline"
        | "network-limit" | "network-timeout" | "network-mock" | "network-url"
        | "network-header" | "network-method" | "network-redirect" | "file-url" | "file-io"
        | "file-limit" | "file-root" | "gcf-unavailable" => Some("java/io/IOException"),
        "media-unsupported-locator"
        | "media-unsupported-format"
        | "media-malformed"
        | "media-limit"
        | "media-work-limit"
        | "media-time-limit"
        | "event-queue-limit"
        | "audio-device"
        | "audio-resource-limit"
        | "mmapi-unavailable"
        | "player-limit"
        | "tone-sequence" => Some("javax/microedition/media/MediaException"),
        "security-exception" | "network-ssrf" | "file-traversal" | "file-symlink" => {
            Some("java/lang/SecurityException")
        }
        "store-not-found" => Some("javax/microedition/rms/RecordStoreNotFoundException"),
        "store-not-open" => Some("javax/microedition/rms/RecordStoreNotOpenException"),
        "invalid-record-id" => Some("javax/microedition/rms/InvalidRecordIDException"),
        "store-full" | "store-limit" | "record-limit" | "record-id-exhausted" => {
            Some("javax/microedition/rms/RecordStoreFullException")
        }
        "store-open" | "open-limit" | "version-exhausted" | "corrupt-store"
        | "unsupported-format" | "rms-unavailable" | "create-suite" | "open-store"
        | "read-store" | "write-temporary" | "sync-temporary" | "commit-store" | "sync-suite"
        | "delete-store" | "delete-temporary" | "list-stores" | "suite-size" => {
            Some("javax/microedition/rms/RecordStoreException")
        }
        _ => None,
    }
}

pub(super) fn class_for_name_target(name: &str) -> Option<String> {
    if name.is_empty() || name.starts_with('[') || name.contains('/') {
        return None;
    }
    let normalized = name.replace('.', "/");
    if normalized.split('/').any(str::is_empty) {
        return None;
    }
    Some(normalized)
}
