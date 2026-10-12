use frontend_core::PlatformLifecycleSignal;
use frontend_ui::PlatformLifecycleEvent;
use std::sync::{Arc, Mutex};

/// Coalesces state while retaining a barrier across a complete suspend/resume
/// or focus-loss/gain cycle between paints. No unbounded lifecycle event queue.
#[derive(Clone)]
pub(crate) struct Lifecycle {
    pub(crate) signal: PlatformLifecycleSignal,
    state: Arc<Mutex<State>>,
}

#[allow(clippy::struct_excessive_bools)] // Independent host causes and retained delivery barriers.
struct State {
    focused: bool,
    minimized: bool,
    sleeping: bool,
    destroyed: bool,
    suspend_barrier: bool,
    reported_suspended: bool,
    reported_destroyed: bool,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            signal: PlatformLifecycleSignal::initially_suspended(),
            state: Arc::new(Mutex::new(State {
                focused: false,
                minimized: false,
                // Do not run a queued launch until logind setup has succeeded.
                sleeping: true,
                destroyed: false,
                suspend_barrier: true,
                reported_suspended: false,
                reported_destroyed: false,
            })),
        }
    }
}

impl Lifecycle {
    // A picker taking focus is expected. Only actual system sleep/destroy
    // cancels portal operations; ordinary focus loss still suspends the guest.
    pub(crate) fn system_suspended(&self) -> bool {
        self.state
            .lock()
            .map_or(true, |state| state.sleeping || state.destroyed)
    }

    pub(crate) fn window(&self, focused: bool, minimized: bool) {
        self.change(|state| {
            state.focused = focused;
            state.minimized = minimized;
        });
    }

    pub(crate) fn sleep(&self, sleeping: bool) {
        self.change(|state| state.sleeping = sleeping);
    }

    pub(crate) fn destroy(&self) {
        self.change(|state| state.destroyed = true);
    }

    fn change(&self, update: impl FnOnce(&mut State)) {
        let Ok(mut state) = self.state.lock() else {
            self.signal.mark_destroyed();
            return;
        };
        let was_suspended = state.suspended();
        update(&mut state);
        let suspended = state.suspended();
        state.suspend_barrier |= !was_suspended && suspended;
        if state.destroyed {
            self.signal.mark_destroyed();
        } else {
            // Keep even a complete sleep/focus cycle latched until the shared
            // UI has dispatched its release/pause commands to the worker.
            self.signal
                .set_suspended(suspended || state.suspend_barrier || state.reported_suspended);
        }
    }

    pub(crate) fn poll(&self) -> Option<PlatformLifecycleEvent> {
        let Ok(mut state) = self.state.lock() else {
            self.signal.mark_destroyed();
            return Some(PlatformLifecycleEvent::Destroyed);
        };
        if state.destroyed {
            if state.reported_destroyed {
                return None;
            }
            state.reported_destroyed = true;
            return Some(PlatformLifecycleEvent::Destroyed);
        }
        if std::mem::take(&mut state.suspend_barrier) {
            state.reported_suspended = true;
            return Some(PlatformLifecycleEvent::Suspended);
        }
        let suspended = state.suspended();
        if suspended == state.reported_suspended {
            return None;
        }
        state.reported_suspended = suspended;
        self.signal.set_suspended(suspended);
        Some(if suspended {
            PlatformLifecycleEvent::Suspended
        } else {
            PlatformLifecycleEvent::Resumed
        })
    }
}

impl State {
    fn suspended(&self) -> bool {
        self.destroyed || self.sleeping || self.minimized || !self.focused
    }
}

/// eframe retains raw events across logic-only passes while a window is hidden.
/// Only newly appended focus events may create another release barrier.
#[derive(Default)]
pub(crate) struct WindowInput {
    focus_events_seen: usize,
}

impl WindowInput {
    pub(crate) fn observe(&mut self, lifecycle: &Lifecycle, input: &eframe::egui::RawInput) {
        let minimized = input.viewport().minimized.unwrap_or(false);
        let mut count = 0;
        for event in &input.events {
            if let eframe::egui::Event::WindowFocused(focused) = event {
                if count >= self.focus_events_seen && !focused {
                    lifecycle.window(false, minimized);
                }
                count += 1;
            }
        }
        self.focus_events_seen = count;
        lifecycle.window(input.focused, minimized);
    }

    pub(crate) fn painted(&mut self) {
        self.focus_events_seen = 0;
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/lifecycle.rs"]
mod tests;
