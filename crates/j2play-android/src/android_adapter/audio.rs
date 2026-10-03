use super::{
    Activity, Arc, AtomicBool, Env, Error, Global, JObject, JValue, JavaVM, Mutex, Ordering,
    Result, active, call, lock, message, new, platform_error, static_call,
};
use frontend_core::{AudioMailbox, AudioStatus};
use jni::objects::JShortArray;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(super) struct AudioPump {
    stop: Arc<AtomicBool>,
    output: Arc<Mutex<Option<Output>>>,
    worker: Option<JoinHandle<()>>,
}

struct Output {
    track: Global<JObject<'static>>,
    playing: bool,
    status: AudioStatus,
}

impl Output {
    fn pause(&mut self, env: &mut Env<'_>) -> Result<()> {
        self.playing = false;
        call(env, &self.track, "pause", "()V", &[])?;
        call(env, &self.track, "flush", "()V", &[])?;
        Ok(())
    }

    fn write(
        &mut self,
        env: &mut Env<'_>,
        samples: &JShortArray<'_>,
        buffer: &[i16],
        status: AudioStatus,
        enabled: bool,
    ) -> Result<()> {
        if self.status.active_generation != status.active_generation
            || self.status.flush_revision != status.flush_revision
            || self.playing != enabled
        {
            self.pause(env)?;
            self.status = status;
            if enabled {
                call(env, &self.track, "play", "()V", &[])?;
                self.playing = true;
            }
        }
        if !enabled || buffer.is_empty() {
            return Ok(());
        }
        samples.set_region(env, 0, buffer)?;
        // WRITE_NON_BLOCKING; a partial write drops the remainder.
        let written = call(
            env,
            &self.track,
            "write",
            "([SIII)I",
            &[
                JValue::Object(samples),
                JValue::Int(0),
                JValue::Int(i32::try_from(buffer.len()).unwrap_or(512)),
                JValue::Int(1),
            ],
        )?
        .i()?;
        if written < 0 {
            return Err(message("Android audio output rejected PCM"));
        }
        Ok(())
    }
}

impl AudioPump {
    pub(super) fn spawn(state: Arc<Activity>, audio: AudioMailbox) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let output = Arc::new(Mutex::new(None));
        let worker_output = Arc::clone(&output);
        let worker = thread::Builder::new()
            .name("j2play-audio-track".into())
            .spawn(move || {
                let result = (|| {
                    JavaVM::singleton()?.attach_current_thread(|env| {
                        play(env, &state, &audio, &worker_stop, &worker_output)
                    })
                })();
                if let Err(error) = result {
                    state
                        .bridge
                        .error(platform_error("android-audio", error.to_string()));
                }
            })?;
        Ok(Self {
            stop,
            output,
            worker: Some(worker),
        })
    }
    pub(super) fn pause(&self, env: &mut Env<'_>) -> Result<()> {
        if let Some(output) = lock(&self.output).as_mut() {
            output.pause(env)?;
        }
        Ok(())
    }
    pub(super) fn stop(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_millis(1500);
        while self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            if Instant::now() >= deadline {
                // Retain ownership on timeout; destruction must not detach this worker.
                return Err(message(
                    "Android audio worker did not stop within its deadline",
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| message("Android audio worker panicked"))?;
        }
        Ok(())
    }
}

fn create<'local>(env: &mut Env<'local>) -> Result<JObject<'local>> {
    let minimum = static_call(
        env,
        "android/media/AudioTrack",
        "getMinBufferSize",
        "(III)I",
        &[JValue::Int(22050), JValue::Int(4), JValue::Int(2)],
    )?
    .i()?;
    if minimum <= 0 {
        return Err(message("Android audio format is unavailable"));
    }
    let attributes = new(env, "android/media/AudioAttributes$Builder", "()V", &[])?;
    call(
        env,
        &attributes,
        "setUsage",
        "(I)Landroid/media/AudioAttributes$Builder;",
        &[JValue::Int(14)],
    )?;
    call(
        env,
        &attributes,
        "setContentType",
        "(I)Landroid/media/AudioAttributes$Builder;",
        &[JValue::Int(2)],
    )?;
    let attributes = call(
        env,
        &attributes,
        "build",
        "()Landroid/media/AudioAttributes;",
        &[],
    )?
    .l()?;
    let format = new(env, "android/media/AudioFormat$Builder", "()V", &[])?;
    for (method, value) in [
        ("setSampleRate", 22050),
        ("setChannelMask", 4),
        ("setEncoding", 2),
    ] {
        call(
            env,
            &format,
            method,
            "(I)Landroid/media/AudioFormat$Builder;",
            &[JValue::Int(value)],
        )?;
    }
    let format = call(env, &format, "build", "()Landroid/media/AudioFormat;", &[])?.l()?;
    let builder = new(env, "android/media/AudioTrack$Builder", "()V", &[])?;
    call(
        env,
        &builder,
        "setAudioAttributes",
        "(Landroid/media/AudioAttributes;)Landroid/media/AudioTrack$Builder;",
        &[JValue::Object(&attributes)],
    )?;
    call(
        env,
        &builder,
        "setAudioFormat",
        "(Landroid/media/AudioFormat;)Landroid/media/AudioTrack$Builder;",
        &[JValue::Object(&format)],
    )?;
    call(
        env,
        &builder,
        "setBufferSizeInBytes",
        "(I)Landroid/media/AudioTrack$Builder;",
        &[JValue::Int(minimum.max(4096))],
    )?;
    call(
        env,
        &builder,
        "setTransferMode",
        "(I)Landroid/media/AudioTrack$Builder;",
        &[JValue::Int(1)],
    )?;
    let track = call(env, &builder, "build", "()Landroid/media/AudioTrack;", &[])?.l()?;
    if call(env, &track, "getState", "()I", &[])?.i()? != 1 {
        call(env, &track, "release", "()V", &[])?;
        return Err(message("Android audio output did not initialize"));
    }
    Ok(track)
}

fn play(
    env: &mut Env<'_>,
    state: &Activity,
    audio: &AudioMailbox,
    stop: &AtomicBool,
    output: &Mutex<Option<Output>>,
) -> Result<()> {
    let track = create(env)?;
    let result = (|| {
        let samples = env.new_short_array(512)?;
        *lock(output) = Some(Output {
            track: env.new_global_ref(&track)?,
            playing: false,
            status: AudioStatus::default(),
        });
        let mut buffer = [0_i16; 512];
        while !stop.load(Ordering::Acquire) && !state.destroyed.load(Ordering::Acquire) {
            let started = Instant::now();
            // Bound local JNI references over an arbitrarily long playback session.
            env.with_local_frame(32, |env| -> Result<()> {
                // UI lifecycle pause and PCM writes share this lock. Once pause
                // returns, an inactive Activity cannot enqueue another block.
                if let Some(output) = lock(output).as_mut() {
                    let (status, count) = audio
                        .read_available(&mut buffer)
                        .map_err(|error| Error::Message(error.to_string()))?;
                    let enabled = active(state) && status.active_generation.is_some();
                    output.write(env, &samples, &buffer[..count], status, enabled)?;
                }
                Ok(())
            })?;
            thread::sleep(Duration::from_millis(5).saturating_sub(started.elapsed()));
        }
        Ok(())
    })();
    // Exclude lifecycle calls while releasing the SDK track.
    let mut output = lock(output);
    env.exception_clear();
    let _ = call(env, &track, "pause", "()V", &[]);
    env.exception_clear();
    let _ = call(env, &track, "flush", "()V", &[]);
    env.exception_clear();
    let _ = call(env, &track, "release", "()V", &[]);
    env.exception_clear();
    *output = None;
    result
}
