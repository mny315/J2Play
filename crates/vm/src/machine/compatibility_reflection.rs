use super::{
    CallOutcome, ClassInitializationOutcome, EmuError, Handle, Machine, Method, MethodKey,
    NativeResume, NoArgConstructor, SuspendedCall, Value, display_key, reference_argument,
    suspended_native_pending_call, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(super) fn intern_java_class(&mut self, name: &str) -> Result<Handle, EmuError> {
        if let Some(handle) = self.classes.objects.get(name) {
            return Ok(*handle);
        }
        let handle = self.allocate_native_instance("java/lang/Class", &[])?;
        self.classes.objects.insert(name.to_owned(), handle);
        Ok(handle)
    }

    pub(super) fn class_new_instance(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let class_object = reference_argument(args, 0)?;
        let class_name = self.class_name_for_handle(class_object)?;
        let definition = self
            .program
            .classes
            .get(&class_name)
            // Array Class objects have no constructor or ordinary class
            // definition. The Class already exists, but cannot be instantiated.
            .ok_or_else(|| vm_error("instantiation-exception", &class_name))?;
        if definition.is_abstract
            || definition.is_interface
            || definition.no_arg_constructor == NoArgConstructor::Missing
        {
            return Err(vm_error("instantiation-exception", &class_name));
        }
        if !definition.is_public || definition.no_arg_constructor == NoArgConstructor::NonPublic {
            return Err(vm_error("illegal-access-exception", &class_name));
        }
        match self.request_class_initialization(&class_name, depth + 1, Some(method))? {
            ClassInitializationOutcome::Ready => {}
            ClassInitializationOutcome::Throw(handle) => {
                return Ok(CallOutcome::Throw(handle));
            }
            ClassInitializationOutcome::Suspend(child) => {
                return Ok(suspended_native_pending_call(
                    method,
                    NativeResume::ClassNewInstanceInitialization { class: class_name },
                    child,
                ));
            }
        }
        self.instantiate_initialized_class(method, &class_name, args, depth)
    }

    pub(super) fn instantiate_initialized_class(
        &mut self,
        method: &Method,
        class_name: &str,
        roots: &[Value],
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let instance = self.allocate_native_instance(class_name, roots)?;
        let constructor_key = MethodKey {
            class: class_name.to_owned(),
            name: "<init>".to_owned(),
            descriptor: "()V".to_owned(),
        };
        let constructor =
            self.program.methods.get(&constructor_key).ok_or_else(|| {
                vm_error("instantiation-exception", display_key(&constructor_key))
            })?;
        match self.call(constructor, [Value::Reference(Some(instance))], depth + 1)? {
            CallOutcome::Return(None) => {
                Ok(CallOutcome::Return(Some(Value::Reference(Some(instance)))))
            }
            CallOutcome::Return(Some(_)) => Err(type_error()),
            CallOutcome::Throw(handle) => Ok(CallOutcome::Throw(handle)),
            CallOutcome::Suspend(child) => Ok(suspended_native_pending_call(
                method,
                NativeResume::ClassNewInstance { instance },
                child,
            )),
        }
    }

    pub(super) fn class_for_name_result(&mut self, class: &str) -> Result<CallOutcome, EmuError> {
        let class = self.intern_java_class(class)?;
        Ok(CallOutcome::Return(Some(Value::Reference(Some(class)))))
    }

    pub(super) fn resume_class_for_name(
        &mut self,
        method: &Method,
        class: String,
        child: Box<SuspendedCall>,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        match self.resume_suspended_call(child, depth + 1)? {
            CallOutcome::Return(None) => self.class_for_name_result(&class),
            CallOutcome::Return(Some(_)) => Err(type_error()),
            CallOutcome::Throw(handle) => Ok(CallOutcome::Throw(handle)),
            CallOutcome::Suspend(child) => Ok(suspended_native_pending_call(
                method,
                NativeResume::ClassForName { class },
                child,
            )),
        }
    }

    pub(super) fn resume_class_new_instance_initialization(
        &mut self,
        method: &Method,
        class: String,
        child: Box<SuspendedCall>,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        match self.resume_suspended_call(child, depth + 1)? {
            CallOutcome::Return(None) => {
                self.instantiate_initialized_class(method, &class, &[], depth)
            }
            CallOutcome::Return(Some(_)) => Err(type_error()),
            CallOutcome::Throw(handle) => Ok(CallOutcome::Throw(handle)),
            CallOutcome::Suspend(child) => Ok(suspended_native_pending_call(
                method,
                NativeResume::ClassNewInstanceInitialization { class },
                child,
            )),
        }
    }

    pub(super) fn resume_class_new_instance(
        &mut self,
        method: &Method,
        instance: Handle,
        child: Box<SuspendedCall>,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        match self.resume_suspended_call(child, depth + 1)? {
            CallOutcome::Return(None) => {
                Ok(CallOutcome::Return(Some(Value::Reference(Some(instance)))))
            }
            CallOutcome::Return(Some(_)) => Err(type_error()),
            CallOutcome::Throw(handle) => Ok(CallOutcome::Throw(handle)),
            CallOutcome::Suspend(child) => Ok(suspended_native_pending_call(
                method,
                NativeResume::ClassNewInstance { instance },
                child,
            )),
        }
    }

    pub(super) fn class_name_for_handle(&self, class_object: Handle) -> Result<String, EmuError> {
        self.classes
            .objects
            .iter()
            .find_map(|(name, handle)| (*handle == class_object).then(|| name.clone()))
            .ok_or_else(|| vm_error("type-mismatch", "receiver is not a Class object"))
    }
}
