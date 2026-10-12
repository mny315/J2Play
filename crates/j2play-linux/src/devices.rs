mod audio;
mod gamepad;

use crate::{
    input::Input,
    operation::{self, error},
};
use diagnostics::EmuError;
use frontend_core::{AudioMailbox, PlatformLifecycleSignal, VibrationMailbox, VibrationRequest};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// SDL owns only audio and controllers, on one dedicated adapter thread. It
/// never initializes video, creates a window, renders or executes guest code.
pub(crate) struct Devices {
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Devices {
    pub(crate) fn start(
        input: Input,
        lifecycle: PlatformLifecycleSignal,
        audio: AudioMailbox,
        vibration: VibrationMailbox,
    ) -> Result<Self, EmuError> {
        vibration.set_available(false)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let thread = thread::Builder::new()
            .name("j2play-linux-devices".into())
            .spawn(move || {
                if let Err(error) = run(&input, &lifecycle, &audio, &vibration, &worker_cancel) {
                    input.overflow();
                    input.error(error);
                }
                let _ = vibration.set_available(false);
            })
            .map_err(|_| {
                error(
                    "linux-devices-thread",
                    "Could not start Linux audio and controller services.",
                )
            })?;
        Ok(Self {
            cancel,
            thread: Some(thread),
        })
    }

    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub(crate) fn shutdown(&mut self) -> Result<(), EmuError> {
        self.cancel();
        operation::join(&mut self.thread)
    }
}

impl Drop for Devices {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("j2play: {error}");
        }
    }
}

fn run(
    input: &Input,
    lifecycle: &PlatformLifecycleSignal,
    audio: &AudioMailbox,
    vibration: &VibrationMailbox,
    cancel: &AtomicBool,
) -> Result<(), EmuError> {
    let sdl = sdl2::init().map_err(|_| {
        error(
            "linux-sdl",
            "Linux audio and controller services could not be initialized.",
        )
    })?;
    sdl2::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
    let mut events = sdl
        .event_pump()
        .map_err(|_| error("linux-input", "Controller events could not be initialized."))?;
    let mut gamepads = gamepad::Gamepads::new(&sdl, input);
    let mut output = audio::Output::new(&sdl, audio.clone(), lifecycle.clone(), input.clone());
    while !cancel.load(Ordering::Acquire) && !lifecycle.destroyed() {
        // Bound work before servicing audio and cancellation. Unconsumed SDL
        // events stay queued for the next pass; this budget is not input loss.
        for event in events.poll_iter().take(256) {
            output.event(&event);
            gamepads.event(&event, input, lifecycle);
        }
        output.pump()?;
        vibration.set_available(gamepads.can_rumble())?;
        if lifecycle.suspended() || audio.active_generation()?.is_none() {
            gamepads.rumble(VibrationRequest::Stop, input);
        }
        if let Some(effect) = vibration.take_latest()? {
            let request = if lifecycle.suspended() {
                VibrationRequest::Stop
            } else {
                effect.request
            };
            gamepads.rumble(request, input);
        }
        gamepads.refresh_rumble(input);
        thread::sleep(Duration::from_millis(5));
    }
    gamepads.rumble(VibrationRequest::Stop, input);
    output.stop();
    Ok(())
}
