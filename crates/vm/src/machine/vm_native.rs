//! VM-owned native dispatch and conversion at the host boundary.

use super::{
    CallOutcome, ClassInitializationOutcome, EmuError, JAVA_RANDOM_MULTIPLIER, Machine,
    MachineNativeContext, Method, NativeResume, SuspendedCall, Value, VibrationRequest,
    class_for_name_target, int_argument, java_error_class, long_argument, native_to_value,
    reference_argument, suspended_native_pending_call, type_error, value_to_native, vm_error,
};

impl Machine<'_, '_> {
    #[inline(never)]
    pub(super) fn call_native_inner(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
        resume: Option<Box<SuspendedCall>>,
    ) -> Result<CallOutcome, EmuError> {
        if let Some(mut resume) = resume {
            let native_resume = resume.native_resume.take().ok_or_else(|| {
                vm_error(
                    "invalid-native-continuation",
                    "suspended native call has no resume operation",
                )
            })?;
            return match native_resume {
                NativeResume::Sleep => self.resume_thread_sleep(),
                NativeResume::FramePresentation { pixels, deadline } => {
                    self.resume_frame_presentation(method, pixels, deadline)
                }
                NativeResume::Join { target } => self.resume_thread_join(method, target, depth),
                NativeResume::MonitorWait {
                    object,
                    monitor_depth,
                } => self.resume_monitor_wait(method, object, monitor_depth, depth),
                NativeResume::ClassNewInstance { instance } => {
                    let pending = resume.pending.take().ok_or_else(|| {
                        vm_error(
                            "invalid-native-continuation",
                            "Class.newInstance continuation has no constructor",
                        )
                    })?;
                    self.resume_class_new_instance(method, instance, pending.child, depth)
                }
                NativeResume::ClassForName { class } => {
                    let pending = resume.pending.take().ok_or_else(|| {
                        vm_error(
                            "invalid-native-continuation",
                            "Class.forName continuation has no class initialization",
                        )
                    })?;
                    self.resume_class_for_name(method, class, pending.child, depth)
                }
                NativeResume::ClassNewInstanceInitialization { class } => {
                    let pending = resume.pending.take().ok_or_else(|| {
                        vm_error(
                            "invalid-native-continuation",
                            "Class.newInstance continuation has no class initialization",
                        )
                    })?;
                    self.resume_class_new_instance_initialization(
                        method,
                        class,
                        pending.child,
                        depth,
                    )
                }
                NativeResume::DataInputReadUtf {
                    input,
                    length,
                    bytes,
                } => {
                    let pending = resume.pending.take().ok_or_else(|| {
                        vm_error(
                            "invalid-native-continuation",
                            "DataInput.readUTF continuation has no pending read",
                        )
                    })?;
                    self.resume_data_input_read_utf(
                        method,
                        input,
                        length,
                        bytes,
                        pending.child,
                        depth,
                    )
                }
                NativeResume::InputStreamReaderClose { reader } => {
                    let pending = resume.pending.take().ok_or_else(|| {
                        vm_error(
                            "invalid-native-continuation",
                            "InputStreamReader.close continuation has no pending close",
                        )
                    })?;
                    let outcome = self.resume_suspended_call(pending.child, depth + 1)?;
                    self.finish_input_stream_reader_close(method, reader, outcome)
                }
                NativeResume::InputStreamReaderRead(state) => {
                    let pending = resume.pending.take().ok_or_else(|| {
                        vm_error(
                            "invalid-native-continuation",
                            "InputStreamReader continuation has no pending read",
                        )
                    })?;
                    self.resume_input_stream_reader_read(method, state, pending.child, depth)
                }
            };
        }
        self.execution.counters.native_calls =
            self.execution.counters.native_calls.saturating_add(1);
        if let Some(outcome) = self.invoke_vm_native(method, args, depth)? {
            return Ok(outcome);
        }
        let mut small_arguments = [const { super::NativeValue::Int(0) }; 8];
        let large_arguments;
        let arguments = if args.len() <= small_arguments.len() {
            for (target, &value) in small_arguments.iter_mut().zip(args) {
                *target = value_to_native(value);
            }
            &small_arguments[..args.len()]
        } else {
            large_arguments = args
                .iter()
                .copied()
                .map(value_to_native)
                .collect::<Vec<_>>();
            &large_arguments
        };
        // Borrow arguments and runtime states; only GC needs a root snapshot.
        let mut context = MachineNativeContext {
            host: self.native_context,
            heap: &mut self.heap,
            m3g: &mut self.m3g,
            micro3d: &mut self.micro3d,
            jsr239: &self.jsr239,
            classes: &mut self.classes,
            program: self.program,
            execution: &self.execution,
            scheduler: &mut self.scheduler,
            arguments: args,
        };
        let invoked = self
            .program
            .natives
            .invoke(&method.key, &mut context, arguments);
        let result = match invoked {
            Ok(result) => result,
            Err(error) => {
                let class = java_error_class(&error);
                if let Some(class) = class
                    && self.program.classes.contains_key(class)
                {
                    let handle = self.allocate_error_exception(class, &error, &[], args)?;
                    self.record_exception_frame(handle, method, 0, &[], args)?;
                    return Ok(CallOutcome::Throw(handle));
                }
                return Err(error);
            }
        };
        let value = result.as_ref().map(native_to_value);
        match (value, method.descriptor.returns) {
            (Some(value), Some(kind)) if value.kind() == kind => {
                Ok(CallOutcome::Return(Some(value)))
            }
            (None, None) => Ok(CallOutcome::Return(None)),
            _ => Err(type_error()),
        }
    }

    pub(super) fn request_device_vibration(&mut self, request: VibrationRequest) -> bool {
        self.device.last_vibration_request = request;
        let accepted = self.native_context.request_vibration(request);
        if !accepted && self.native_context.vm_flight_recorder_enabled() {
            self.native_context.record_vm_flight(format!(
                "vibration-request request={request:?} accepted=false"
            ));
        }
        accepted
    }

    pub(super) fn require_return(
        &self,
        outcome: &CallOutcome,
        operation: &str,
    ) -> Result<(), EmuError> {
        match outcome {
            CallOutcome::Return(None) => Ok(()),
            CallOutcome::Return(Some(_)) => Err(vm_error(
                "invalid-lifecycle-return",
                format!("{operation} returned a value"),
            )),
            CallOutcome::Throw(handle) => Err(self.throwable_error(
                *handle,
                if self.heap.managed_heap_limit_throwables.contains(handle) {
                    super::MANAGED_HEAP_LIMIT_CODE
                } else {
                    "uncaught-exception"
                },
                operation,
            )?),
            CallOutcome::Suspend(_) => Err(vm_error(
                "invalid-lifecycle-suspend",
                format!("{operation} suspended outside the scheduler"),
            )),
        }
    }

    pub(super) fn invoke_vm_native(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
    ) -> Result<Option<CallOutcome>, EmuError> {
        if method.key.class.starts_with("javax/microedition/m3g/") {
            return self.invoke_m3g_native(method, args, depth).map(Some);
        }
        if method
            .key
            .class
            .starts_with("com/mascotcapsule/micro3d/v3/")
        {
            return self.invoke_micro3d_native(method, args).map(Some);
        }
        if method.key.class.starts_with("java/nio/") {
            return self.invoke_nio_native(method, args).map(Some);
        }
        if method.key.class.starts_with("javax/microedition/khronos/") {
            return self.invoke_jsr239_native(method, args).map(Some);
        }
        if matches!(
            method.key.class.as_str(),
            "java/lang/Thread" | "java/lang/Object"
        ) {
            return self.invoke_thread_native(method, args, depth);
        }
        if matches!(
            method.key.class.as_str(),
            "com/nokia/mid/ui/DirectUtils"
                | "javax/microedition/lcdui/Image"
                | "javax/microedition/lcdui/Graphics"
                | "javax/microedition/lcdui/Canvas"
                | "javax/microedition/lcdui/Screen"
                | "javax/microedition/lcdui/game/Sprite"
        ) {
            return self.invoke_graphics_native(method, args);
        }
        let signature = (
            method.key.class.as_str(),
            method.key.name.as_str(),
            method.key.descriptor.as_str(),
        );
        let outcome = match signature {
            ("com/nokia/mid/ui/DeviceControl", "setLights", "(II)V") => {
                let device = int_argument(args, 0)?;
                let level = int_argument(args, 1)?;
                if !(0..=1).contains(&device) || !(0..=100).contains(&level) {
                    self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid DeviceControl light or level"),
                    )?
                } else {
                    self.device.light_levels[usize::try_from(device).unwrap_or_default()] = level;
                    CallOutcome::Return(None)
                }
            }
            ("com/siemens/mp/game/Light", "setLightOn", "()V") => {
                self.device.light_levels[0] = 100;
                CallOutcome::Return(None)
            }
            ("com/siemens/mp/game/Light", "setLightOff", "()V") => {
                self.device.light_levels[0] = 0;
                CallOutcome::Return(None)
            }
            ("com/siemens/mp/game/Vibrator", "startVibrator", "()V") => {
                self.request_device_vibration(VibrationRequest::Continuous { level: None });
                CallOutcome::Return(None)
            }
            ("com/siemens/mp/game/Vibrator", "stopVibrator", "()V")
            | ("com/nokia/mid/ui/DeviceControl", "stopVibra", "()V") => {
                self.request_device_vibration(VibrationRequest::Stop);
                CallOutcome::Return(None)
            }
            ("com/siemens/mp/game/Vibrator", "triggerVibrator", "(I)V") => {
                let duration = int_argument(args, 0)?;
                let request = if duration <= 0 {
                    VibrationRequest::Stop
                } else {
                    VibrationRequest::Timed {
                        duration_millis: duration as u64,
                        level: None,
                    }
                };
                self.request_device_vibration(request);
                CallOutcome::Return(None)
            }
            ("javax/microedition/lcdui/Display", "vibrate", "(I)Z") => {
                let duration = int_argument(args, 1)?;
                if duration < 0 {
                    self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("negative vibration duration"),
                    )?
                } else {
                    let request = if duration == 0 {
                        VibrationRequest::Stop
                    } else {
                        VibrationRequest::Timed {
                            duration_millis: duration as u64,
                            level: None,
                        }
                    };
                    let accepted = self.request_device_vibration(request);
                    CallOutcome::Return(Some(Value::Int(i32::from(accepted))))
                }
            }
            ("com/nokia/mid/ui/DeviceControl", "startVibra", "(IJ)V") => {
                let frequency = int_argument(args, 0)?;
                let duration = long_argument(args, 1)?;
                if !(0..=100).contains(&frequency) || duration < 0 {
                    self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid DeviceControl vibration frequency or duration"),
                    )?
                } else {
                    let request = if frequency == 0 || duration == 0 {
                        VibrationRequest::Stop
                    } else {
                        VibrationRequest::Timed {
                            duration_millis: duration as u64,
                            level: Some(frequency as u8),
                        }
                    };
                    self.request_device_vibration(request);
                    CallOutcome::Return(None)
                }
            }
            ("com/nokia/mid/ui/DeviceControl", "flashLights", "(J)V") => {
                if long_argument(args, 0)? < 0 {
                    self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("negative DeviceControl light duration"),
                    )?
                } else {
                    CallOutcome::Return(None)
                }
            }
            // The Java contract only requests a collection. Deferring it is
            // valid, and allocation-pressure GC has the complete root set.
            ("java/lang/System", "gc", "()V") => CallOutcome::Return(None),
            ("java/lang/System", "arraycopy", "(Ljava/lang/Object;ILjava/lang/Object;II)V") => {
                self.system_arraycopy(args)?;
                CallOutcome::Return(None)
            }
            ("java/lang/System", "identityHashCode", "(Ljava/lang/Object;)I") => {
                let hash = match args.first() {
                    Some(Value::Reference(Some(handle))) => {
                        let raw = handle.to_raw();
                        (raw ^ (raw >> 32)) as i32
                    }
                    Some(Value::Reference(None)) => 0,
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Int(hash)))
            }
            ("java/lang/Runtime", "freeMemory", "()J") => {
                let free = self
                    .limits
                    .max_heap_bytes
                    .saturating_sub(self.heap.managed.bytes());
                CallOutcome::Return(Some(Value::Long(i64::try_from(free).unwrap_or(i64::MAX))))
            }
            ("java/lang/Runtime", "totalMemory", "()J") => CallOutcome::Return(Some(Value::Long(
                i64::try_from(self.limits.max_heap_bytes).unwrap_or(i64::MAX),
            ))),
            ("java/lang/Class", "forName", "(Ljava/lang/String;)Ljava/lang/Class;") => {
                let name = reference_argument(args, 0)?;
                let name = self
                    .heap
                    .string_values
                    .get(&name)
                    .ok_or_else(|| vm_error("type-mismatch", "Class.forName expects String"))?;
                let name = String::from_utf16(name)
                    .map_err(|_| vm_error("class-for-name", "invalid UTF-16 class name"))?;
                let name = class_for_name_target(&name)
                    .filter(|candidate| self.program.classes.contains_key(candidate))
                    .ok_or_else(|| vm_error("class-for-name", name))?;
                match self.request_class_initialization(&name, depth + 1, Some(method))? {
                    ClassInitializationOutcome::Ready => self.class_for_name_result(&name)?,
                    ClassInitializationOutcome::Throw(handle) => CallOutcome::Throw(handle),
                    ClassInitializationOutcome::Suspend(child) => suspended_native_pending_call(
                        method,
                        NativeResume::ClassForName { class: name },
                        child,
                    ),
                }
            }
            ("java/lang/Class", "newInstance", "()Ljava/lang/Object;") => {
                self.class_new_instance(method, args, depth)?
            }
            ("java/lang/Class", "isAssignableFrom", "(Ljava/lang/Class;)Z") => {
                let target = self.class_name_for_handle(reference_argument(args, 0)?)?;
                let candidate = self.class_name_for_handle(reference_argument(args, 1)?)?;
                CallOutcome::Return(Some(Value::Int(i32::from(
                    self.is_instance(&candidate, &target),
                ))))
            }
            ("java/lang/Class", "isArray", "()Z") => {
                let class = self.class_name_for_handle(reference_argument(args, 0)?)?;
                CallOutcome::Return(Some(Value::Int(i32::from(class.starts_with('[')))))
            }
            ("java/lang/Class", "isInterface", "()Z") => {
                let class = self.class_name_for_handle(reference_argument(args, 0)?)?;
                let is_interface = self
                    .program
                    .classes
                    .get(&class)
                    .is_some_and(|class| class.is_interface);
                CallOutcome::Return(Some(Value::Int(i32::from(is_interface))))
            }
            ("java/lang/Class", "toString", "()Ljava/lang/String;") => {
                let class = self.class_name_for_handle(reference_argument(args, 0)?)?;
                let is_interface = self
                    .program
                    .classes
                    .get(&class)
                    .is_some_and(|class| class.is_interface);
                let kind = if is_interface { "interface" } else { "class" };
                let text = format!("{kind} {}", class.replace('/', "."));
                CallOutcome::Return(Some(Value::Reference(Some(self.allocate_dynamic_string(
                    &text,
                    &[],
                    args,
                )?))))
            }
            ("java/lang/Class", "isInstance", "(Ljava/lang/Object;)Z") => {
                let target = self.class_name_for_handle(reference_argument(args, 0)?)?;
                let result = match args.get(1) {
                    Some(Value::Reference(Some(object))) => {
                        self.is_instance(&self.object_class(*object)?, &target)
                    }
                    Some(Value::Reference(None)) => false,
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Int(i32::from(result))))
            }
            ("java/lang/ref/Reference", "<init>", "(Ljava/lang/Object;)V") => {
                let reference = reference_argument(args, 0)?;
                let referent = match args.get(1) {
                    Some(Value::Reference(referent)) => *referent,
                    _ => return Err(type_error()),
                };
                self.heap.weak_references.insert(reference, referent);
                CallOutcome::Return(None)
            }
            ("java/lang/ref/Reference", "get", "()Ljava/lang/Object;") => {
                let reference = reference_argument(args, 0)?;
                let referent = self
                    .heap
                    .weak_references
                    .get(&reference)
                    .copied()
                    .flatten()
                    .filter(|referent| self.heap.managed.get(*referent).is_ok());
                CallOutcome::Return(Some(Value::Reference(referent)))
            }
            ("java/lang/ref/Reference", "clear", "()V") => {
                let reference = reference_argument(args, 0)?;
                self.heap.weak_references.insert(reference, None);
                CallOutcome::Return(None)
            }
            ("java/lang/String", "<init>", "(Ljava/lang/StringBuffer;)V") => {
                self.string_from_buffer(args)?;
                CallOutcome::Return(None)
            }
            ("java/lang/String", "regionMatches", "(ZILjava/lang/String;II)Z") => {
                CallOutcome::Return(Some(Value::Int(i32::from(
                    self.string_region_matches(args)?,
                ))))
            }
            ("java/lang/String", "intern", "()Ljava/lang/String;") => {
                let string = reference_argument(args, 0)?;
                CallOutcome::Return(Some(Value::Reference(Some(
                    self.intern_existing_string(string, args)?,
                ))))
            }
            ("java/io/DataInputStream", "readUTF", "(Ljava/io/DataInput;)Ljava/lang/String;") => {
                self.data_input_read_utf(method, args, depth)?
            }
            ("java/io/ByteArrayOutputStream", "write", "([BII)V") => {
                self.byte_array_output_write_slice(args)?;
                CallOutcome::Return(None)
            }
            ("java/util/Random", "__initialSeed", "()J") => {
                let sequence = self.device.random_seed_sequence;
                self.device.random_seed_sequence = sequence.wrapping_add(1);
                let seed = self.native_context.wall_clock_millis()
                    ^ sequence.wrapping_mul(0x9e37_79b9_7f4a_7c15).cast_signed();
                // The override describes the internal 48-bit state. Undo
                // setSeed's xor here; the normal constructor applies it once.
                let seed = self
                    .native_context
                    .random_seed_override()
                    .map_or(seed, |seed| seed ^ JAVA_RANDOM_MULTIPLIER);
                CallOutcome::Return(Some(Value::Long(seed)))
            }
            ("java/io/InputStreamReader", "<init>", "(Ljava/io/InputStream;)V") => {
                self.input_stream_reader_init(args, None)?;
                CallOutcome::Return(None)
            }
            (
                "java/io/InputStreamReader",
                "<init>",
                "(Ljava/io/InputStream;Ljava/lang/String;)V",
            ) => {
                self.input_stream_reader_init(args, Some(2))?;
                CallOutcome::Return(None)
            }
            ("java/io/InputStreamReader", "read", "()I" | "([CII)I") => {
                self.input_stream_reader_read(method, args, depth)?
            }
            ("java/io/InputStreamReader", "close", "()V") => {
                self.input_stream_reader_close(method, args, depth)?
            }
            ("java/lang/StringBuffer", "delete", "(II)Ljava/lang/StringBuffer;") => {
                let receiver = reference_argument(args, 0)?;
                self.string_buffer_delete(
                    receiver,
                    int_argument(args, 1)?,
                    int_argument(args, 2)?,
                    false,
                )?;
                CallOutcome::Return(Some(Value::Reference(Some(receiver))))
            }
            ("java/lang/StringBuffer", "deleteCharAt", "(I)Ljava/lang/StringBuffer;") => {
                let receiver = reference_argument(args, 0)?;
                let index = int_argument(args, 1)?;
                self.string_buffer_delete(receiver, index, index.saturating_add(1), true)?;
                CallOutcome::Return(Some(Value::Reference(Some(receiver))))
            }
            _ => return Ok(None),
        };
        Ok(Some(outcome))
    }
}
