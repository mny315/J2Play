use super::{
    Allocation, ArrayKind, CallOutcome, EmuError, Handle, HeapValue, MAIN_THREAD_ID, Machine,
    Method, NativeResume, ThreadState, Value, heap_error, reference_argument,
    suspended_native_call, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(super) fn schedule_frame_presentation(
        &mut self,
        method: &Method,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let pixels = reference_argument(args, 0)?;
        let dimensions = self.java_frame_dimensions(pixels)?;
        self.pace_pending_instructions()?;
        let delay = if self.native_context.realtime_pacing() {
            let changed = !self.java_frame_matches_presented(pixels, dimensions)?;
            self.native_context.lcdui_frame_delay_millis(changed)
        } else {
            0
        };
        let delay = i64::try_from(delay)
            .map_err(|_| vm_error("time-overflow", "frame delay does not fit i64"))?;
        if delay != 0 {
            let deadline = self
                .pacing_monotonic_millis()
                .checked_add(delay)
                .ok_or_else(|| vm_error("time-overflow", "frame deadline overflow"))?;
            // Display and timer callbacks run synchronously and cannot acquire
            // a worker continuation. Ordinary GameCanvas workers can park
            // without stopping independent resource-loading or audio workers.
            if self.is_scheduler_worker()
                && !self.scheduler.dispatching_display
                && !self.scheduler.dispatching_canvas_paint
            {
                return Ok(self.park_frame_presentation(method, pixels, deadline));
            }
            self.pace_or_advance_time(delay)?;
        }
        self.finish_frame_presentation(pixels)
    }

    fn park_frame_presentation(
        &mut self,
        method: &Method,
        pixels: Handle,
        deadline: i64,
    ) -> CallOutcome {
        let thread = Handle::from_raw(self.scheduler.current_thread);
        self.scheduler
            .thread_states
            .insert(thread, ThreadState::Sleeping);
        self.scheduler.sleeping_threads.insert(thread, deadline);
        suspended_native_call(method, NativeResume::FramePresentation { pixels, deadline })
    }

    pub(super) fn resume_frame_presentation(
        &mut self,
        method: &Method,
        pixels: Handle,
        deadline: i64,
    ) -> Result<CallOutcome, EmuError> {
        // Thread.interrupt can wake a parked worker, but flushGraphics is not
        // Thread.sleep: retain its interrupt status and its presentation slot.
        if self.native_context.realtime_pacing() && self.pacing_monotonic_millis() < deadline {
            return Ok(self.park_frame_presentation(method, pixels, deadline));
        }
        let thread = Handle::from_raw(self.scheduler.current_thread);
        self.scheduler.sleeping_threads.remove(&thread);
        self.finish_frame_presentation(pixels)
    }

    fn finish_frame_presentation(&mut self, pixels: Handle) -> Result<CallOutcome, EmuError> {
        let presentation = self.present_java_frame(pixels);
        self.begin_instruction_pacing_segment();
        presentation?;
        if self.native_context.realtime_pacing() {
            if self.is_scheduler_worker() {
                // Poll host input before another guest frame, even when its
                // presentation deadline had already elapsed.
                self.scheduler.suspend_requested = true;
            } else if self.scheduler.current_thread == MAIN_THREAD_ID
                && self.scheduler.host_driver_active
                && self.scheduler.suspended_driver_call.is_none()
            {
                self.scheduler.driver_host_poll_yield = true;
                if !self.scheduler.dispatching_display {
                    self.scheduler.suspend_requested = true;
                }
            }
        }
        Ok(CallOutcome::Return(None))
    }

    pub(in crate::machine) fn java_frame_dimensions(
        &self,
        pixels: Handle,
    ) -> Result<(u32, u32), EmuError> {
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(pixels).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let full_dimensions = (self.limits.lcd_width, self.limits.lcd_height);
        let normal_dimensions = (self.limits.lcd_normal_width, self.limits.lcd_normal_height);
        let area = |(width, height): (u32, u32)| {
            usize::try_from(width)
                .unwrap_or(usize::MAX)
                .checked_mul(usize::try_from(height).unwrap_or(usize::MAX))
        };
        if area(full_dimensions) == Some(elements.len()) {
            Ok(full_dimensions)
        } else if area(normal_dimensions) == Some(elements.len()) {
            Ok(normal_dimensions)
        } else {
            Err(vm_error(
                "framebuffer-size",
                "LCDUI framebuffer matches neither Canvas mode in the active device profile",
            ))
        }
    }

    pub(in crate::machine) fn java_frame_matches_presented(
        &self,
        pixels: Handle,
        dimensions: (u32, u32),
    ) -> Result<bool, EmuError> {
        if self.graphics.presented_dimensions != Some(dimensions) {
            return Ok(false);
        }
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(pixels).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        Ok(elements.len() == self.graphics.present_scratch.len()
            && elements.iter().zip(&self.graphics.present_scratch).all(|(value, previous)| {
                matches!(value, HeapValue::Int(pixel) if pixel.cast_unsigned() == *previous)
            }))
    }

    fn present_java_frame(&mut self, pixels: Handle) -> Result<(), EmuError> {
        let dimensions = self.java_frame_dimensions(pixels)?;
        let Allocation::Array { elements, .. } =
            self.heap.managed.get(pixels).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        // A failed sink call or malformed pixel must not turn its staging
        // buffer into the baseline for a later unchanged-frame decision.
        self.graphics.presented_dimensions = None;
        self.graphics.present_scratch.resize(elements.len(), 0);
        for (target, value) in self.graphics.present_scratch.iter_mut().zip(elements) {
            let HeapValue::Int(pixel) = value else {
                return Err(type_error());
            };
            *target = (*pixel).cast_unsigned();
        }
        self.native_context.present_lcdui_frame(
            dimensions.0,
            dimensions.1,
            &self.graphics.present_scratch,
        )?;
        self.graphics.presented_dimensions = Some(dimensions);
        Ok(())
    }
}
