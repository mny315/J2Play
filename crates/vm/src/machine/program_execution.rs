//! Isolated VM execution and host-driven instance calls.

use super::{
    CallOutcome, CallTarget, ClassState, DefaultNativeContext, DeviceState, DriverStep, DriverTurn,
    EmuError, Execution, ExecutionCounters, ExecutionState, GraphicsState, HashMap, HeapState,
    HostServices, InstanceCall, Jsr239State, Limits, M3gExecutionMetrics, M3gGraphicsState,
    M3gState, MAIN_THREAD_ID, Machine, MethodKey, Micro3dState, Program, SchedulerState,
    SuspendedDriverCall, SuspendedDriverWake, Value, VecDeque, VibrationRequest, display_key,
    timer_poll_instruction_interval, vm_error,
};

impl Program {
    /// Executes a no-argument static method and returns its primitive value.
    ///
    /// # Errors
    ///
    /// Returns a categorized VM diagnostic for invalid bytecode state,
    /// unsupported operations, VM exceptions, or resource-limit exhaustion.
    pub fn execute(
        &self,
        class: &str,
        name: &str,
        descriptor: &str,
        limits: Limits,
        trace: bool,
    ) -> Result<Execution, EmuError> {
        let mut context = DefaultNativeContext;
        self.execute_with_context(class, name, descriptor, limits, trace, &mut context)
    }

    /// Executes with explicit host services for native methods.
    ///
    /// # Errors
    /// Returns the same categorized diagnostics as [`Program::execute`], plus
    /// errors produced by the supplied native context.
    pub fn execute_with_context(
        &self,
        class: &str,
        name: &str,
        descriptor: &str,
        limits: Limits,
        trace: bool,
        context: &mut dyn HostServices,
    ) -> Result<Execution, EmuError> {
        limits.validate()?;
        let key = MethodKey {
            class: class.to_owned(),
            name: name.to_owned(),
            descriptor: descriptor.to_owned(),
        };
        let method = self
            .methods
            .get(&key)
            .ok_or_else(|| vm_error("method-not-found", format!("{class}::{name}{descriptor}")))?;
        if !method.descriptor.parameters.is_empty() {
            return Err(vm_error(
                "entry-arguments",
                "entry method must have no arguments",
            ));
        }
        let mut machine = self.machine(limits, trace, context);
        if self.classes.contains_key(class) {
            machine.initialize_class(class, 1)?;
        }
        let value = match machine.call(method, [], 1)? {
            CallOutcome::Return(value) => value,
            CallOutcome::Suspend(_) => unreachable!("main thread cannot be suspended"),
            CallOutcome::Throw(handle) => {
                return Err(machine.throwable_error(
                    handle,
                    if machine.heap.managed_heap_limit_throwables.contains(&handle) {
                        super::MANAGED_HEAP_LIMIT_CODE
                    } else {
                        "uncaught-exception"
                    },
                    "entry method",
                )?);
            }
        };
        while machine.run_one_thread(1)? {}
        Ok(machine.finish_execution(value))
    }

    /// Creates one object and invokes a sequence of instance methods in a
    /// single isolated VM execution. This is the VM-facing primitive used by
    /// lifecycle managers without introducing a dependency on MIDP.
    ///
    /// # Errors
    /// Returns a categorized diagnostic when construction, invocation, or
    /// shutdown fails.
    pub fn execute_instance_sequence_with_context(
        &self,
        class: &str,
        calls: &[InstanceCall],
        limits: Limits,
        trace: bool,
        context: &mut dyn HostServices,
    ) -> Result<Execution, EmuError> {
        let mut calls = calls.iter().cloned();
        self.execute_instance_driver_with_context(
            class,
            || Ok(calls.next()),
            limits,
            trace,
            context,
        )
    }

    /// Creates one object and lets a host-side driver choose each successive
    /// instance call. The driver runs between callbacks, so lifecycle managers
    /// can react to notifications emitted by the preceding callback while the
    /// Java instance, heap and statics remain alive.
    ///
    /// # Errors
    /// Returns a categorized diagnostic when construction, driver operation,
    /// invocation, or shutdown fails.
    pub fn execute_instance_driver_with_context<F>(
        &self,
        class: &str,
        mut next_call: F,
        limits: Limits,
        trace: bool,
        context: &mut dyn HostServices,
    ) -> Result<Execution, EmuError>
    where
        F: FnMut() -> Result<Option<InstanceCall>, EmuError>,
    {
        self.execute_instance_step_driver_with_context(
            class,
            |_| {
                Ok(match next_call()? {
                    Some(call) => DriverStep::Call(call),
                    None => DriverStep::Stop,
                })
            },
            limits,
            trace,
            context,
        )
    }

    /// Like [`Self::execute_instance_driver_with_context`], but allows a host
    /// scheduling tick that advances worker threads without dispatching a
    /// synthetic guest Java method.
    ///
    /// # Errors
    /// Returns a categorized diagnostic when construction, driver operation,
    /// invocation, scheduling, or shutdown fails.
    pub fn execute_instance_step_driver_with_context<F>(
        &self,
        class: &str,
        next_step: F,
        limits: Limits,
        trace: bool,
        context: &mut dyn HostServices,
    ) -> Result<Execution, EmuError>
    where
        F: FnMut(DriverTurn) -> Result<DriverStep, EmuError>,
    {
        self.execute_instance_checkpoint_driver_with_context(
            class, next_step, limits, trace, context, None,
        )
    }

    /// Runs or resumes one instance from a validated automatic checkpoint.
    /// The program and host backends must match the checkpoint's launch identity.
    /// Consumes and releases checkpoint bytes before resuming guest execution.
    ///
    /// # Errors
    /// Returns a diagnostic for invalid state or failed guest execution.
    pub fn execute_instance_checkpoint_driver_with_context<F>(
        &self,
        class: &str,
        mut next_step: F,
        limits: Limits,
        trace: bool,
        context: &mut dyn HostServices,
        checkpoint: Option<Vec<u8>>,
    ) -> Result<Execution, EmuError>
    where
        F: FnMut(DriverTurn) -> Result<DriverStep, EmuError>,
    {
        limits.validate()?;
        let mut machine = self.machine(limits, trace, context);
        if !self.classes.contains_key(class) {
            return Err(vm_error("class-not-found", class));
        }
        let instance = if let Some(bytes) = checkpoint {
            let instance = machine.restore_checkpoint(class, &bytes)?;
            drop(bytes);
            machine.native_context.checkpoint_restored()?;
            machine.scheduler.instruction_pacing_started_millis =
                machine.native_context.monotonic_millis();
            instance
        } else {
            machine.initialize_class(class, 1)?;
            let instance = machine.allocate_native_instance(class, &[])?;
            let constructor = machine.resolve_symbolic_method(&MethodKey {
                class: class.to_owned(),
                name: "<init>".to_owned(),
                descriptor: "()V".to_owned(),
            })?;
            let constructor = self
                .methods
                .get(&constructor)
                .ok_or_else(|| vm_error("method-not-found", format!("{class}::<init>()V")))?;
            let outcome = machine.call(constructor, [Value::Reference(Some(instance))], 1)?;
            machine.require_return(&outcome, "constructor")?;
            instance
        };
        machine.scheduler.host_driver_active = true;
        loop {
            // Paused or suspended callbacks can leave the driver returning
            // Idle without executing any bytecode. Cancellation must still
            // reach them; instruction polling alone cannot enforce shutdown.
            if machine.native_context.execution_cancelled() {
                return Err(vm_error(
                    "execution-cancelled",
                    "guest execution was cancelled by the host",
                ));
            }
            machine.pump_host_media()?;
            let mut driver_turn = DriverTurn::Regular;
            let wake_suspended_driver =
                if let Some(suspended) = machine.scheduler.suspended_driver_call.as_mut() {
                    match suspended.wake {
                        SuspendedDriverWake::PollHost => {
                            // A frame produced by a non-returning lifecycle callback
                            // is a cooperative boundary. Give the frontend exactly
                            // one turn to poll and dispatch input before resuming the
                            // saved Java stack.
                            suspended.wake = SuspendedDriverWake::ResumeAfterHostPoll;
                            driver_turn = DriverTurn::HostPoll;
                            false
                        }
                        SuspendedDriverWake::ResumeAfterHostPoll => true,
                        SuspendedDriverWake::MainThreadSignal => {
                            machine.scheduler.notified_threads.contains(&MAIN_THREAD_ID)
                                || machine
                                    .scheduler
                                    .interrupted_threads
                                    .contains(&MAIN_THREAD_ID)
                        }
                    }
                } else {
                    false
                };
            if wake_suspended_driver {
                let Some(SuspendedDriverCall {
                    continuation,
                    operation,
                    run_worker_after,
                    ..
                }) = machine.scheduler.suspended_driver_call.take()
                else {
                    continue;
                };
                let outcome = machine.resume_suspended_call(continuation, 1)?;
                machine.complete_driver_call(outcome, operation, run_worker_after)?;
                continue;
            }
            let (call, run_worker_after, stop_application_activity_before) =
                match next_step(driver_turn)? {
                    DriverStep::SaveCheckpoint { driver_state } => {
                        if driver_turn == DriverTurn::HostPoll
                            && let Some(suspended) =
                                machine.scheduler.suspended_driver_call.as_mut()
                        {
                            suspended.wake = SuspendedDriverWake::PollHost;
                        }
                        let clock = (
                            machine.native_context.monotonic_millis(),
                            machine.native_context.wall_clock_millis(),
                        );
                        let saved = machine.encode_checkpoint(instance).and_then(|bytes| {
                            machine
                                .native_context
                                .save_checkpoint(&bytes, &driver_state, clock)
                        });
                        if let Err(error) = saved {
                            machine.native_context.checkpoint_failed(&error);
                        }
                        continue;
                    }
                    DriverStep::Call(call) => (call, true, false),
                    DriverStep::CallAfterApplicationShutdown(call) => (call, false, true),
                    DriverStep::Tick => {
                        machine.run_one_thread(1)?;
                        continue;
                    }
                    DriverStep::Idle => {
                        if driver_turn == DriverTurn::HostPoll
                            && let Some(suspended) =
                                machine.scheduler.suspended_driver_call.as_mut()
                            && suspended.wake == SuspendedDriverWake::ResumeAfterHostPoll
                        {
                            suspended.wake = SuspendedDriverWake::PollHost;
                        }
                        continue;
                    }
                    DriverStep::Stop => break,
                };
            if stop_application_activity_before {
                machine.terminate_application_threads();
            }
            let key = match &call.target {
                CallTarget::Instance => {
                    machine.resolve_virtual(class, &call.name, &call.descriptor)?
                }
                CallTarget::Static { class } => {
                    machine.initialize_class(class, 1)?;
                    machine.resolve_symbolic_method(&MethodKey {
                        class: class.clone(),
                        name: call.name.clone(),
                        descriptor: call.descriptor.clone(),
                    })?
                }
            };
            let method = self
                .methods
                .get(&key)
                .ok_or_else(|| vm_error("method-not-found", display_key(&key)))?;
            let mut arguments = call.arguments;
            if matches!(call.target, CallTarget::Instance) {
                arguments.insert(0, Value::Reference(Some(instance)));
            }
            let outcome = machine.call(method, &arguments, 1)?;
            machine.complete_driver_call(outcome, call.name, run_worker_after)?;
        }
        machine.terminate_application_threads();
        Ok(machine.finish_execution(None))
    }

    pub(super) fn machine<'program, 'context>(
        &'program self,
        limits: Limits,
        trace: bool,
        context: &'context mut dyn HostServices,
    ) -> Machine<'program, 'context> {
        let heap_limit = limits.max_heap_bytes;
        let profiling = trace || limits.profile_methods;
        let m3g_runtime =
            m3g::Runtime::new_with_graph_depth(limits.m3g_arena, limits.m3g_graph_depth);
        let lcd_width = limits.lcd_width;
        let lcd_height = limits.lcd_height;
        let lcd_pixels = usize::try_from(lcd_width)
            .unwrap_or(usize::MAX)
            .saturating_mul(usize::try_from(lcd_height).unwrap_or(usize::MAX));
        let m3g_graphics = M3gGraphicsState::new(lcd_width, lcd_height, limits.m3g_render);
        let micro3d_runtime = micro3d::Runtime::new_with_render_limits(
            limits.micro3d_objects,
            limits.micro3d_bytes,
            lcd_width,
            lcd_height,
            limits.m3g_render,
        )
        .expect("VM 3D limits and initial target are valid");
        let instruction_pacing_started_millis = context.monotonic_millis();
        let timer_poll_instructions =
            timer_poll_instruction_interval(context.realtime_interpreter_instructions_per_second());
        Machine {
            program: self,
            limits,
            execution: ExecutionState {
                instructions: 0,
                stack_slots: 0,
                frame_slots: 0,
                trace: super::execution_trace::ExecutionTrace::default(),
                tracing: trace,
                profiling,
                caught_exceptions: HashMap::new(),
                method_instructions: HashMap::new(),
                thread_failure_count: 0,
                thread_failures: Vec::new(),
                call_stack: Vec::new(),
                frame_storage_pool: std::rc::Rc::default(),
                counters: ExecutionCounters::default(),
                leaf_cache: super::leaf_cache::LeafCache::default(),
            },
            classes: ClassState::with_program(self),
            heap: HeapState::new(heap_limit),
            scheduler: SchedulerState::new(
                instruction_pacing_started_millis,
                timer_poll_instructions,
            ),
            graphics: GraphicsState {
                present_scratch: Vec::with_capacity(lcd_pixels),
                presented_dimensions: None,
                draw_rgb_scratch: Vec::with_capacity(lcd_pixels),
            },
            device: DeviceState {
                light_levels: [100; 2],
                random_seed_sequence: 1,
                last_vibration_request: VibrationRequest::Stop,
            },
            m3g: M3gState {
                runtime: m3g_runtime,
                graphics3d: None,
                graphics: m3g_graphics,
                metrics: M3gExecutionMetrics::default(),
                render_totals: m3g::RenderStats::default(),
            },
            micro3d: Micro3dState {
                runtime: micro3d_runtime,
                target: None,
                target_scissor: [0; 4],
                command_scissor: [0; 4],
                render_pending: false,
                render_diagnostic_calls: 0,
                render_diagnostics: VecDeque::with_capacity(64),
            },
            jsr239: Jsr239State::default(),
            native_context: context,
        }
    }
}

impl Machine<'_, '_> {
    fn complete_driver_call(
        &mut self,
        outcome: CallOutcome,
        operation: String,
        run_worker_after: bool,
    ) -> Result<(), EmuError> {
        if let CallOutcome::Suspend(continuation) = outcome {
            if self.scheduler.suspended_driver_call.is_some() {
                return Err(vm_error(
                    "nested-driver-suspend",
                    "host callback suspended while another driver call was parked",
                ));
            }
            let wake = if std::mem::take(&mut self.scheduler.driver_host_poll_yield) {
                SuspendedDriverWake::PollHost
            } else {
                SuspendedDriverWake::MainThreadSignal
            };
            self.scheduler.suspended_driver_call = Some(SuspendedDriverCall {
                continuation,
                operation,
                run_worker_after,
                wake,
            });
        } else {
            self.require_return(&outcome, &operation)?;
            if run_worker_after {
                self.run_one_thread(1)?;
            }
        }
        Ok(())
    }

    fn finish_execution(mut self, value: Option<Value>) -> Execution {
        let trace = self.finish_trace();
        let m3g = self.m3g_execution_metrics();
        let micro3d = self.micro3d.runtime.metrics();
        let caught_exception_diagnostics = self.caught_exception_diagnostics();
        Execution {
            value,
            instructions: self.execution.instructions,
            virtual_millis: self.scheduler.virtual_monotonic_millis,
            heap_bytes: self.heap.managed.bytes(),
            heap_objects: self.heap.managed.len(),
            peak_heap_bytes: self.heap.managed.peak_bytes(),
            peak_heap_objects: self.heap.managed.peak_objects(),
            m3g,
            micro3d,
            micro3d_diagnostics: self.micro3d.render_diagnostics.into_iter().collect(),
            caught_exception_diagnostics,
            thread_failure_count: self.execution.thread_failure_count,
            trace,
            thread_failures: self.execution.thread_failures,
        }
    }
}
