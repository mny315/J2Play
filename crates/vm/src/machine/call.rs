use super::{
    DIRECT_HOST_FRAME_DEPTH, EmuError, HOST_STACK_RED_ZONE, HOST_STACK_SEGMENT, Handle, Machine,
    Method, SuspendedCall, Value, ValueKind, display_key, java_error_class, reference_argument,
    slot_count, suspended_monitor_entry, type_error, vm_error,
};

pub(super) enum CallOutcome {
    Return(Option<Value>),
    Throw(Handle),
    Suspend(Box<SuspendedCall>),
}

impl Machine<'_, '_> {
    pub(super) fn call_int_virtual(
        &mut self,
        receiver: Handle,
        name: &str,
        descriptor: &str,
        depth: usize,
    ) -> Result<Result<i32, CallOutcome>, EmuError> {
        let class = self.object_class(receiver)?;
        let key = self.resolve_virtual(&class, name, descriptor)?;
        let method = self
            .program
            .methods
            .get(&key)
            .ok_or_else(|| vm_error("method-not-found", display_key(&key)))?;
        match self.call(method, [Value::Reference(Some(receiver))], depth + 1)? {
            CallOutcome::Return(Some(Value::Int(value))) => Ok(Ok(value)),
            outcome @ (CallOutcome::Throw(_) | CallOutcome::Suspend(_)) => Ok(Err(outcome)),
            _ => Err(type_error()),
        }
    }

    pub(super) fn invoke_from_stack(
        &mut self,
        callee: &Method,
        argument_start: usize,
        depth: usize,
        locals: &[Option<Value>],
        stack: &mut Vec<Value>,
        operand_stack_slots: &mut usize,
    ) -> Result<CallOutcome, EmuError> {
        let removed_slots = slot_count(stack.get(argument_start..).ok_or_else(type_error)?);
        let outcome = {
            let (caller_stack, args) = stack.split_at(argument_start);
            if let Some(outcome) = self.try_readonly_leaf(callee, args, depth + 1) {
                Ok(outcome)
            } else {
                self.publish_frame_roots(depth, locals, caller_stack, args);
                self.call_validated(callee, args, depth + 1)
            }
        };
        self.heap.frame_roots.remove(depth);
        stack.truncate(argument_start);
        self.execution.stack_slots = self.execution.stack_slots.saturating_sub(removed_slots);
        *operand_stack_slots = operand_stack_slots.saturating_sub(removed_slots);
        outcome
    }

    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn call<A: AsRef<[Value]>>(
        &mut self,
        method: &Method,
        args: A,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let args = args.as_ref();
        validate_call_arguments(method, args)?;
        self.call_validated(method, args, depth)
    }

    pub(super) fn call_validated(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let top_level_display_turn = depth == 1
            && !self.scheduler.dispatching_display
            && method.is_static
            && method.key.class == "javax/microedition/lcdui/Display"
            && method.key.name == "__hostIdle"
            && method.key.descriptor == "()V";
        if top_level_display_turn {
            // The lifecycle driver invokes this private implementation hook as
            // an ordinary static call. Mark the complete callback as a display
            // turn so paint() cannot be parked with Canvas.painting still set.
            self.scheduler.dispatching_display = true;
        }
        let canvas_paint_callback = self.scheduler.dispatching_display
            && method.key.name == "paint"
            && method.key.descriptor == "(Ljavax/microedition/lcdui/Graphics;)V"
            && self
                .program
                .is_assignable_to(&method.key.class, "javax/microedition/lcdui/Canvas");
        let previous_canvas_paint = self.scheduler.dispatching_canvas_paint;
        self.scheduler.dispatching_canvas_paint |= canvas_paint_callback;
        let outcome = self.call_with_synchronized_monitor(method, args, depth, None);
        self.scheduler.dispatching_canvas_paint = previous_canvas_paint;
        if top_level_display_turn {
            self.scheduler.dispatching_display = false;
            if matches!(outcome, Ok(CallOutcome::Return(_))) {
                // Returning to the driver is already the requested host-poll
                // boundary; do not carry it into the next unrelated callback.
                self.scheduler.driver_host_poll_yield = false;
            }
        }
        outcome
    }

    pub(super) fn call_with_synchronized_monitor(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
        pending_monitor: Option<Handle>,
    ) -> Result<CallOutcome, EmuError> {
        // Check before entering the large interpreter frame. Performing this
        // only inside `call_inner` lets the host overflow while constructing
        // the frame that was supposed to report the guest StackOverflowError.
        if self.execution.call_stack.len() >= self.limits.max_frames {
            return Err(vm_error(
                "stack-overflow",
                format!("frame limit {} exceeded", self.limits.max_frames),
            ));
        }
        let record_flight = self.native_context.vm_flight_recorder_enabled();
        if record_flight {
            self.native_context.record_vm_flight(format!(
                "instruction={} thread={} depth={depth} call-enter {} args={args:?}",
                self.execution.instructions,
                self.scheduler.current_thread,
                display_key(&method.key),
            ));
        }
        let caller_frame_slots = self.execution.frame_slots;
        let caller_stack_slots = self.execution.stack_slots;
        let synchronized_monitor = if let Some(monitor) = pending_monitor {
            if !method.is_synchronized {
                return Err(vm_error(
                    "invalid-monitor-continuation",
                    "monitor entry requires a synchronized method",
                ));
            }
            Some(monitor)
        } else if method.is_synchronized {
            let monitor = if method.is_static {
                self.intern_java_class(&method.key.class)?
            } else {
                reference_argument(args, 0)?
            };
            Some(monitor)
        } else {
            None
        };
        if let Some(monitor) = synchronized_monitor
            && !self.enter_monitor(monitor, depth)?
        {
            return Ok(suspended_monitor_entry(method, monitor, args.to_vec()));
        }
        self.execution.call_stack.push(method.active_stack_frame(0));
        // Deep recursive calls grow the host stack only near its red zone.
        // The Java frame limit remains independent of host stack capacity.
        let inner = if self.execution.call_stack.len() >= DIRECT_HOST_FRAME_DEPTH {
            stacker::maybe_grow(HOST_STACK_RED_ZONE, HOST_STACK_SEGMENT, || {
                self.call_inner(method, args, depth, synchronized_monitor, None)
            })
        } else {
            self.call_inner(method, args, depth, synchronized_monitor, None)
        };
        let suspended = matches!(inner, Ok(CallOutcome::Suspend(_)));
        let monitor_exit = if suspended {
            Ok(())
        } else {
            synchronized_monitor.map_or(Ok(()), |monitor| self.exit_monitor(monitor))
        };
        self.execution.frame_slots = caller_frame_slots;
        self.execution.stack_slots = caller_stack_slots;
        self.heap.frame_roots.remove(depth);
        let inner = match monitor_exit {
            Ok(()) => inner,
            Err(error) => Err(error),
        };
        let outcome = match inner {
            Err(error) => self.call_error_outcome(method, error, args),
            outcome => outcome,
        };
        if record_flight {
            let result = match &outcome {
                Ok(CallOutcome::Return(value)) => format!("return {value:?}"),
                Ok(CallOutcome::Throw(handle)) => format!("throw {handle:?}"),
                Ok(CallOutcome::Suspend(_)) => "suspend".to_owned(),
                Err(error) => format!(
                    "error {}[{}]: {}",
                    error.category().as_str(),
                    error.code(),
                    error.message()
                ),
            };
            self.native_context.record_vm_flight(format!(
                "instruction={} thread={} depth={depth} call-exit {} {result}",
                self.execution.instructions,
                self.scheduler.current_thread,
                display_key(&method.key),
            ));
        }
        self.execution.call_stack.pop();
        outcome
    }

    pub(super) fn call_error_outcome(
        &mut self,
        method: &Method,
        error: EmuError,
        roots: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        if let Some(class) = java_error_class(&error)
            && self.program.classes.contains_key(class)
        {
            let pc = self
                .execution
                .call_stack
                .last()
                .map_or(0, |frame| frame.bytecode_pc);
            match self.allocate_error_exception(class, &error, &[], roots) {
                Ok(handle) => {
                    self.record_exception_frame(handle, method, pc, &[], roots)
                        .map_err(|_| error)?;
                    Ok(CallOutcome::Throw(handle))
                }
                Err(_) => Err(error),
            }
        } else {
            Err(self.contextual_error(error))
        }
    }
}

pub(super) fn validate_call_arguments(method: &Method, args: &[Value]) -> Result<(), EmuError> {
    let receiver_slots = usize::from(!method.is_static);
    let expected_len = method
        .descriptor
        .parameters
        .len()
        .saturating_add(receiver_slots);
    let receiver_matches = method.is_static
        || args
            .first()
            .is_some_and(|value| value.kind() == ValueKind::Reference);
    let parameters_match = args.get(receiver_slots..).is_some_and(|values| {
        values
            .iter()
            .zip(&method.descriptor.parameters)
            .all(|(value, expected)| value.kind() == *expected)
    });
    if args.len() == expected_len && receiver_matches && parameters_match {
        Ok(())
    } else {
        Err(vm_error(
            "type-mismatch",
            format!("arguments do not match {}", display_key(&method.key)),
        ))
    }
}
