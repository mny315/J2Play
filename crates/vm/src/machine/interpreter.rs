use super::frame_storage::PooledFrameStorage;

mod stack;

impl Machine<'_, '_> {
    pub(super) fn call_inner(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
        synchronized_monitor: Option<Handle>,
        resume: Option<Box<SuspendedCall>>,
    ) -> Result<CallOutcome, EmuError> {
        if depth > self.limits.max_frames {
            return Err(vm_error(
                "stack-overflow",
                format!("frame limit {} exceeded", self.limits.max_frames),
            ));
        }
        if !method.is_native && method.code.is_empty() {
            return Err(vm_error("abstract-method", display_key(&method.key)));
        }
        if resume.is_none()
            && compatibility_intrinsic_candidate(method)
            && let Some(outcome) = self.invoke_compatibility_intrinsic(method, args, depth)?
        {
            return Ok(outcome);
        }
        if method.is_native {
            // Native methods can call guest code that collects while their
            // arguments or continuation state are only held by Rust frames.
            let roots = self.heap.frame_roots.at_depth(depth);
            roots.clear();
            roots.extend(args.iter().filter_map(super::value_reference));
            if let Some(resume) = &resume {
                resume.roots(roots);
            }
            return self.call_native_inner(method, args, depth, resume);
        }
        let projected_slots = self
            .execution
            .frame_slots
            .checked_add(method.max_locals)
            .ok_or_else(|| vm_error("memory-limit", "frame storage size overflow"))?;
        let projected_bytes = projected_slots
            .checked_mul(std::mem::size_of::<Option<Value>>())
            .and_then(|bytes| bytes.checked_add(self.program.runtime_bytes))
            .ok_or_else(|| vm_error("memory-limit", "execution memory size overflow"))?;
        if projected_bytes > self.limits.max_runtime_bytes {
            return Err(vm_error(
                "memory-limit",
                format!("execution exceeds {} bytes", self.limits.max_runtime_bytes),
            ));
        }
        if resume.is_none()
            && let Some(outcome) = self.try_compiled_integer(method, args)
        {
            return Ok(outcome);
        }
        // Both call boundaries restore the caller's slots and roots for every
        // outcome, including errors and repeated continuation suspension.
        self.execution.frame_slots = projected_slots;
        let frame_storage_pool = std::rc::Rc::clone(&self.execution.frame_storage_pool);
        let (mut frame_storage, mut pc, mut pending) = if let Some(resume) = resume {
            let resume = *resume;
            (
                PooledFrameStorage {
                    locals: resume.locals,
                    stack: resume.stack,
                    pool: frame_storage_pool,
                },
                resume.pc,
                resume.pending,
            )
        } else {
            let mut storage = PooledFrameStorage::take(
                frame_storage_pool,
                method.max_locals,
                method.max_stack.min(32),
            );
            let mut local = 0;
            for value in args.iter().copied() {
                set_local(&mut storage.locals, local, value)?;
                local += value.slots();
            }
            (storage, 0, None)
        };
        let (locals, stack) = (&mut frame_storage.locals, &mut frame_storage.stack);
        let mut operand_stack_slots = slot_count(stack);
        self.execution.stack_slots = self
            .execution
            .stack_slots
            .saturating_add(operand_stack_slots);
        let mut sequential_instruction = None;
        // Batch the same supported operations in every application method.
        let batch_enabled = !self.execution.tracing
            && !self.execution.profiling
            && u16::try_from(method.runtime_instructions.len()).is_ok()
            && !self.native_context.vm_flight_recorder_enabled();
        macro_rules! push {
            ($v:expr) => {{
                let v = $v;
                let slots = v.slots();
                self.execution.stack_slots += slots;
                operand_stack_slots += slots;
                if self.execution.stack_slots > self.limits.max_stack_slots
                    || operand_stack_slots > method.max_stack
                {
                    return Err(vm_error(
                        "operand-stack-overflow",
                        format!("{} pc={pc}", display_key(&method.key)),
                    ));
                };
                stack.push(v);
            }};
        }
        macro_rules! vm_throw {
            ($handle:expr) => {{
                let handle = $handle;
                if let Some(handler) = self.catch_exception(
                    method,
                    pc,
                    handle,
                    locals,
                    stack,
                    &mut operand_stack_slots,
                )? {
                    pc = handler;
                    sequential_instruction = None;
                    continue;
                }
                return Ok(CallOutcome::Throw(handle));
            }};
        }
        macro_rules! vm_raise {
            ($class:expr, $message:expr) => {{
                let handle = self.allocate_bytecode_exception($class, $message, locals, stack)?;
                vm_throw!(handle);
            }};
        }
        loop {
            if let Some(waiting) = pending.take() {
                // Publish the parent roots before the child can allocate or collect.
                self.publish_frame_roots(depth, locals, stack, &[]);
                let expected = waiting.child.expected_return();
                let resumed = self.resume_suspended_call(waiting.child, depth + 1);
                self.heap.frame_roots.remove(depth);
                match resumed? {
                    CallOutcome::Return(value) => {
                        match (value, expected) {
                            (Some(value), Some(kind)) if value.kind() == kind => {
                                push!(value);
                            }
                            (None, None) => {}
                            _ => return Err(type_error()),
                        }
                        pc = waiting.next_pc;
                    }
                    CallOutcome::Throw(handle) => {
                        vm_throw!(handle);
                    }
                    CallOutcome::Suspend(child) => {
                        pending = Some(PendingCall {
                            child,
                            next_pc: waiting.next_pc,
                        });
                        // A child boundary must suspend every caller in the
                        // saved Java stack even when it was caused by sleep or
                        // yield before the instruction quantum was exhausted.
                        self.scheduler.suspend_requested = true;
                    }
                }
            }
            let host_driver_frame_boundary = self.scheduler.driver_host_poll_yield
                && self.scheduler.current_thread == MAIN_THREAD_ID
                && self.scheduler.host_driver_active
                // An inline LCDUI turn must finish before its Java stack can
                // be parked. The completed presentation keeps the host-poll
                // request set, so the first caller boundary after the turn
                // returns performs the cooperative suspension instead.
                && !self.scheduler.dispatching_display;
            if (self.scheduler.current_thread != MAIN_THREAD_ID
                && (self.scheduler.suspend_requested || self.scheduler.quantum_remaining == 0))
                || host_driver_frame_boundary
            {
                self.scheduler.suspend_requested = false;
                return Ok(CallOutcome::Suspend(Box::new(SuspendedCall {
                    locals: std::mem::take(locals),
                    stack: std::mem::take(stack),
                    pc,
                    synchronized_monitor,
                    pending,
                    ..SuspendedCall::new(method)
                })));
            }
            let has_scheduled_tasks = !self.scheduler.scheduled_tasks.is_empty();
            if has_scheduled_tasks
                && !self.scheduler.dispatching_timer
                && (self.scheduler.timer_poll_requested || self.scheduler.timer_poll_countdown == 0)
                && !self.owns_monitor()
            {
                self.run_due_timer_tasks(depth, locals, stack)?;
            }
            // Keep the original polling instruction at each 1024 boundary.
            // Between boundaries, pure bytecodes share scheduling and stack
            // accounting while retaining all operand/type/bounds checks.
            if batch_enabled {
                let mut budget = (1_023 - self.execution.instructions % 1_024).min(
                    self.limits
                        .max_instructions
                        .saturating_sub(self.execution.instructions),
                );
                if self.scheduler.current_thread != MAIN_THREAD_ID {
                    budget = budget.min(self.scheduler.quantum_remaining);
                }
                if has_scheduled_tasks && self.scheduler.timer_poll_countdown != 0 {
                    budget = budget.min(self.scheduler.timer_poll_countdown);
                }
                if budget != 0 {
                    let parent_slots = self
                        .execution
                        .stack_slots
                        .saturating_sub(operand_stack_slots);
                    let slot_limit = method
                        .max_stack
                        .min(self.limits.max_stack_slots.saturating_sub(parent_slots));
                    let strings = (self.execution.call_stack.len() < self.limits.max_frames
                        && depth < self.limits.max_frames)
                        .then_some(&self.heap.string_values);
                    let batch = super::interpreter_batch::execute_with_strings(
                        method,
                        locals,
                        stack,
                        pc,
                        operand_stack_slots,
                        slot_limit,
                        budget,
                        &mut self.heap.managed,
                        &mut self.classes,
                        strings,
                    );
                    if batch.instructions != 0 {
                        self.execution.instructions += batch.instructions;
                        self.execution.stack_slots = parent_slots + batch.slots;
                        operand_stack_slots = batch.slots;
                        if self.scheduler.current_thread != MAIN_THREAD_ID {
                            self.scheduler.quantum_remaining -= batch.instructions;
                        }
                        if has_scheduled_tasks {
                            self.scheduler.timer_poll_countdown = self
                                .scheduler
                                .timer_poll_countdown
                                .saturating_sub(batch.instructions);
                        }
                        if let Some(frame) = self.execution.call_stack.last_mut() {
                            frame.bytecode_pc = batch.last_pc;
                        }
                        if batch.returned() {
                            let value = method.descriptor.returns.and_then(|_| stack.pop());
                            if let Some(value) = value {
                                self.execution.stack_slots =
                                    self.execution.stack_slots.saturating_sub(value.slots());
                            }
                            return Ok(CallOutcome::Return(value));
                        }
                        pc = batch.pc;
                        sequential_instruction = None;
                        if batch.instructions == budget {
                            continue;
                        }
                    }
                }
            }
            self.execution.instructions = self
                .execution
                .instructions
                .checked_add(1)
                .ok_or_else(|| vm_error("instruction-limit", "instruction counter overflow"))?;
            if has_scheduled_tasks && self.scheduler.timer_poll_countdown != 0 {
                self.scheduler.timer_poll_countdown -= 1;
            }
            if self.execution.instructions.is_multiple_of(1_024) {
                if self.native_context.execution_cancelled() {
                    return Err(vm_error(
                        "execution-cancelled",
                        "guest execution was cancelled by the host",
                    ));
                }
                if self.native_context.execution_suspended() {
                    if self.scheduler.current_thread == MAIN_THREAD_ID
                        && self.scheduler.host_driver_active
                    {
                        // Park even a non-returning startApp/loading callback.
                        // The normal host-poll continuation path retains every
                        // Java frame and lets the lifecycle driver apply pause.
                        self.scheduler.driver_host_poll_yield = true;
                    } else if self.is_scheduler_worker() {
                        // Inline TimerTask callbacks must return to their dispatcher;
                        // they have no worker continuation to park. The enclosing
                        // driver observes lifecycle suspension after the callback.
                        self.scheduler.suspend_requested = true;
                    }
                }
            }
            if self.scheduler.current_thread != MAIN_THREAD_ID {
                self.scheduler.quantum_remaining =
                    self.scheduler.quantum_remaining.saturating_sub(1);
            }
            if self.execution.instructions > self.limits.max_instructions {
                return Err(vm_error(
                    "instruction-limit",
                    format!(
                        "instruction limit {} exceeded; current-locals={locals:?}",
                        self.limits.max_instructions,
                    ),
                ));
            }
            // Resolve bytecode PCs in O(1), including non-sequential branches.
            let ins_index = match sequential_instruction.take() {
                Some(index) if index < method.runtime_instructions.len() => index,
                _ => method
                    .instruction_index
                    .get(pc)
                    .copied()
                    .filter(|index| *index != u16::MAX)
                    .map(usize::from)
                    .ok_or_else(|| {
                        vm_error(
                            "invalid-pc",
                            format!("{} at bytecode offset {pc}", display_key(&method.key)),
                        )
                    })?,
            };
            let ins = &method.runtime_instructions[ins_index];
            let next = ins.next_pc as usize;
            if let Some(frame) = self.execution.call_stack.last_mut() {
                frame.bytecode_pc = pc;
            }
            // Trace every instruction; sample standalone profiling to limit
            // MethodKey cloning and hashing in the interpreter loop.
            let profile_weight = if self.execution.tracing {
                Some(1)
            } else if self.execution.profiling && self.execution.instructions.is_multiple_of(1_024)
            {
                Some(1_024)
            } else {
                None
            };
            if let Some(weight) = profile_weight {
                *self
                    .execution
                    .method_instructions
                    .entry(method.key.clone())
                    .or_default() += weight;
            }
            if self.execution.tracing {
                self.execution.trace.record(format_args!(
                    "{}::{}{} pc={:04} {} stack={:?}",
                    method.key.class,
                    method.key.name,
                    method.key.descriptor,
                    pc,
                    method.instructions[ins_index].mnemonic(),
                    stack
                ));
            }
            if ins.counted_array_fill != 0
                && let Some(next_pc) = self.try_counted_array_fill(
                    method,
                    ins.counted_array_fill,
                    locals,
                    stack.is_empty(),
                )
            {
                pc = next_pc;
                sequential_instruction = None;
                continue;
            }
            macro_rules! pop {
                () => {{
                    let v = stack.pop().ok_or_else(|| {
                        vm_error(
                            "operand-stack-underflow",
                            format!("{} pc={pc}", display_key(&method.key)),
                        )
                    })?;
                    let slots = v.slots();
                    self.execution.stack_slots = self.execution.stack_slots.saturating_sub(slots);
                    operand_stack_slots = operand_stack_slots.saturating_sub(slots);
                    v
                }};
            }
            macro_rules! vm_try {
                ($expression:expr) => {{
                    match $expression {
                        Ok(value) => value,
                        Err(error) => {
                            let Some(class) = java_error_class(&error) else {
                                return Err(error);
                            };
                            if !self.program.classes.contains_key(class) {
                                return Err(error);
                            }
                            let Ok(handle) =
                                self.allocate_error_exception(class, &error, locals, stack)
                            else {
                                return Err(error);
                            };
                            vm_throw!(handle);
                        }
                    }
                }};
            }
            macro_rules! vm_initialize_class {
                ($expression:expr) => {{
                    match vm_try!($expression) {
                        ClassInitializationOutcome::Ready => {}
                        ClassInitializationOutcome::Throw(handle) => {
                            vm_throw!(handle);
                        }
                        ClassInitializationOutcome::Suspend(child) => {
                            return Ok(CallOutcome::Suspend(Box::new(SuspendedCall {
                                locals: std::mem::take(locals),
                                stack: std::mem::take(stack),
                                pc,
                                synchronized_monitor,
                                pending: Some(PendingCall {
                                    child,
                                    // Initialization is a prerequisite, not
                                    // the bytecode itself. Retry the original
                                    // instruction once the class is ready.
                                    next_pc: pc,
                                }),
                                ..SuspendedCall::new(method)
                            })));
                        }
                    }
                }};
            }
            macro_rules! vm_invoke {
                ($callee:expr, $argument_start:expr) => {{
                    match self.invoke_from_stack(
                        $callee,
                        $argument_start,
                        depth,
                        locals,
                        stack,
                        &mut operand_stack_slots,
                    )? {
                        CallOutcome::Return(value) => value,
                        CallOutcome::Throw(handle) => {
                            vm_throw!(handle);
                        }
                        CallOutcome::Suspend(child) => {
                            return Ok(CallOutcome::Suspend(Box::new(SuspendedCall {
                                locals: std::mem::take(locals),
                                stack: std::mem::take(stack),
                                pc,
                                synchronized_monitor,
                                pending: Some(PendingCall {
                                    child,
                                    next_pc: next,
                                }),
                                ..SuspendedCall::new(method)
                            })));
                        }
                    }
                }};
            }
            match ins.opcode {
                0x00 => pc = next,
                0x01 => {
                    push!(Value::Reference(None));
                    pc = next;
                }
                0x02..=0x08 => {
                    push!(Value::Int(i32::from(ins.opcode) - 3));
                    pc = next;
                }
                0x09..=0x0a => {
                    push!(Value::Long(i64::from(ins.opcode - 0x09)));
                    pc = next;
                }
                0x0b..=0x0d => {
                    push!(Value::Float(f32::from(ins.opcode - 0x0b)));
                    pc = next;
                }
                0x0e..=0x0f => {
                    push!(Value::Double(f64::from(ins.opcode - 0x0e)));
                    pc = next;
                }
                0x10 => {
                    push!(Value::Int(i32::from(ins.operands[0] as i8)));
                    pc = next;
                }
                0x11 => {
                    push!(Value::Int(i32::from(i16::from_be_bytes([
                        ins.operands[0],
                        ins.operands[1]
                    ]))));
                    pc = next;
                }
                0x12..=0x14 => {
                    let index = if ins.opcode == 0x12 {
                        u16::from(ins.operands[0])
                    } else {
                        u16::from_be_bytes([ins.operands[0], ins.operands[1]])
                    };
                    let value = match method
                        .constants
                        .get(usize::from(index))
                        .and_then(Option::as_ref)
                    {
                        Some(Constant::String { string_index }) => Value::Reference(Some(
                            self.intern_string_constant(method, *string_index, locals, stack)?,
                        )),
                        _ => constant_value(&method.constants, index)?,
                    };
                    if (ins.opcode == 0x14) != (value.slots() == 2) {
                        return Err(vm_error(
                            "type-mismatch",
                            format!(
                                "{} uses the wrong constant category",
                                method.instructions[ins_index].mnemonic()
                            ),
                        ));
                    }
                    push!(value);
                    pc = next;
                }
                0x15..=0x19 => {
                    let expected = kind_for_typed_opcode(ins.opcode)?;
                    let v = get_local_typed(locals, usize::from(ins.operands[0]), expected)?;
                    push!(v);
                    pc = next;
                }
                0x1a..=0x2d => {
                    let index = usize::from((ins.opcode - 0x1a) % 4);
                    push!(get_local_typed(
                        locals,
                        index,
                        kind_for_typed_opcode(ins.opcode)?
                    )?);
                    pc = next;
                }
                0x2e..=0x35 => {
                    let Value::Int(index) = pop!() else {
                        return Err(type_error());
                    };
                    let Value::Reference(reference) = pop!() else {
                        return Err(type_error());
                    };
                    let Some(handle) = reference else {
                        vm_raise!("java/lang/NullPointerException", None)
                    };
                    let access = typed_array_access(ins.opcode)?;
                    let value = match self.heap.managed.array_get_typed(handle, index, access) {
                        Ok(value) => value,
                        Err(HeapError::Bounds) => vm_raise!(
                            "java/lang/ArrayIndexOutOfBoundsException",
                            Some(&format!("array index {index} out of bounds"))
                        ),
                        Err(error) => return Err(array_opcode_error(error)),
                    };
                    push!(value);
                    pc = next;
                }
                0x36..=0x39 => {
                    let index = usize::from(ins.operands[0]);
                    let v = pop!();
                    require_kind(v, kind_for_typed_opcode(ins.opcode)?)?;
                    set_local(locals, index, v)?;
                    pc = next;
                }
                0x3a => {
                    let v = pop!();
                    if !matches!(v, Value::Reference(_)) && !valid_return_address(method, v) {
                        return Err(vm_error(
                            "type-mismatch",
                            format!("astore expected a reference or return address, found {v:?}"),
                        ));
                    }
                    set_local(locals, usize::from(ins.operands[0]), v)?;
                    pc = next;
                }
                0x3b..=0x4a => {
                    let index = usize::from((ins.opcode - 0x3b) % 4);
                    let v = pop!();
                    require_kind(v, kind_for_typed_opcode(ins.opcode)?)?;
                    set_local(locals, index, v)?;
                    pc = next;
                }
                0x4b..=0x4e => {
                    let v = pop!();
                    if !matches!(v, Value::Reference(_)) && !valid_return_address(method, v) {
                        return Err(type_error());
                    }
                    set_local(locals, usize::from(ins.opcode - 0x4b), v)?;
                    pc = next;
                }
                0x4f..=0x56 => {
                    let value = pop!();
                    let Value::Int(index) = pop!() else {
                        return Err(type_error());
                    };
                    let Value::Reference(reference) = pop!() else {
                        return Err(type_error());
                    };
                    let Some(handle) = reference else {
                        vm_raise!("java/lang/NullPointerException", None)
                    };
                    require_array_opcode(ins.opcode - 0x21, value)?;
                    let access = typed_array_access(ins.opcode - 0x21)?;
                    if ins.opcode == 0x53 {
                        let Allocation::Array {
                            kind: ArrayKind::Reference(component),
                            elements,
                        } = self.heap.managed.get(handle).map_err(heap_error)?
                        else {
                            return Err(type_error());
                        };
                        // aastore must report an invalid index before checking
                        // whether the value is assignable to the component type.
                        if usize::try_from(index)
                            .ok()
                            .is_none_or(|index| index >= elements.len())
                        {
                            vm_raise!(
                                "java/lang/ArrayIndexOutOfBoundsException",
                                Some(&format!("array index {index} out of bounds"))
                            )
                        }
                        if let Err(error) = self.validate_reference_component(component, value) {
                            if error.code() != "array-store-exception" {
                                return Err(error);
                            }
                            vm_raise!(
                                "java/lang/ArrayStoreException",
                                Some("incompatible array element")
                            )
                        }
                    }
                    match self
                        .heap
                        .managed
                        .array_set_typed(handle, index, access, value)
                    {
                        Ok(()) => {}
                        Err(HeapError::Bounds) => vm_raise!(
                            "java/lang/ArrayIndexOutOfBoundsException",
                            Some(&format!("array index {index} out of bounds"))
                        ),
                        Err(error) => return Err(array_opcode_error(error)),
                    }
                    pc = next;
                }
                0x57..=0x5f => {
                    let parent_slots = self
                        .execution
                        .stack_slots
                        .saturating_sub(operand_stack_slots);
                    let limit = method
                        .max_stack
                        .min(self.limits.max_stack_slots.saturating_sub(parent_slots));
                    operand_stack_slots =
                        stack::rearrange(ins.opcode, stack, operand_stack_slots, limit).map_err(
                            |error| {
                                let code = match error {
                                    stack::Error::TypeMismatch => return type_error(),
                                    stack::Error::Underflow => "operand-stack-underflow",
                                    stack::Error::Overflow => "operand-stack-overflow",
                                    stack::Error::UnsupportedOpcode => "unsupported-opcode",
                                };
                                vm_error(code, format!("{} pc={pc}", display_key(&method.key)))
                            },
                        )?;
                    self.execution.stack_slots = parent_slots + operand_stack_slots;
                    pc = next;
                }
                0x60..=0x73 => {
                    let b = pop!();
                    let a = pop!();
                    match binary(ins.opcode, a, b) {
                        Ok(value) => push!(value),
                        Err(error)
                            if error.code() == "arithmetic-exception"
                                && self
                                    .program
                                    .classes
                                    .contains_key("java/lang/ArithmeticException") =>
                        {
                            let handle = self.allocate_exception(
                                "java/lang/ArithmeticException",
                                Some("/ by zero"),
                                locals,
                                stack,
                            )?;
                            vm_throw!(handle);
                        }
                        Err(error) => return Err(error),
                    }
                    pc = next;
                }
                0x74..=0x77 => {
                    let a = pop!();
                    push!(negate(ins.opcode, a)?);
                    pc = next;
                }
                0x78..=0x83 => {
                    let b = pop!();
                    let a = pop!();
                    push!(bitwise(ins.opcode, a, b)?);
                    pc = next;
                }
                0x84 => {
                    let i = usize::from(ins.operands[0]);
                    let add = i32::from(ins.operands[1] as i8);
                    let Value::Int(v) = get_local(locals, i)? else {
                        return Err(type_error());
                    };
                    set_local(locals, i, Value::Int(v.wrapping_add(add)))?;
                    pc = next;
                }
                0x85..=0x93 => {
                    let v = pop!();
                    push!(convert(ins.opcode, v)?);
                    pc = next;
                }
                0x94..=0x98 => {
                    let b = pop!();
                    let a = pop!();
                    push!(Value::Int(compare(ins.opcode, a, b)?));
                    pc = next;
                }
                0x99..=0x9e => {
                    let Value::Int(v) = pop!() else {
                        return Err(type_error());
                    };
                    pc = if test_zero(ins.opcode, v) {
                        branch(pc, &ins.operands)?
                    } else {
                        next
                    };
                }
                0x9f..=0xa4 => {
                    let Value::Int(b) = pop!() else {
                        return Err(type_error());
                    };
                    let Value::Int(a) = pop!() else {
                        return Err(type_error());
                    };
                    pc = if test_pair(ins.opcode, a, b) {
                        branch(pc, &ins.operands)?
                    } else {
                        next
                    };
                }
                0xa5..=0xa6 => {
                    let Value::Reference(b) = pop!() else {
                        return Err(type_error());
                    };
                    let Value::Reference(a) = pop!() else {
                        return Err(type_error());
                    };
                    pc = if (a == b) == (ins.opcode == 0xa5) {
                        branch(pc, &ins.operands)?
                    } else {
                        next
                    };
                }
                0xa7 => pc = branch(pc, &ins.operands)?,
                0xa8 => {
                    push!(Value::Int(i32::try_from(next).map_err(|_| type_error())?));
                    pc = branch(pc, &ins.operands)?;
                }
                0xa9 => {
                    let value = get_local(locals, usize::from(ins.operands[0]))?;
                    if !valid_return_address(method, value) {
                        return Err(type_error());
                    }
                    let Value::Int(target) = value else {
                        unreachable!()
                    };
                    pc = usize::try_from(target).map_err(|_| type_error())?;
                }
                0xaa => {
                    let Value::Int(key) = pop!() else {
                        return Err(type_error());
                    };
                    pc = switch_table(pc, &method.code, key)?;
                }
                0xab => {
                    let Value::Int(key) = pop!() else {
                        return Err(type_error());
                    };
                    pc = switch_lookup(pc, &method.code, key)?;
                }
                0xac..=0xb0 => {
                    let value = stack.pop().ok_or_else(|| {
                        vm_error(
                            "operand-stack-underflow",
                            format!("{} pc={pc}", display_key(&method.key)),
                        )
                    })?;
                    self.execution.stack_slots =
                        self.execution.stack_slots.saturating_sub(value.slots());
                    let expected = kind_for_return_opcode(ins.opcode)?;
                    if method.descriptor.returns != Some(expected) || value.kind() != expected {
                        return Err(vm_error(
                            "type-mismatch",
                            format!("return opcode does not match {}", method.key.descriptor),
                        ));
                    }
                    return Ok(CallOutcome::Return(Some(value)));
                }
                0xb1 => {
                    if method.descriptor.returns.is_some() {
                        return Err(vm_error(
                            "type-mismatch",
                            format!("void return does not match {}", method.key.descriptor),
                        ));
                    }
                    return Ok(CallOutcome::Return(None));
                }
                0xb2..=0xb5 => {
                    let field_index = u16::from_be_bytes([ins.operands[0], ins.operands[1]]);
                    if matches!(ins.opcode, 0xb2 | 0xb3)
                        && let Some(access) =
                            self.classes.static_field_fast_access(method, field_index)
                        && access
                            .slots
                            .declaring_class
                            .is_some_and(|slot| self.classes.initialized.contains_at(slot))
                        && let Some(field_slot) = access.slots.static_field
                    {
                        if ins.opcode == 0xb2 {
                            if let Some(value) =
                                self.classes.static_fields.get_linked(field_slot).copied()
                            {
                                push!(value);
                                pc = next;
                                sequential_instruction = Some(ins_index + 1);
                                continue;
                            }
                        } else if !self.native_context.vm_flight_recorder_enabled()
                            && let Some(target) =
                                self.classes.static_fields.get_linked_mut(field_slot)
                        {
                            let value = pop!();
                            require_kind(value, access.kind)?;
                            *target = value;
                            pc = next;
                            sequential_instruction = Some(ins_index + 1);
                            continue;
                        }
                    }
                    let (definition, field_slots) = vm_try!(
                        self.resolve_field_cached(method, field_index)
                            .map_err(|error| {
                                annotate_resolution(
                                    error.message().to_owned(),
                                    method,
                                    pc,
                                    ins.opcode,
                                    error,
                                )
                            })
                    );
                    if definition.is_static != matches!(ins.opcode, 0xb2 | 0xb3) {
                        let message = match ins.opcode {
                            0xb2 => "getstatic on instance field",
                            0xb3 => "putstatic on instance field",
                            0xb4 => "getfield on static field",
                            _ => "putfield on static field",
                        };
                        let _: () = vm_try!(Err::<(), EmuError>(vm_error(
                            "incompatible-class-change",
                            message,
                        )));
                    }
                    if matches!(ins.opcode, 0xb2 | 0xb3)
                        && !field_slots
                            .declaring_class
                            .is_some_and(|slot| self.classes.initialized.contains_at(slot))
                    {
                        vm_initialize_class!(
                            self.initialize_class_from_frame(
                                &definition.declaring_class,
                                method,
                                depth,
                                locals,
                                stack,
                            )
                            .map_err(|error| {
                                annotate_resolution(
                                    format!("class {}", definition.declaring_class),
                                    method,
                                    pc,
                                    ins.opcode,
                                    error,
                                )
                            })
                        );
                    }
                    match ins.opcode {
                        0xb2 => {
                            let value = field_slots
                                .static_field
                                .and_then(|slot| {
                                    self.classes.static_fields.get_at(slot, &definition.key)
                                })
                                .or_else(|| self.classes.static_fields.get(&definition.key))
                                .copied()
                                .unwrap_or_else(|| {
                                    if let Some(slot) = field_slots.static_field {
                                        self.classes.static_fields.insert_at(
                                            slot,
                                            &definition.key,
                                            definition.initial,
                                        );
                                    } else {
                                        self.classes
                                            .static_fields
                                            .insert(definition.key.to_string(), definition.initial);
                                    }
                                    definition.initial
                                });
                            push!(value);
                        }
                        0xb3 => {
                            let value = pop!();
                            require_kind(value, definition.kind)?;
                            if self.native_context.vm_flight_recorder_enabled() {
                                self.native_context.record_vm_flight(format!(
                                    "instruction={} thread={} field-write static {} value={value:?} from={} pc={pc}",
                                    self.execution.instructions,
                                    self.scheduler.current_thread,
                                    definition.key,
                                    display_key(&method.key),
                                ));
                            }
                            if let Some(slot) = field_slots.static_field {
                                self.classes
                                    .static_fields
                                    .insert_at(slot, &definition.key, value);
                            } else if let Some(value_slot) =
                                self.classes.static_fields.get_mut(&definition.key)
                            {
                                *value_slot = value;
                            } else {
                                self.classes
                                    .static_fields
                                    .insert(definition.key.to_string(), value);
                            }
                        }
                        0xb4 => {
                            let Value::Reference(reference) = pop!() else {
                                return Err(type_error());
                            };
                            let Some(handle) = reference else {
                                vm_raise!("java/lang/NullPointerException", None)
                            };
                            let value = if let Some(slot) = definition.instance_slot {
                                self.heap
                                    .managed
                                    .field_at(
                                        handle,
                                        slot,
                                        &definition.field_token,
                                        &definition.key,
                                    )
                                    .map_err(heap_error)?
                            } else {
                                self.heap
                                    .managed
                                    .field(handle, &definition.key)
                                    .map_err(heap_error)?
                            };
                            push!(value);
                        }
                        0xb5 => {
                            let value = pop!();
                            require_kind(value, definition.kind)?;
                            let Value::Reference(reference) = pop!() else {
                                return Err(type_error());
                            };
                            let Some(handle) = reference else {
                                vm_raise!("java/lang/NullPointerException", None)
                            };
                            if self.native_context.vm_flight_recorder_enabled() {
                                self.native_context.record_vm_flight(format!(
                                    "instruction={} thread={} field-write object={handle:?} {} value={value:?} from={} pc={pc}",
                                    self.execution.instructions,
                                    self.scheduler.current_thread,
                                    definition.key,
                                    display_key(&method.key),
                                ));
                            }
                            if let Some(slot) = definition.instance_slot {
                                self.heap
                                    .managed
                                    .set_field_at(
                                        handle,
                                        slot,
                                        &definition.field_token,
                                        &definition.key,
                                        value,
                                    )
                                    .map_err(heap_error)?;
                            } else {
                                self.heap
                                    .managed
                                    .set_field(handle, &definition.key, value)
                                    .map_err(heap_error)?;
                            }
                        }
                        _ => unreachable!(),
                    }
                    pc = next;
                }
                0xb6 | 0xb7 | 0xb9 => {
                    let index = u16::from_be_bytes([ins.operands[0], ins.operands[1]]);
                    let cached_special = if ins.opcode == 0xb7 {
                        self.fixed_method_inline_cached(method, index, ins.opcode)
                            .map(|(callee, _)| callee)
                    } else {
                        None
                    };
                    let symbolic = if cached_special.is_none() {
                        Some(vm_try!(
                            self.resolve_method_ref_cached(method, index)
                                .map_err(|error| {
                                    annotate_resolution(
                                        error.message().to_owned(),
                                        method,
                                        pc,
                                        ins.opcode,
                                        error,
                                    )
                                })
                        ))
                    } else {
                        None
                    };
                    let (descriptor, is_static) = if let Some(callee) = cached_special.as_ref() {
                        (&callee.descriptor, callee.is_static)
                    } else {
                        let symbolic = symbolic.as_ref().expect("uncached invoke has a symbol");
                        (&symbolic.descriptor, symbolic.is_static)
                    };
                    if is_static {
                        let _: () = vm_try!(Err::<(), EmuError>(vm_error(
                            "incompatible-class-change",
                            "instance invocation on static method",
                        )));
                    }
                    if ins.opcode == 0xb9 {
                        let expected_count = 1usize
                            + descriptor
                                .parameters
                                .iter()
                                .map(|kind| match kind {
                                    ValueKind::Long | ValueKind::Double => 2,
                                    _ => 1,
                                })
                                .sum::<usize>();
                        if usize::from(ins.operands[2]) != expected_count || ins.operands[3] != 0 {
                            return Err(vm_error(
                                "invalid-invokeinterface",
                                "invalid count or reserved byte",
                            ));
                        }
                    }
                    let argument_count = descriptor.parameters.len().saturating_add(1);
                    let argument_start =
                        stack.len().checked_sub(argument_count).ok_or_else(|| {
                            vm_error(
                                "operand-stack-underflow",
                                format!("{} pc={pc}", display_key(&method.key)),
                            )
                        })?;
                    let parameters_match = stack[argument_start + 1..]
                        .iter()
                        .zip(&descriptor.parameters)
                        .all(|(value, expected)| value.kind() == *expected);
                    if !parameters_match {
                        return Err(type_error());
                    }
                    let Value::Reference(reference) = stack[argument_start] else {
                        return Err(type_error());
                    };
                    let Some(receiver) = reference else {
                        let removed_slots = slot_count(&stack[argument_start..]);
                        stack.truncate(argument_start);
                        self.execution.stack_slots =
                            self.execution.stack_slots.saturating_sub(removed_slots);
                        operand_stack_slots = operand_stack_slots.saturating_sub(removed_slots);
                        vm_raise!("java/lang/NullPointerException", None)
                    };
                    let callee = if let Some(callee) = cached_special {
                        callee
                    } else if ins.opcode == 0xb7 {
                        let symbolic = symbolic.as_ref().expect("uncached invoke has a symbol");
                        vm_try!(
                            self.resolve_fixed_method_cached(
                                method,
                                index,
                                ins.opcode,
                                &symbolic.symbolic
                            )
                            .map_err(|error| {
                                annotate_resolution(
                                    display_key(&symbolic.symbolic),
                                    method,
                                    pc,
                                    ins.opcode,
                                    error,
                                )
                            })
                        )
                        .0
                    } else {
                        let symbolic = symbolic.as_ref().expect("virtual invoke has a symbol");
                        vm_try!(
                            self.resolve_virtual_method_cached(
                                method,
                                index,
                                receiver,
                                &symbolic.symbolic,
                            )
                            .map_err(|error| {
                                annotate_resolution(
                                    display_key(&symbolic.symbolic),
                                    method,
                                    pc,
                                    ins.opcode,
                                    error,
                                )
                            })
                        )
                    };
                    if callee.is_static {
                        let _: () = vm_try!(Err::<(), EmuError>(vm_error(
                            "incompatible-class-change",
                            "instance invocation on static method",
                        )));
                    }
                    if let Some(value) = vm_invoke!(&callee, argument_start) {
                        push!(value);
                    }
                    pc = next;
                }
                0xb8 => {
                    let index = u16::from_be_bytes([ins.operands[0], ins.operands[1]]);
                    let (callee, declaring_class) = if let Some(cached) =
                        self.fixed_method_inline_cached(method, index, ins.opcode)
                    {
                        cached
                    } else {
                        let symbolic = vm_try!(
                            self.resolve_method_ref_cached(method, index)
                                .map_err(|error| {
                                    annotate_resolution(
                                        error.message().to_owned(),
                                        method,
                                        pc,
                                        ins.opcode,
                                        error,
                                    )
                                })
                        );
                        vm_try!(
                            self.resolve_fixed_method_cached(
                                method,
                                index,
                                ins.opcode,
                                &symbolic.symbolic
                            )
                            .map_err(|error| {
                                annotate_resolution(
                                    display_key(&symbolic.symbolic),
                                    method,
                                    pc,
                                    ins.opcode,
                                    error,
                                )
                            })
                        )
                    };
                    if !callee.is_static {
                        let _: () = vm_try!(Err::<(), EmuError>(vm_error(
                            "incompatible-class-change",
                            "invokestatic on instance method",
                        )));
                    }
                    // Fully linked classfile methods always carry a Program
                    // stack-key slot, so their declaring class is necessarily
                    // present. Avoid hashing the class name on every hot
                    // invokestatic; hand-built methods used by conformance
                    // fixtures retain the defensive symbolic fallback.
                    let already_initialized = declaring_class
                        .is_some_and(|slot| self.classes.initialized.contains_at(slot));
                    if !already_initialized
                        && (callee.stack_key_id.is_some()
                            || self.program.classes.contains_key(&callee.key.class))
                    {
                        vm_initialize_class!(
                            self.initialize_class_from_frame(
                                &callee.key.class,
                                method,
                                depth,
                                locals,
                                stack
                            )
                            .map_err(|error| {
                                annotate_resolution(
                                    &callee.key.class,
                                    method,
                                    pc,
                                    ins.opcode,
                                    error,
                                )
                            })
                        );
                    }
                    let argument_start = stack
                        .len()
                        .checked_sub(callee.descriptor.parameters.len())
                        .ok_or_else(|| {
                            vm_error(
                                "operand-stack-underflow",
                                format!("{} pc={pc}", display_key(&method.key)),
                            )
                        })?;
                    validate_call_arguments(&callee, &stack[argument_start..])?;
                    let returned = vm_invoke!(&callee, argument_start);
                    match (returned, callee.descriptor.returns) {
                        (Some(value), Some(expected)) if value.kind() == expected => {
                            push!(value)
                        }
                        (None, None) => {}
                        _ => return Err(type_error()),
                    }
                    pc = next;
                }
                0xbb => {
                    let class = resolve_class(
                        &method.constants,
                        u16::from_be_bytes([ins.operands[0], ins.operands[1]]),
                    )
                    .map_err(|error| {
                        annotate_resolution(
                            error.message().to_owned(),
                            method,
                            pc,
                            ins.opcode,
                            error,
                        )
                    })?;
                    vm_initialize_class!(
                        self.initialize_class_from_frame(class, method, depth, locals, stack)
                            .map_err(|error| {
                                annotate_resolution(class, method, pc, ins.opcode, error)
                            })
                    );
                    let fields = vm_try!(self.initial_instance_fields(class).map_err(|error| {
                        annotate_resolution(class, method, pc, ins.opcode, error)
                    }));
                    let handle = vm_try!(self.allocate_linked_object(class, fields, locals, stack));
                    push!(Value::Reference(Some(handle)));
                    pc = next;
                }
                0xbc | 0xbd => {
                    let Value::Int(length) = pop!() else {
                        return Err(type_error());
                    };
                    if length < 0 {
                        vm_raise!(
                            "java/lang/NegativeArraySizeException",
                            Some("negative array size")
                        )
                    }
                    let kind = if ins.opcode == 0xbc {
                        primitive_array_kind(ins.operands[0])?
                    } else {
                        ArrayKind::Reference(
                            resolve_class(
                                &method.constants,
                                u16::from_be_bytes([ins.operands[0], ins.operands[1]]),
                            )
                            .map_err(|error| {
                                annotate_resolution(
                                    error.message().to_owned(),
                                    method,
                                    pc,
                                    ins.opcode,
                                    error,
                                )
                            })?
                            .into(),
                        )
                    };
                    let handle = vm_try!(self.allocate_array(kind, length, locals, stack));
                    push!(Value::Reference(Some(handle)));
                    pc = next;
                }
                0xbe => {
                    let Value::Reference(reference) = pop!() else {
                        return Err(type_error());
                    };
                    let Some(handle) = reference else {
                        vm_raise!("java/lang/NullPointerException", None)
                    };
                    let length = self.heap.managed.array_length(handle).map_err(heap_error)?;
                    push!(Value::Int(i32::try_from(length).map_err(|_| vm_error(
                        "array-limit",
                        "array length exceeds i32"
                    ))?));
                    pc = next;
                }
                0xbf => {
                    let Value::Reference(reference) = pop!() else {
                        return Err(type_error());
                    };
                    let Some(handle) = reference else {
                        vm_raise!("java/lang/NullPointerException", Some("cannot throw null"))
                    };
                    vm_throw!(handle);
                }
                0xc0 | 0xc1 => {
                    let target_class = resolve_class(
                        &method.constants,
                        u16::from_be_bytes([ins.operands[0], ins.operands[1]]),
                    )
                    .map_err(|error| {
                        annotate_resolution(
                            error.message().to_owned(),
                            method,
                            pc,
                            ins.opcode,
                            error,
                        )
                    })?;
                    let value = pop!();
                    let Value::Reference(reference) = value else {
                        return Err(type_error());
                    };
                    let matches = match reference {
                        None => true,
                        Some(handle) => self.is_instance(&self.object_class(handle)?, target_class),
                    };
                    if ins.opcode == 0xc1 {
                        push!(Value::Int(i32::from(reference.is_some() && matches)));
                    } else if matches {
                        push!(value);
                    } else {
                        vm_raise!(
                            "java/lang/ClassCastException",
                            Some(&format!("cannot cast to {target_class}"))
                        )
                    }
                    pc = next;
                }
                0xc2 | 0xc3 => {
                    let Value::Reference(reference) = pop!() else {
                        return Err(type_error());
                    };
                    let Some(handle) = reference else {
                        let _: () = vm_try!(Err::<(), EmuError>(vm_error(
                            "null-pointer-exception",
                            "null monitor reference",
                        )));
                        unreachable!();
                    };
                    if ins.opcode == 0xc2 {
                        if !vm_try!(self.enter_monitor(handle, depth)) {
                            // `monitorenter` consumed its operand before the
                            // worker discovered contention. Preserve the
                            // operand and bytecode PC in the continuation so
                            // the instruction can retry after the owner fully
                            // releases the monitor.
                            push!(Value::Reference(Some(handle)));
                            self.scheduler.suspend_requested = true;
                            continue;
                        }
                    } else {
                        vm_try!(self.exit_monitor(handle));
                    }
                    pc = next;
                }
                0xc5 => {
                    let descriptor = resolve_class(
                        &method.constants,
                        u16::from_be_bytes([ins.operands[0], ins.operands[1]]),
                    )
                    .map_err(|error| {
                        annotate_resolution(
                            error.message().to_owned(),
                            method,
                            pc,
                            ins.opcode,
                            error,
                        )
                    })?;
                    let dimensions = usize::from(ins.operands[2]);
                    let mut lengths = Vec::with_capacity(dimensions);
                    for _ in 0..dimensions {
                        let Value::Int(length) = pop!() else {
                            return Err(type_error());
                        };
                        lengths.push(length);
                    }
                    lengths.reverse();
                    if lengths.iter().any(|length| *length < 0) {
                        let _: () = vm_try!(Err::<(), EmuError>(vm_error(
                            "negative-array-size-exception",
                            "negative array size",
                        )));
                        unreachable!();
                    }
                    let handle =
                        vm_try!(self.allocate_multi_array(descriptor, &lengths, locals, stack,));
                    push!(Value::Reference(Some(handle)));
                    pc = next;
                }
                0xc6 | 0xc7 => {
                    let Value::Reference(value) = pop!() else {
                        return Err(type_error());
                    };
                    pc = if value.is_none() == (ins.opcode == 0xc6) {
                        branch(pc, &ins.operands)?
                    } else {
                        next
                    };
                }
                0xc4 => {
                    let op = ins.operands[0];
                    let index = usize::from(u16::from_be_bytes([ins.operands[1], ins.operands[2]]));
                    match op {
                        0x15..=0x19 => {
                            push!(get_local_typed(locals, index, kind_for_typed_opcode(op)?)?);
                        }
                        0x36..=0x39 => {
                            let v = pop!();
                            require_kind(v, kind_for_typed_opcode(op)?)?;
                            set_local(locals, index, v)?;
                        }
                        0x3a => {
                            let value = pop!();
                            if !matches!(value, Value::Reference(_))
                                && !valid_return_address(method, value)
                            {
                                return Err(type_error());
                            }
                            set_local(locals, index, value)?;
                        }
                        0x84 => {
                            let add =
                                i32::from(i16::from_be_bytes([ins.operands[3], ins.operands[4]]));
                            let Value::Int(v) = get_local(locals, index)? else {
                                return Err(type_error());
                            };
                            set_local(locals, index, Value::Int(v.wrapping_add(add)))?;
                        }
                        0xa9 => {
                            let value = get_local(locals, index)?;
                            if !valid_return_address(method, value) {
                                return Err(type_error());
                            }
                            let Value::Int(target) = value else {
                                unreachable!()
                            };
                            pc = usize::try_from(target).map_err(|_| type_error())?;
                            continue;
                        }
                        _ => {
                            return Err(vm_error("unsupported-opcode", "unsupported wide opcode"));
                        }
                    }
                    pc = next;
                }
                0xc8 => pc = branch_wide(pc, &ins.operands)?,
                0xc9 => {
                    push!(Value::Int(i32::try_from(next).map_err(|_| type_error())?));
                    pc = branch_wide(pc, &ins.operands)?;
                }
                _ => {
                    return Err(vm_error(
                        "unsupported-opcode",
                        format!(
                            "{} (0x{:02x}) in {}",
                            method.instructions[ins_index].mnemonic(),
                            ins.opcode,
                            display_key(&method.key)
                        ),
                    ));
                }
            }
            if pc == next {
                sequential_instruction = Some(ins_index + 1);
            }
        }
    }
}
use super::{
    Allocation, ArrayKind, CallOutcome, ClassInitializationOutcome, Constant, EmuError, Handle,
    HeapError, MAIN_THREAD_ID, Machine, Method, PendingCall, SuspendedCall, Value, ValueKind,
    annotate_resolution, array_opcode_error, binary, bitwise, branch, branch_wide, compare,
    compatibility_intrinsic_candidate, constant_value, convert, display_key, get_local,
    get_local_typed, heap_error, java_error_class, kind_for_return_opcode, kind_for_typed_opcode,
    negate, primitive_array_kind, require_array_opcode, require_kind, resolve_class, set_local,
    slot_count, switch_lookup, switch_table, test_pair, test_zero, type_error, typed_array_access,
    valid_return_address, validate_call_arguments, vm_error,
};
