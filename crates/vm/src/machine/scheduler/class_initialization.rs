use super::{
    CallOutcome, Class, ClassInitializationOutcome, ClassInitializationPhase, EmuError, Handle,
    Machine, Method, MethodKey, SuspendedCall, SuspendedClassInitialization, ThreadState, Value,
    suspended_class_initialization, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn resume_class_initialization(
        &mut self,
        suspension_method: &Method,
        initialization: SuspendedClassInitialization,
        child: Option<Box<SuspendedCall>>,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let SuspendedClassInitialization { class, phase } = initialization;
        if phase == ClassInitializationPhase::Waiting {
            if child.is_some() {
                return Err(vm_error(
                    "invalid-class-initializer-continuation",
                    "a class waiter unexpectedly owns a child continuation",
                ));
            }
            return match self.request_class_initialization(
                &class,
                depth + 1,
                Some(suspension_method),
            ) {
                Ok(ClassInitializationOutcome::Ready) => Ok(CallOutcome::Return(None)),
                Ok(ClassInitializationOutcome::Suspend(call)) => Ok(CallOutcome::Suspend(call)),
                Ok(ClassInitializationOutcome::Throw(handle)) => Ok(CallOutcome::Throw(handle)),
                Err(error) => self.error_as_call_outcome(error, &[]),
            };
        }

        let child = child.ok_or_else(|| {
            vm_error(
                "invalid-class-initializer-continuation",
                "a suspended class initialization has no child continuation",
            )
        })?;
        let outcome = match self.resume_suspended_call(child, depth + 1) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.fail_class_initialization(&class, &error);
                return self.error_as_call_outcome(error, &[]);
            }
        };
        match outcome {
            CallOutcome::Suspend(child) => Ok(
                match suspended_class_initialization(suspension_method, class, phase, Some(child)) {
                    ClassInitializationOutcome::Suspend(call) => CallOutcome::Suspend(call),
                    ClassInitializationOutcome::Ready | ClassInitializationOutcome::Throw(_) => {
                        unreachable!()
                    }
                },
            ),
            CallOutcome::Return(Some(_)) => {
                let error = type_error();
                self.fail_class_initialization(&class, &error);
                self.error_as_call_outcome(error, &[])
            }
            CallOutcome::Throw(handle) => {
                if phase == ClassInitializationPhase::AfterInitializer {
                    match self.fail_initializer_from_throwable(&class, handle) {
                        Ok(ClassInitializationOutcome::Throw(handle)) => {
                            Ok(CallOutcome::Throw(handle))
                        }
                        Ok(
                            ClassInitializationOutcome::Ready
                            | ClassInitializationOutcome::Suspend(_),
                        ) => unreachable!(),
                        Err(error) => {
                            self.error_as_call_outcome(error, &[Value::Reference(Some(handle))])
                        }
                    }
                } else {
                    self.fail_dependent_class_from_throwable(&class, handle);
                    Ok(CallOutcome::Throw(handle))
                }
            }
            CallOutcome::Return(None) => {
                if phase == ClassInitializationPhase::AfterInitializer {
                    self.complete_class_initialization(&class);
                    return Ok(CallOutcome::Return(None));
                }
                match self.continue_class_initialization(&class, depth + 1, Some(suspension_method))
                {
                    Ok(ClassInitializationOutcome::Ready) => Ok(CallOutcome::Return(None)),
                    Ok(ClassInitializationOutcome::Suspend(call)) => Ok(CallOutcome::Suspend(call)),
                    Ok(ClassInitializationOutcome::Throw(handle)) => Ok(CallOutcome::Throw(handle)),
                    Err(error) => self.error_as_call_outcome(error, &[]),
                }
            }
        }
    }

    pub(in crate::machine) fn initialize_class_from_frame(
        &mut self,
        name: &str,
        method: &Method,
        depth: usize,
        locals: &[Option<Value>],
        stack: &[Value],
    ) -> Result<ClassInitializationOutcome, EmuError> {
        if self.classes.initialized.contains(name)
            || self.classes.initializing.get(name) == Some(&self.scheduler.current_thread)
        {
            return Ok(ClassInitializationOutcome::Ready);
        }
        self.publish_frame_roots(depth, locals, stack, &[]);
        let result = self.request_class_initialization(name, depth + 1, Some(method));
        self.heap.frame_roots.remove(depth);
        result
    }

    pub(in crate::machine) fn initialize_class(
        &mut self,
        name: &str,
        depth: usize,
    ) -> Result<(), EmuError> {
        match self.request_class_initialization(name, depth, None)? {
            ClassInitializationOutcome::Ready => Ok(()),
            ClassInitializationOutcome::Suspend(_) => Err(vm_error(
                "initializer-suspend",
                "a blocking class initialization unexpectedly suspended",
            )),
            ClassInitializationOutcome::Throw(handle) => self.require_return(
                &CallOutcome::Throw(handle),
                &format!("class {name} initialization"),
            ),
        }
    }

    pub(in crate::machine) fn request_class_initialization(
        &mut self,
        name: &str,
        depth: usize,
        suspension_method: Option<&Method>,
    ) -> Result<ClassInitializationOutcome, EmuError> {
        if self.classes.initialized.contains(name) {
            return Ok(ClassInitializationOutcome::Ready);
        }
        // Parent initialization recurses even when no class has a <clinit>.
        // Bound that path before it can exhaust the host stack independently
        // of the ordinary method-call guard.
        if depth > self.limits.max_frames {
            return Err(vm_error(
                "stack-overflow",
                format!(
                    "class initialization exceeds frame limit {}",
                    self.limits.max_frames
                ),
            ));
        }
        if let Some(cause) = self.classes.failed_initialization.get(name) {
            return Err(vm_error(
                "class-not-found",
                format!("{name} initialization failed: {cause}"),
            ));
        }
        if let Some(owner) = self.classes.initializing.get(name).copied() {
            if owner == self.scheduler.current_thread {
                return Ok(ClassInitializationOutcome::Ready);
            }
            if self.is_scheduler_worker() {
                let method = suspension_method.ok_or_else(|| {
                    vm_error(
                        "initializer-suspend",
                        "worker class initialization has no resumable call site",
                    )
                })?;
                let waiter = Handle::from_raw(self.scheduler.current_thread);
                self.scheduler
                    .thread_states
                    .insert(waiter, ThreadState::Sleeping);
                let waiters = self
                    .classes
                    .initialization_waiters
                    .entry(name.to_owned())
                    .or_default();
                if !waiters.contains(&waiter) {
                    waiters.push(waiter);
                }
                return Ok(suspended_class_initialization(
                    method,
                    name.to_owned(),
                    ClassInitializationPhase::Waiting,
                    None,
                ));
            }
            while self.classes.initializing.contains_key(name) {
                if !self.run_one_thread(depth + 1)? {
                    return Err(vm_error(
                        "deadlock",
                        format!("class {name} is owned by blocked thread {owner}"),
                    ));
                }
            }
            return self.request_class_initialization(name, depth + 1, suspension_method);
        }
        let Some(class) = self.program.classes.get(name) else {
            if name == "java/lang/Object" {
                self.classes.initialized.insert(name.to_owned());
                return Ok(ClassInitializationOutcome::Ready);
            }
            return Err(vm_error("class-not-found", name));
        };
        self.classes
            .initializing
            .insert(name.to_owned(), self.scheduler.current_thread);
        if let Err(error) = self.prepare_class_static_fields(class) {
            self.fail_class_initialization(name, &error);
            return Err(error);
        }
        if let Some(parent) = class.super_name.as_deref() {
            match self.request_class_initialization(parent, depth + 1, suspension_method) {
                Ok(ClassInitializationOutcome::Ready) => {}
                Ok(ClassInitializationOutcome::Suspend(child)) => {
                    let Some(method) = suspension_method else {
                        let error = vm_error(
                            "initializer-suspend",
                            "class parent suspended without a resumable call site",
                        );
                        self.fail_class_initialization(name, &error);
                        return Err(error);
                    };
                    return Ok(suspended_class_initialization(
                        method,
                        name.to_owned(),
                        ClassInitializationPhase::AfterParent,
                        Some(child),
                    ));
                }
                Ok(ClassInitializationOutcome::Throw(handle)) => {
                    self.fail_dependent_class_from_throwable(name, handle);
                    return Ok(ClassInitializationOutcome::Throw(handle));
                }
                Err(error) => {
                    self.fail_class_initialization(name, &error);
                    return Err(error);
                }
            }
        }
        self.continue_class_initialization(name, depth, suspension_method)
    }

    pub(in crate::machine) fn prepare_class_static_fields(
        &mut self,
        class: &Class,
    ) -> Result<(), EmuError> {
        // Active-use callers publish their frame before entering the request;
        // `intern_string` therefore sees those handles through `frame_roots`
        // even though preparation itself has no synthetic operand stack.
        for field in &class.fields {
            if field.is_static {
                let initial = if let Some(units) = field.constant_string.as_deref() {
                    Value::Reference(Some(self.intern_string_units(units.to_vec(), &[], &[])?))
                } else {
                    field.initial
                };
                if self.classes.static_fields.get(&field.key).is_none() {
                    self.classes
                        .static_fields
                        .insert(field.key.to_string(), initial);
                }
            }
        }
        Ok(())
    }

    pub(in crate::machine) fn continue_class_initialization(
        &mut self,
        name: &str,
        depth: usize,
        suspension_method: Option<&Method>,
    ) -> Result<ClassInitializationOutcome, EmuError> {
        let result = (|| {
            let key = MethodKey {
                class: name.to_owned(),
                name: "<clinit>".into(),
                descriptor: "()V".into(),
            };
            let Some(initializer) = self.program.methods.get(&key) else {
                self.complete_class_initialization(name);
                return Ok(ClassInitializationOutcome::Ready);
            };
            match self.call(initializer, [], depth + 1)? {
                CallOutcome::Return(None) => {
                    self.complete_class_initialization(name);
                    Ok(ClassInitializationOutcome::Ready)
                }
                CallOutcome::Return(Some(_)) => Err(type_error()),
                CallOutcome::Throw(handle) => self.fail_initializer_from_throwable(name, handle),
                CallOutcome::Suspend(child) => {
                    let method = suspension_method.ok_or_else(|| {
                        vm_error(
                            "initializer-suspend",
                            "class initializer suspended without a resumable call site",
                        )
                    })?;
                    Ok(suspended_class_initialization(
                        method,
                        name.to_owned(),
                        ClassInitializationPhase::AfterInitializer,
                        Some(child),
                    ))
                }
            }
        })();
        if let Err(error) = &result
            && self.classes.initializing.contains_key(name)
        {
            self.fail_class_initialization(name, error);
        }
        result
    }

    pub(in crate::machine) fn fail_initializer_from_throwable(
        &mut self,
        name: &str,
        original: Handle,
    ) -> Result<ClassInitializationOutcome, EmuError> {
        let actual = match self.object_class(original) {
            Ok(actual) => actual,
            Err(error) => {
                self.fail_class_initialization(name, &error);
                return Err(error);
            }
        };
        let diagnostic = match self.exception_in_initializer_error(name, original) {
            Ok(diagnostic) => diagnostic,
            Err(error) => {
                self.fail_class_initialization(name, &error);
                return Err(error);
            }
        };
        let throwable = if self.is_instance(&actual, "java/lang/Error") {
            Ok(original)
        } else {
            let roots = [Value::Reference(Some(original))];
            self.allocate_exception(
                "java/lang/ExceptionInInitializerError",
                Some(diagnostic.message()),
                &[],
                &roots,
            )
        };
        // The class becomes erroneous even if allocating the mandated wrapper
        // itself fails. The original throwable stays rooted for that attempt.
        self.fail_class_initialization(name, &diagnostic);
        throwable.map(ClassInitializationOutcome::Throw)
    }

    pub(in crate::machine) fn fail_dependent_class_from_throwable(
        &mut self,
        name: &str,
        handle: Handle,
    ) {
        let diagnostic = match self.exception_in_initializer_error(name, handle) {
            Ok(diagnostic) | Err(diagnostic) => diagnostic,
        };
        self.fail_class_initialization(name, &diagnostic);
    }

    pub(in crate::machine) fn exception_in_initializer_error(
        &self,
        name: &str,
        handle: Handle,
    ) -> Result<EmuError, EmuError> {
        self.throwable_error(handle, "exception-in-initializer", name)
    }

    pub(in crate::machine) fn complete_class_initialization(&mut self, name: &str) {
        self.classes.initializing.remove(name);
        self.classes.initialized.insert(name.to_owned());
        self.wake_class_initialization_waiters(name);
    }

    pub(in crate::machine) fn fail_class_initialization(&mut self, name: &str, error: &EmuError) {
        self.classes.initializing.remove(name);
        self.classes.failed_initialization.insert(
            name.to_owned(),
            format!("{}: {}", error.code(), error.message()),
        );
        self.wake_class_initialization_waiters(name);
    }

    pub(in crate::machine) fn wake_class_initialization_waiters(&mut self, name: &str) {
        let Some(waiters) = self.classes.initialization_waiters.remove(name) else {
            return;
        };
        for waiter in waiters {
            if self.scheduler.thread_states.get(&waiter) == Some(&ThreadState::Sleeping) {
                self.scheduler
                    .thread_states
                    .insert(waiter, ThreadState::Runnable);
                self.scheduler.runnable_threads.push_back(waiter);
            }
        }
    }
}
