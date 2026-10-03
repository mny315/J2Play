//! Java exception allocation, handler entry and bounded diagnostics.

use super::diagnostic_helpers::{append_java_stack, diagnostic_message};
use super::{
    Allocation, Arc, CaughtExceptionRecord, EmuError, Handle, HeapValue, JavaStackFrame,
    MANAGED_HEAP_LIMIT_CODE, MANAGED_HEAP_LIMIT_MESSAGE, MANAGED_HEAP_NOTICE_CATCHES,
    MANAGED_HEAP_NOTICE_WINDOW_INSTRUCTIONS, Machine, ManagedHeapLimitDecision,
    ManagedHeapLimitNotice, Method, Value, display_key, heap_error, resolve_class, type_error,
    vm_error,
};

impl Machine<'_, '_> {
    pub(super) fn allocate_exception(
        &mut self,
        class: &str,
        message: Option<&str>,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        let mut fields = self.initial_instance_fields(class)?;
        if let Some(message) = message {
            let string = self.allocate_dynamic_string(message, locals, stack)?;
            if let Some((_, _, field)) = fields.iter_mut().find(|(key, _, _)| {
                key.as_ref() == "java/lang/Throwable.detailMessage:Ljava/lang/String;"
            }) {
                *field = HeapValue::Reference(Some(string));
            }
        }
        self.allocate_linked_object(class, fields, locals, stack)
    }

    pub(super) fn allocate_error_exception(
        &mut self,
        class: &str,
        error: &EmuError,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        let handle = self.allocate_exception(class, Some(error.message()), locals, stack)?;
        if error.code() == MANAGED_HEAP_LIMIT_CODE {
            self.heap.managed_heap_limit_throwables.insert(handle);
        }
        Ok(handle)
    }

    /// Allocates an implicit bytecode exception, retaining VM diagnostics when
    /// a small test program has no definition for the Java exception class.
    pub(super) fn allocate_bytecode_exception(
        &mut self,
        class: &str,
        message: Option<&str>,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<Handle, EmuError> {
        if !self.program.classes.contains_key(class) {
            let code = match class {
                "java/lang/NullPointerException" => "null-pointer-exception",
                "java/lang/ArrayIndexOutOfBoundsException" => "array-index-out-of-bounds-exception",
                "java/lang/NegativeArraySizeException" => "negative-array-size-exception",
                "java/lang/ClassCastException" => "class-cast-exception",
                "java/lang/ArrayStoreException" => "array-store-exception",
                _ => "vm-exception",
            };
            return Err(vm_error(code, message.unwrap_or(class)));
        }
        self.allocate_exception(class, message, locals, stack)
    }

    /// Enters a matching handler with only the throwable on its operand stack.
    /// All implicit, explicit, called and resumed throws share this accounting.
    pub(super) fn catch_exception(
        &mut self,
        method: &Method,
        pc: usize,
        handle: Handle,
        locals: &[Option<Value>],
        stack: &mut Vec<Value>,
        operand_stack_slots: &mut usize,
    ) -> Result<Option<usize>, EmuError> {
        self.record_exception_frame(handle, method, pc, locals, stack)?;
        let Some(handler) = self.find_handler(method, pc, handle)? else {
            return Ok(None);
        };
        let caller_slots = self
            .execution
            .stack_slots
            .saturating_sub(*operand_stack_slots);
        if method.max_stack == 0 || caller_slots >= self.limits.max_stack_slots {
            return Err(vm_error(
                "operand-stack-overflow",
                format!("{} pc={pc}", display_key(&method.key)),
            ));
        }
        stack.clear();
        stack.push(Value::Reference(Some(handle)));
        self.execution.stack_slots = caller_slots + 1;
        *operand_stack_slots = 1;
        Ok(Some(handler))
    }

    pub(super) fn record_exception_frame(
        &mut self,
        handle: Handle,
        method: &Method,
        pc: usize,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<(), EmuError> {
        if self.heap.throwable_traces.contains_key(&handle) {
            return Ok(());
        }
        self.heap.validate_throwable(self.program, handle)?;
        // VM-created exceptions bypass the Java constructor. Capture once at
        // their origin, while all callers are still active; rethrows keep it.
        let mut frames = self
            .execution
            .call_stack
            .iter()
            .rev()
            .map(|frame| JavaStackFrame {
                key: Arc::clone(self.program.active_stack_key(frame)),
                bytecode_pc: frame.bytecode_pc,
            })
            .collect::<Vec<_>>();
        if frames.is_empty() {
            frames.push(JavaStackFrame {
                key: Arc::clone(&method.stack_key),
                bytecode_pc: pc,
            });
        }
        self.account_external_bytes(
            handle,
            std::mem::size_of_val(frames.as_slice()),
            locals,
            stack,
        )?;
        self.heap.throwable_traces.insert(handle, frames);
        Ok(())
    }

    pub(super) fn find_handler(
        &mut self,
        method: &Method,
        pc: usize,
        thrown: Handle,
    ) -> Result<Option<usize>, EmuError> {
        let Allocation::Object { class: actual, .. } =
            self.heap.managed.get(thrown).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        for handler in method.exception_table.iter() {
            if pc >= usize::from(handler.start_pc)
                && pc < usize::from(handler.end_pc)
                && (handler.catch_type == 0
                    || self.is_instance(
                        actual,
                        resolve_class(&method.constants, handler.catch_type)?,
                    ))
            {
                let signature = (
                    Arc::clone(actual),
                    JavaStackFrame {
                        key: Arc::clone(&method.stack_key),
                        bytecode_pc: pc,
                    },
                );
                let record_trace = self.execution.profiling && !self.execution.trace.is_truncated();
                if !record_trace
                    && self.execution.caught_exceptions.len() >= 256
                    && !self.execution.caught_exceptions.contains_key(&signature)
                {
                    return Ok(Some(usize::from(handler.handler_pc)));
                }
                let caught_managed_heap_limit =
                    self.heap.managed_heap_limit_throwables.contains(&thrown);
                let diagnostic = diagnostic_message(|output| {
                    write!(output, "caught-exception {actual}")?;
                    if let Some(message) = self.throwable_diagnostic_message(thrown) {
                        write!(output, " message={message:?}")?;
                    }
                    write!(
                        output,
                        " at {}::{}{} pc={pc}: ",
                        method.key.class, method.key.name, method.key.descriptor
                    )?;
                    append_java_stack(output, self.throwable_stack_frames(thrown))
                });
                let managed_heap_burst_count =
                    if let Some(record) = self.execution.caught_exceptions.get_mut(&signature) {
                        let within_burst = self
                            .execution
                            .instructions
                            .saturating_sub(record.last_instruction)
                            <= MANAGED_HEAP_NOTICE_WINDOW_INSTRUCTIONS;
                        record.managed_heap_burst_count = if caught_managed_heap_limit {
                            if within_burst && record.managed_heap_burst_count != 0 {
                                record.managed_heap_burst_count.saturating_add(1)
                            } else {
                                1
                            }
                        } else {
                            0
                        };
                        record.count = record.count.saturating_add(1);
                        record.last_instruction = self.execution.instructions;
                        record.last_description.clone_from(&diagnostic);
                        record.managed_heap_burst_count
                    } else if self.execution.caught_exceptions.len() < 256 {
                        let managed_heap_burst_count = u64::from(caught_managed_heap_limit);
                        if std::env::var_os("J2PLAY_TRACE_EXCEPTIONS").is_some() {
                            eprintln!("j2play: vm[exception]: {diagnostic}");
                        }
                        self.execution.caught_exceptions.insert(
                            signature,
                            CaughtExceptionRecord {
                                count: 1,
                                managed_heap_burst_count,
                                first_instruction: self.execution.instructions,
                                last_instruction: self.execution.instructions,
                                first_description: diagnostic.clone(),
                                last_description: diagnostic.clone(),
                            },
                        );
                        managed_heap_burst_count
                    } else {
                        0
                    };
                if record_trace {
                    self.execution.trace.record(format_args!("{diagnostic}"));
                }
                if managed_heap_burst_count == MANAGED_HEAP_NOTICE_CATCHES {
                    let notice = ManagedHeapLimitNotice {
                        caught_count: managed_heap_burst_count,
                        heap_bytes: self.heap.managed.bytes(),
                        heap_limit_bytes: self.limits.max_heap_bytes,
                    };
                    if self.native_context.repeated_managed_heap_limit(notice)
                        == ManagedHeapLimitDecision::RequestProfileChange
                    {
                        return Err(vm_error(
                            "managed-heap-profile-change-requested",
                            format!(
                                "{MANAGED_HEAP_LIMIT_MESSAGE} repeatedly; frontend requested a device profile change after {managed_heap_burst_count} caught failures"
                            ),
                        ));
                    }
                }
                return Ok(Some(usize::from(handler.handler_pc)));
            }
        }
        Ok(None)
    }

    pub(super) fn caught_exception_diagnostics(&self) -> Vec<String> {
        let mut diagnostics = self
            .execution
            .caught_exceptions
            .values()
            .map(|record| {
                let descriptions = if record.first_description == record.last_description {
                    record.first_description.clone()
                } else {
                    format!(
                        "first=[{}] last=[{}]",
                        record.first_description, record.last_description
                    )
                };
                (
                    record.first_instruction,
                    format!(
                        "count={} first_instruction={} last_instruction={} {descriptions}",
                        record.count, record.first_instruction, record.last_instruction,
                    ),
                )
            })
            .collect::<Vec<_>>();
        diagnostics.sort_unstable_by_key(|(first_instruction, _)| *first_instruction);
        diagnostics
            .into_iter()
            .map(|(_, description)| description)
            .collect()
    }
}
