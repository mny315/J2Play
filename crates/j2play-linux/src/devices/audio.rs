use crate::{input::Input, operation::error};
use diagnostics::EmuError;
use frontend_core::{AttemptId, AudioMailbox, PlatformLifecycleSignal, SessionId};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};

struct Callback {
    mailbox: AudioMailbox,
    lifecycle: PlatformLifecycleSignal,
    input: Input,
    failed: bool,
    #[cfg(test)]
    played: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl AudioCallback for Callback {
    type Channel = i16;
    fn callback(&mut self, output: &mut [i16]) {
        if self.lifecycle.suspended() || self.failed {
            output.fill(0);
            return;
        }
        match self.mailbox.read_or_silence(output) {
            Ok(count) => {
                let _ = count;
                #[cfg(test)]
                if output[..count].iter().any(|sample| *sample != 0) {
                    self.played
                        .fetch_add(count, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Err(error) => {
                output.fill(0);
                self.failed = true;
                self.input.error(error);
            }
        }
    }
}

/// The callback reads the shared ring directly. There is no second unbounded
/// queue or retained chunk which could replay audio from a previous attempt.
pub(super) struct Output {
    sdl: sdl2::Sdl,
    subsystem: Option<sdl2::AudioSubsystem>,
    device: Option<AudioDevice<Callback>>,
    mailbox: AudioMailbox,
    lifecycle: PlatformLifecycleSignal,
    input: Input,
    attempted: Option<(SessionId, AttemptId)>,
    retry: bool,
    #[cfg(test)]
    played: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Output {
    pub(super) fn new(
        sdl: &sdl2::Sdl,
        mailbox: AudioMailbox,
        lifecycle: PlatformLifecycleSignal,
        input: Input,
    ) -> Self {
        let subsystem = sdl
            .audio()
            .ok()
            .filter(|audio| !matches!(audio.current_audio_driver(), "dummy" | "disk"));
        Self {
            sdl: sdl.clone(),
            subsystem,
            device: None,
            mailbox,
            lifecycle,
            input,
            attempted: None,
            retry: true,
            #[cfg(test)]
            played: std::sync::Arc::default(),
        }
    }

    pub(super) fn event(&mut self, event: &sdl2::event::Event) {
        if matches!(
            event,
            sdl2::event::Event::AudioDeviceAdded {
                iscapture: false,
                ..
            }
        ) {
            self.retry = true;
        }
    }

    pub(super) fn pump(&mut self) -> Result<(), EmuError> {
        let status = self.mailbox.status()?;
        if self
            .device
            .as_ref()
            .is_some_and(|device| device.status() == sdl2::audio::AudioStatus::Stopped)
        {
            self.device = None;
            self.input.error(error("linux-audio-lost", "The audio output device was disconnected. Connect an output device or restart the game."));
        }
        if self.device.is_none()
            && status.queued_frames > 0
            && (self.retry || self.attempted != status.active_generation)
        {
            self.retry = false;
            self.attempted = status.active_generation;
            match self.open() {
                Ok(device) => self.device = Some(device),
                Err(error) => self.input.error(error),
            }
        }
        if let Some(device) = &self.device {
            let suspended = self.lifecycle.suspended() || status.active_generation.is_none();
            // SDL pause/resume locks the audio callback. Change playback only
            // on an actual transition, not on every adapter poll.
            match device.status() {
                sdl2::audio::AudioStatus::Playing if suspended => device.pause(),
                sdl2::audio::AudioStatus::Paused if !suspended && status.queued_frames > 0 => {
                    device.resume();
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn open(&mut self) -> Result<AudioDevice<Callback>, EmuError> {
        let unavailable = || {
            error(
                "linux-audio-unavailable",
                "No audio output is available. Check PulseAudio or PipeWire and connect an output device, then restart the game.",
            )
        };
        if self.subsystem.is_none() {
            self.subsystem = self
                .sdl
                .audio()
                .ok()
                .filter(|audio| !matches!(audio.current_audio_driver(), "dummy" | "disk"));
        }
        let subsystem = self.subsystem.as_ref().ok_or_else(unavailable)?;
        let spec = AudioSpecDesired {
            freq: Some(22_050),
            channels: Some(1),
            samples: Some(256),
        };
        subsystem
            .open_playback(None, &spec, |_| Callback {
                mailbox: self.mailbox.clone(),
                lifecycle: self.lifecycle.clone(),
                input: self.input.clone(),
                failed: false,
                #[cfg(test)]
                played: self.played.clone(),
            })
            .map_err(|_| unavailable())
    }

    pub(super) fn stop(&mut self) {
        if let Some(device) = self.device.take() {
            device.pause();
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/j2play-linux/devices/audio.rs"]
mod tests;
