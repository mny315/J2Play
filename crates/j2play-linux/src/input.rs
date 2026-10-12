use crate::operation::error;
use diagnostics::EmuError;
use frontend_core::{
    PlatformLifecycleSignal,
    physical_input::{PhysicalBindings, PhysicalInputEvent, PhysicalInputState},
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

struct InputState {
    events: VecDeque<PhysicalInputEvent>,
    overflowed: bool,
    axes: PhysicalInputState,
    dead_zone_percent: u8,
    devices: Vec<(u32, String)>,
    errors: VecDeque<(EmuError, u64)>,
    omitted: u64,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            overflowed: false,
            axes: PhysicalInputState::default(),
            dead_zone_percent: PhysicalBindings::default().dead_zone_percent,
            devices: Vec::new(),
            errors: VecDeque::new(),
            omitted: 0,
            waker: None,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Input(Arc<Mutex<InputState>>);

impl Input {
    fn lock(&self) -> MutexGuard<'_, InputState> {
        self.0.lock().unwrap_or_else(|poisoned| {
            let mut state = poisoned.into_inner();
            self.0.clear_poison();
            state.overflow();
            state
        })
    }

    fn update(&self, update: impl FnOnce(&mut InputState) -> bool) {
        let waker = {
            let mut state = self.lock();
            update(&mut state).then(|| state.waker.clone()).flatten()
        };
        if let Some(waker) = waker {
            waker();
        }
    }

    pub(crate) fn event(&self, event: PhysicalInputEvent, lifecycle: &PlatformLifecycleSignal) {
        self.update(|state| {
            let changes = state.axes.update(event, state.dead_zone_percent);
            if matches!(
                event,
                PhysicalInputEvent::Disconnected { .. } | PhysicalInputEvent::Reset
            ) {
                return state.push(event);
            }
            let mut queued = false;
            for change in changes {
                if !change.pressed || !lifecycle.suspended() {
                    queued |= state.push(PhysicalInputEvent::Button {
                        device: change.device,
                        control: change.control,
                        pressed: change.pressed,
                    });
                }
            }
            queued
        });
    }

    pub(crate) fn poll(&self) -> Option<PhysicalInputEvent> {
        let mut state = self.lock();
        let event = state.events.pop_front();
        if state.events.is_empty() {
            state.overflowed = false;
        }
        event
    }
    pub(crate) fn configure(&self, dead_zone_percent: u8) {
        self.lock().dead_zone_percent = dead_zone_percent;
    }
    pub(crate) fn waker(&self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.lock().waker = Some(waker);
    }
    pub(crate) fn devices(&self) -> Vec<String> {
        self.lock()
            .devices
            .iter()
            .map(|(_, name)| name.clone())
            .collect()
    }
    pub(crate) fn has_devices(&self) -> bool {
        !self.lock().devices.is_empty()
    }

    pub(crate) fn error(&self, error: EmuError) {
        self.update(|state| {
            state.error(error);
            true
        });
    }

    pub(crate) fn device_changed(&self, id: u32, name: Option<String>) {
        self.update(|state| {
            state.devices.retain(|(old, _)| *old != id);
            let event = PhysicalInputEvent::Disconnected { device: id };
            if name.is_none() {
                state.axes.update(event, state.dead_zone_percent);
            }
            if let Some(name) = name
                && state.devices.len() < 8
            {
                state.devices.push((id, name));
            }
            // A remap releases frontend actions but retains the native held
            // controls until neutral. Only removal forgets the device entirely.
            state.push(event);
            true
        });
    }
    pub(crate) fn poll_error(&self) -> Option<EmuError> {
        let mut state = self.lock();
        if let Some((error, count)) = state.errors.pop_front() {
            return Some(if count == 1 {
                error
            } else {
                let message = format!("{} ({count} occurrences)", error.message());
                error.with_message(message)
            });
        }
        let omitted = std::mem::take(&mut state.omitted);
        (omitted > 0).then(|| {
            EmuError::new(
                diagnostics::Category::Platform,
                "linux-diagnostics-overflow",
                format!("{omitted} additional Linux diagnostics were coalesced."),
            )
        })
    }

    pub(crate) fn overflow(&self) {
        self.update(InputState::overflow);
    }
}

impl InputState {
    fn push(&mut self, event: PhysicalInputEvent) -> bool {
        if self.overflowed {
            return false;
        }
        if self.events.len() >= 256 {
            return self.overflow();
        }
        self.events.push_back(event);
        true
    }

    fn overflow(&mut self) -> bool {
        if self.overflowed {
            return false;
        }
        self.events.clear();
        self.events.push_back(PhysicalInputEvent::Reset);
        self.overflowed = true;
        // Keep the native held-state cache: only release/repress may rearm it.
        self.error(error(
            "physical-input-overflow",
            "Physical input exceeded its event limit and was released.",
        ));
        true
    }

    fn error(&mut self, error: EmuError) {
        if let Some((_, count)) = self
            .errors
            .iter_mut()
            .find(|(old, _)| old.code() == error.code())
        {
            *count = count.saturating_add(1);
        } else if self.errors.len() < 8 {
            self.errors.push_back((error, 1));
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/input.rs"]
mod tests;
