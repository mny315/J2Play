//! Native access to managed Java objects, arrays, strings and continuations.

use super::super::{
    Allocation, Arc, ArrayKind, EmuError, Handle, HeapValue, JavaStackFrame, MachineNativeContext,
    ScheduledJavaTask, SchedulerState, VmAccess, array_descriptor, heap_error, type_error,
    vm_error,
};

impl VmAccess for MachineNativeContext<'_> {
    fn native_caller_class(&self) -> Option<String> {
        let mut frames = self.execution.call_stack.iter().rev();
        let native_class = &self.program.active_stack_key(frames.next()?).class;
        frames.find_map(|frame| {
            let class = &self.program.active_stack_key(frame).class;
            (class != native_class).then(|| class.clone())
        })
    }

    fn read_java_string(&self, reference: u64) -> Result<String, EmuError> {
        Ok(String::from_utf16_lossy(self.string_units(reference)?))
    }
    fn read_java_utf16(&self, reference: u64) -> Result<&[u16], EmuError> {
        self.string_units(reference)
    }
    fn write_java_utf16(&mut self, reference: u64, value: Box<[u16]>) -> Result<(), EmuError> {
        let handle = Handle::from_raw(reference);
        let allocation = self.heap.managed.get(handle).map_err(heap_error)?;
        if !matches!(allocation, Allocation::Object { class, .. } if class.as_ref() == "java/lang/String")
        {
            return Err(vm_error(
                "type-mismatch",
                "native reference is not a java/lang/String",
            ));
        }
        self.store_string_units(handle, value.into_vec())
    }
    fn intern_java_string(&mut self, value: &str) -> Result<u64, EmuError> {
        self.allocate_native_string(value.encode_utf16().collect())
    }
    fn read_java_char_array(&self, reference: u64) -> Result<Vec<u16>, EmuError> {
        let allocation = self
            .heap
            .managed
            .get(Handle::from_raw(reference))
            .map_err(heap_error)?;
        let Allocation::Array {
            kind: ArrayKind::Char,
            elements,
        } = allocation
        else {
            return Err(vm_error(
                "type-mismatch",
                "native reference is not a char[]",
            ));
        };
        elements
            .iter()
            .map(|value| match value {
                HeapValue::Int(value) => Ok(*value as u16),
                _ => Err(vm_error("type-mismatch", "char[] contains a non-int value")),
            })
            .collect()
    }
    fn read_java_char_array_range(
        &self,
        reference: u64,
        offset: i32,
        length: i32,
    ) -> Result<Option<Vec<u16>>, EmuError> {
        let allocation = self
            .heap
            .managed
            .get(Handle::from_raw(reference))
            .map_err(heap_error)?;
        let Allocation::Array {
            kind: ArrayKind::Char,
            elements,
        } = allocation
        else {
            return Err(vm_error(
                "type-mismatch",
                "native reference is not a char[]",
            ));
        };
        let (Ok(start), Ok(length)) = (usize::try_from(offset), usize::try_from(length)) else {
            return Ok(None);
        };
        let Some(range) = start
            .checked_add(length)
            .and_then(|end| elements.get(start..end))
        else {
            return Ok(None);
        };
        range
            .iter()
            .map(|value| match value {
                HeapValue::Int(value) => Ok(*value as u16),
                _ => Err(vm_error("type-mismatch", "char[] contains a non-int value")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
    fn read_java_int_array(&self, reference: u64) -> Result<Vec<i32>, EmuError> {
        let allocation = self
            .heap
            .managed
            .get(Handle::from_raw(reference))
            .map_err(heap_error)?;
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = allocation
        else {
            return Err(vm_error(
                "type-mismatch",
                "native reference is not an int[]",
            ));
        };
        elements
            .iter()
            .map(|value| match value {
                HeapValue::Int(value) => Ok(*value),
                _ => Err(vm_error("type-mismatch", "int[] contains a non-int value")),
            })
            .collect()
    }
    fn read_java_byte_array(&self, reference: u64) -> Result<Vec<u8>, EmuError> {
        let allocation = self
            .heap
            .managed
            .get(Handle::from_raw(reference))
            .map_err(heap_error)?;
        let Allocation::Array {
            kind: ArrayKind::Byte,
            elements,
        } = allocation
        else {
            return Err(vm_error(
                "type-mismatch",
                "native reference is not a byte[]",
            ));
        };
        let mut bytes = Vec::with_capacity(elements.len());
        for value in elements {
            let HeapValue::Int(value) = value else {
                return Err(vm_error("type-mismatch", "byte[] contains a non-int value"));
            };
            bytes.push(*value as u8);
        }
        Ok(bytes)
    }
    fn read_java_byte_array_range(
        &self,
        reference: u64,
        offset: i32,
        length: i32,
    ) -> Result<Option<Vec<u8>>, EmuError> {
        let allocation = self
            .heap
            .managed
            .get(Handle::from_raw(reference))
            .map_err(heap_error)?;
        let Allocation::Array {
            kind: ArrayKind::Byte,
            elements,
        } = allocation
        else {
            return Err(vm_error(
                "type-mismatch",
                "native reference is not a byte[]",
            ));
        };
        let (Ok(start), Ok(length)) = (usize::try_from(offset), usize::try_from(length)) else {
            return Ok(None);
        };
        let Some(range) = start
            .checked_add(length)
            .and_then(|end| elements.get(start..end))
        else {
            return Ok(None);
        };
        let mut bytes = Vec::with_capacity(range.len());
        for value in range {
            let HeapValue::Int(value) = value else {
                return Err(vm_error("type-mismatch", "byte[] contains a non-int value"));
            };
            bytes.push(*value as u8);
        }
        Ok(Some(bytes))
    }
    fn allocate_java_int_array(&mut self, value: &[i32]) -> Result<u64, EmuError> {
        let length = i32::try_from(value.len())
            .map_err(|_| vm_error("array-limit", "native int array exceeds i32"))?;
        let (handle, elements) = self.allocate_native_array(&ArrayKind::Int, length)?;
        for (destination, item) in elements.iter_mut().zip(value) {
            *destination = HeapValue::Int(*item);
        }
        Ok(handle.to_raw())
    }
    fn intern_java_utf16(&mut self, value: &[u16]) -> Result<u64, EmuError> {
        self.allocate_native_string(value.to_vec())
    }
    fn read_java_class(&self, reference: u64) -> Result<String, EmuError> {
        let handle = Handle::from_raw(reference);
        self.classes
            .objects
            .iter()
            .find_map(|(name, candidate)| (*candidate == handle).then(|| name.clone()))
            .ok_or_else(|| vm_error("type-mismatch", "native reference is not a Class object"))
    }
    fn intern_java_class(&mut self, name: &str) -> Result<u64, EmuError> {
        if let Some(handle) = self.classes.objects.get(name) {
            return Ok(handle.to_raw());
        }
        let handle = self.allocate_native_object("java/lang/Class")?;
        self.classes.objects.insert(name.to_owned(), handle);
        Ok(handle.to_raw())
    }
    fn allocate_java_byte_array(&mut self, value: &[u8]) -> Result<u64, EmuError> {
        let length = i32::try_from(value.len())
            .map_err(|_| vm_error("memory-limit", "resource is too large for byte[]"))?;
        let (handle, elements) = self.allocate_native_array(&ArrayKind::Byte, length)?;
        for (destination, byte) in elements.iter_mut().zip(value) {
            *destination = HeapValue::Int(i32::from(byte.cast_signed()));
        }
        Ok(handle.to_raw())
    }
    fn allocate_java_long_array(&mut self, value: &[i64]) -> Result<u64, EmuError> {
        let length = i32::try_from(value.len())
            .map_err(|_| vm_error("memory-limit", "long array is too large"))?;
        let (handle, elements) = self.allocate_native_array(&ArrayKind::Long, length)?;
        for (destination, item) in elements.iter_mut().zip(value) {
            *destination = HeapValue::Long(*item);
        }
        Ok(handle.to_raw())
    }
    fn read_throwable_trace_frame(
        &self,
        reference: u64,
        index: usize,
    ) -> Result<Option<String>, EmuError> {
        if self.host.execution_cancelled() {
            return Err(vm_error(
                "execution-cancelled",
                "Throwable trace output cancelled",
            ));
        }
        self.heap
            .managed
            .get(Handle::from_raw(reference))
            .map_err(heap_error)?;
        Ok(self
            .heap
            .throwable_traces
            .get(&Handle::from_raw(reference))
            .and_then(|frames| frames.get(index))
            .map(|frame| {
                format!(
                    "at {}.{}({}:pc={})",
                    frame.key.class.replace('/', "."),
                    frame.key.name,
                    frame.key.descriptor,
                    frame.bytecode_pc
                )
            }))
    }
    fn capture_throwable_trace(&mut self, reference: u64) -> Result<(), EmuError> {
        let handle = Handle::from_raw(reference);
        self.heap.validate_throwable(self.program, handle)?;
        let frames = self
            .execution
            .call_stack
            .iter()
            .rev()
            .filter_map(|frame| {
                let key = self.program.active_stack_key(frame);
                (key.name != "<init>" && key.name != "fillInStackTrace0").then(|| JavaStackFrame {
                    key: Arc::clone(key),
                    bytecode_pc: frame.bytecode_pc,
                })
            })
            .collect::<Vec<_>>();
        self.account_external_bytes(handle, std::mem::size_of_val(frames.as_slice()))?;
        self.heap.throwable_traces.insert(handle, frames);
        Ok(())
    }
    fn schedule_timer_task(
        &mut self,
        timer: u64,
        task: u64,
        deadline: i64,
        period: i64,
        fixed_rate: bool,
    ) -> Result<(), EmuError> {
        // The dispatcher temporarily removes the active callback from the
        // queue. Reserve its slot so a periodic task can be requeued even if
        // the callback schedules more work before returning.
        let queue_capacity =
            SchedulerState::MAX_SCHEDULED_TASKS - usize::from(self.scheduler.dispatching_timer);
        if self.scheduler.scheduled_tasks.len() >= queue_capacity {
            return Err(vm_error("timer-limit", "too many scheduled Timer tasks"));
        }
        let timer = Handle::from_raw(timer);
        let task = Handle::from_raw(task);
        self.heap.managed.get(timer).map_err(heap_error)?;
        self.heap.managed.get(task).map_err(heap_error)?;
        self.scheduler.scheduled_tasks.push(ScheduledJavaTask {
            timer,
            task,
            deadline,
            period,
            fixed_rate,
        });
        self.scheduler.timer_poll_requested = true;
        Ok(())
    }
    fn cancel_timer(&mut self, timer: u64) {
        self.scheduler.cancel_timer(Handle::from_raw(timer));
    }
    fn object_class(&self, reference: u64) -> Result<String, EmuError> {
        match self
            .heap
            .managed
            .get(Handle::from_raw(reference))
            .map_err(heap_error)?
        {
            Allocation::Object { class, .. } => Ok(class.to_string()),
            Allocation::Array { kind, .. } => Ok(array_descriptor(kind).into_owned()),
        }
    }
    fn read_reference_field(&self, reference: u64, field: &str) -> Result<Option<u64>, EmuError> {
        match self
            .heap
            .managed
            .field(Handle::from_raw(reference), field)
            .map_err(heap_error)?
        {
            HeapValue::Reference(value) => Ok(value.map(Handle::to_raw)),
            _ => Err(type_error()),
        }
    }
}
