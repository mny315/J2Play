//! Android SDK access. Generated callback declarations contain no application logic.

mod audio;
mod document;
mod ffi;
mod ime;
mod open_file;
pub(crate) mod physical_input;
mod window;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use jni::objects::{Global, JObject, JString, JValue, JValueOwned};
use jni::signature::RuntimeMethodSignature;
use jni::strings::JNIString;
use jni::{Env, JavaVM};

use crate::platform_bridge::{BridgeState, Command, platform_error};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
enum Error {
    Jni(jni::errors::Error),
    Io(std::io::Error),
    Message(String),
    Expired,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jni(error) => write!(f, "Android SDK call failed: {error}"),
            Self::Io(error) => write!(f, "Android host operation failed: {error}"),
            Self::Message(message) => f.write_str(message),
            Self::Expired => f.write_str("Android callback belongs to an expired Activity"),
        }
    }
}
impl std::error::Error for Error {}
impl From<jni::errors::Error> for Error {
    fn from(error: jni::errors::Error) -> Self {
        Self::Jni(error)
    }
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn message(value: &str) -> Error {
    Error::Message(value.to_owned())
}

fn call<'local>(
    env: &mut Env<'local>,
    object: &JObject<'_>,
    name: &str,
    signature: &str,
    args: &[JValue<'_>],
) -> Result<JValueOwned<'local>> {
    Ok(env.call_method(
        object,
        JNIString::from(name),
        RuntimeMethodSignature::from_str(signature)?.method_signature(),
        args,
    )?)
}

fn static_call<'local>(
    env: &mut Env<'local>,
    class: &str,
    name: &str,
    signature: &str,
    args: &[JValue<'_>],
) -> Result<JValueOwned<'local>> {
    Ok(env.call_static_method(
        JNIString::from(class),
        JNIString::from(name),
        RuntimeMethodSignature::from_str(signature)?.method_signature(),
        args,
    )?)
}

fn super_call<'local>(
    env: &mut Env<'local>,
    object: &JObject<'_>,
    class: &str,
    name: &str,
    signature: &str,
    args: &[JValue<'_>],
) -> Result<JValueOwned<'local>> {
    let object = env.new_local_ref(object)?;
    Ok(env.call_nonvirtual_method(
        &object,
        JNIString::from(class),
        JNIString::from(name),
        RuntimeMethodSignature::from_str(signature)?.method_signature(),
        args,
    )?)
}

fn new<'local>(
    env: &mut Env<'local>,
    class: &str,
    signature: &str,
    args: &[JValue<'_>],
) -> Result<JObject<'local>> {
    Ok(env.new_object(
        JNIString::from(class),
        RuntimeMethodSignature::from_str(signature)?.method_signature(),
        args,
    )?)
}

fn string<'local>(env: &mut Env<'local>, value: &str) -> Result<JString<'local>> {
    Ok(JString::from_str(env, value)?)
}

fn text(env: &mut Env<'_>, object: &JObject<'_>, maximum: usize) -> Result<String> {
    let length = call(env, object, "length", "()I", &[])?.i()?;
    if length < 0 || usize::try_from(length).unwrap_or(usize::MAX) > maximum {
        return Err(message("Android text exceeds its size limit"));
    }
    let value = call(env, object, "toString", "()Ljava/lang/String;", &[])?.l()?;
    let value = JString::cast_local(env, value)?.to_string();
    if value.len() > maximum {
        return Err(message("Android text exceeds its size limit"));
    }
    Ok(value)
}

fn field_int(env: &mut Env<'_>, object: &JObject<'_>, name: &str) -> Result<i32> {
    Ok(env
        .get_field(object, JNIString::from(name), jni::jni_sig!("I"))?
        .i()?)
}

fn service<'local>(env: &mut Env<'local>, state: &Activity, name: &str) -> Result<JObject<'local>> {
    let name = string(env, name)?;
    call(
        env,
        &state.object,
        "getSystemService",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        &[JValue::Object(&name)],
    )?
    .l()
    .map_err(Into::into)
}

struct Activity {
    object: Global<JObject<'static>>,
    callbacks: Global<JObject<'static>>,
    editor: Mutex<Option<Arc<Global<JObject<'static>>>>>,
    back: Mutex<Option<Arc<Global<JObject<'static>>>>>,
    connection: Mutex<Option<Arc<Global<JObject<'static>>>>>,
    bridge: Arc<BridgeState>,
    commands: Mutex<VecDeque<Command>>,
    scheduled: AtomicBool,
    destroyed: AtomicBool,
    resumed: AtomicBool,
    focused: AtomicBool,
    fullscreen: AtomicBool,
    theme_mode: Mutex<Option<frontend_ui::PlatformThemeMode>>,
    ime_requested: AtomicBool,
    ime_visible: AtomicBool,
    dead_accent: Mutex<i32>,
    api: i32,
    audio: Mutex<Option<audio::AudioPump>>,
    document: Mutex<document::Documents>,
}

static CURRENT: Mutex<Option<Arc<Activity>>> = Mutex::new(None);

pub(crate) fn bridge() -> std::result::Result<Arc<BridgeState>, diagnostics::EmuError> {
    lock(&CURRENT)
        .as_ref()
        .map(|state| Arc::clone(&state.bridge))
        .ok_or_else(|| {
            platform_error(
                "android-activity",
                "Android callback adapter is not initialized",
            )
        })
}

fn current(env: &mut Env<'_>, object: &JObject<'_>) -> Result<Arc<Activity>> {
    let state = lock(&CURRENT).clone().ok_or(Error::Expired)?;
    if state.destroyed.load(Ordering::Acquire) || !env.is_same_object(&state.object, object)? {
        return Err(Error::Expired);
    }
    Ok(state)
}

fn owner(env: &mut Env<'_>, object: &JObject<'_>) -> Result<Arc<Activity>> {
    let activity = env
        .get_field(
            object,
            jni::jni_str!("owner"),
            jni::jni_sig!("Lio/github/mny315/j2play/J2PlayActivity;"),
        )?
        .l()?;
    current(env, &activity)
}

fn active(state: &Activity) -> bool {
    !state.destroyed.load(Ordering::Acquire)
        && state.resumed.load(Ordering::Acquire)
        && state.focused.load(Ordering::Acquire)
}

pub(crate) fn post(
    bridge: &Arc<BridgeState>,
    command: Command,
) -> std::result::Result<(), diagnostics::EmuError> {
    let result = (|| -> Result<()> {
        let state = lock(&CURRENT)
            .clone()
            .ok_or_else(|| message("Android Activity is unavailable"))?;
        if state.destroyed.load(Ordering::Acquire) || !Arc::ptr_eq(bridge, &state.bridge) {
            return Err(message("Android request belongs to an expired Activity"));
        }
        {
            let mut commands = lock(&state.commands);
            if matches!(command, Command::Shutdown) {
                commands.clear();
            }
            // Latest state commands replace pending commands of the same kind.
            if let Some(index) = commands.iter().position(|old| command.replaces(old)) {
                commands.remove(index);
            }
            if commands.len() >= 64 {
                return Err(message("Android command queue is full"));
            }
            commands.push_back(command);
        }
        if !state.scheduled.swap(true, Ordering::AcqRel) {
            let result = JavaVM::singleton()?.attach_current_thread(|env| -> Result<()> {
                call(
                    env,
                    &state.object,
                    "runOnUiThread",
                    "(Ljava/lang/Runnable;)V",
                    &[JValue::Object(&state.callbacks)],
                )?;
                Ok(())
            });
            if result.is_err() {
                state.scheduled.store(false, Ordering::Release);
            }
            result?;
        }
        Ok(())
    })();
    result.map_err(|error| platform_error("android-command", error.to_string()))
}

fn run(env: &mut Env<'_>, state: &Arc<Activity>) {
    let commands: Vec<_> = {
        let mut commands = lock(&state.commands);
        state.scheduled.store(false, Ordering::Release);
        commands.drain(..).collect()
    };
    for command in commands {
        let result = env.with_local_frame(128, |env| dispatch(env, state, command));
        if let Err(error) = result {
            env.exception_clear();
            state
                .bridge
                .error(platform_error("android-operation", error.to_string()));
        }
    }
}

fn dispatch(env: &mut Env<'_>, state: &Arc<Activity>, command: Command) -> Result<()> {
    match command {
        Command::Pick(kind) => {
            document::pick(env, state, kind);
            Ok(())
        }
        Command::BindAudio(mailbox) => {
            if lock(&state.audio).is_some() {
                return Err(message("Android audio is already connected"));
            }
            let pump = audio::AudioPump::spawn(Arc::clone(state), mailbox)?;
            *lock(&state.audio) = Some(pump);
            Ok(())
        }
        Command::Vibration(request) => window::vibration(env, state, request),
        Command::Haptic(strength) => window::haptic(env, state, strength),
        Command::Fullscreen(value) => {
            state.fullscreen.store(value, Ordering::Release);
            window::configure(env, state)
        }
        Command::Orientation(value) => window::orientation(env, state, value),
        Command::ThemeMode(value) => {
            *lock(&state.theme_mode) = Some(value);
            window::configure(env, state)
        }
        Command::Ime(value) => {
            state.ime_requested.store(value, Ordering::Release);
            ime::visibility(env, state)
        }
        Command::OpenUrl { url, generation } => state
            .bridge
            .open_external_url(generation, || window::open_url(env, state, &url)),
        Command::Shutdown => {
            let result = stop_audio(state);
            report(env, state, result);
            state.ime_requested.store(false, Ordering::Release);
            state.fullscreen.store(false, Ordering::Release);
            // Clear each JNI exception before the next independent cleanup.
            // A failed IME call must not leave vibration or orientation active.
            let result = ime::visibility(env, state);
            report(env, state, result);
            let result = window::vibration(env, state, frontend_core::VibrationRequest::Stop);
            report(env, state, result);
            let result =
                window::orientation(env, state, frontend_ui::PlatformOrientation::Automatic);
            report(env, state, result);
            window::configure(env, state)
        }
    }
}

fn create(env: &mut Env<'_>, object: &JObject<'_>, saved: &JObject<'_>) -> Result<()> {
    let old = lock(&CURRENT).clone();
    if let Some(old) = old {
        if !old.destroyed.load(Ordering::Acquire) {
            return Err(message("A second Android Activity cannot own the emulator"));
        }
        // A failed teardown retains its handles. Never replace it while workers
        // are still running or silently detach them on Activity recreation.
        stop_audio(&old)?;
        document::cancel(&old)?;
    }
    let callbacks = new(
        env,
        "io/github/mny315/j2play/Callbacks",
        "(Lio/github/mny315/j2play/J2PlayActivity;)V",
        &[JValue::Object(object)],
    )?;
    let api = env
        .get_static_field(
            jni::jni_str!("android/os/Build$VERSION"),
            jni::jni_str!("SDK_INT"),
            jni::jni_sig!("I"),
        )?
        .i()?;
    let state = Arc::new(Activity {
        object: env.new_global_ref(object)?,
        callbacks: env.new_global_ref(callbacks)?,
        editor: Mutex::new(None),
        back: Mutex::new(None),
        connection: Mutex::new(None),
        bridge: Arc::new(BridgeState::default()),
        commands: Mutex::new(VecDeque::new()),
        scheduled: AtomicBool::new(false),
        destroyed: AtomicBool::new(false),
        resumed: AtomicBool::new(false),
        focused: AtomicBool::new(false),
        fullscreen: AtomicBool::new(false),
        theme_mode: Mutex::new(None),
        ime_requested: AtomicBool::new(false),
        ime_visible: AtomicBool::new(false),
        dead_accent: Mutex::new(0),
        api,
        audio: Mutex::new(None),
        document: Mutex::new(document::Documents::default()),
    });
    *lock(&CURRENT) = Some(Arc::clone(&state));
    let result = window::configure(env, &state);
    report(env, &state, result);
    super_call(
        env,
        object,
        "android/app/NativeActivity",
        "onCreate",
        "(Landroid/os/Bundle;)V",
        &[JValue::Object(saved)],
    )?;
    let result = ime::create(env, &state);
    report(env, &state, result);
    let result = window::install(env, &state);
    report(env, &state, result);
    let result = window::theme(env, &state);
    report(env, &state, result);
    let result = window::text_scale(env, &state);
    report(env, &state, result);
    let result = document::restore(env, &state, saved);
    report(env, &state, result);
    let result = physical_input::install(env, &state);
    report(env, &state, result);
    if saved.is_null() {
        let intent = call(env, object, "getIntent", "()Landroid/content/Intent;", &[])?.l()?;
        open_file::receive(env, &state, &intent);
    }
    Ok(())
}

fn report(env: &mut Env<'_>, state: &Activity, result: Result<()>) {
    if let Err(error) = result {
        env.exception_clear();
        eprintln!("j2play: platform[android-callback]: {error}");
        state
            .bridge
            .error(platform_error("android-callback", error.to_string()));
    }
}

fn stop_audio(state: &Activity) -> Result<()> {
    let mut audio = lock(&state.audio);
    if let Some(pump) = audio.as_mut() {
        pump.stop()?;
    }
    *audio = None;
    Ok(())
}

fn transient(env: &mut Env<'_>, state: &Activity) {
    if !active(state) {
        lock(&state.commands).retain(|command| !command.is_transient());
        if let Some(pump) = lock(&state.audio).as_ref() {
            let result = pump.pause(env);
            report(env, state, result);
        }
        let result = window::vibration(env, state, frontend_core::VibrationRequest::Stop);
        report(env, state, result);
    }
    let result = ime::visibility(env, state);
    report(env, state, result);
}

fn destroy(env: &mut Env<'_>, state: &Arc<Activity>) -> Result<()> {
    state.destroyed.store(true, Ordering::Release);
    state.bridge.destroy();
    lock(&state.commands).clear();
    transient(env, state);
    let audio = stop_audio(state);
    let audio_stopped = audio.is_ok();
    report(env, state, audio);
    let document = document::cancel(state);
    let document_stopped = document.is_ok();
    report(env, state, document);
    let result = window::uninstall(env, state);
    report(env, state, result);
    let result = physical_input::uninstall(env, state);
    report(env, state, result);
    // Never retain the global lock while NativeActivity waits for android_main.
    if audio_stopped && document_stopped {
        *lock(&CURRENT) = None;
    }
    super_call(
        env,
        &state.object,
        "android/app/NativeActivity",
        "onDestroy",
        "()V",
        &[],
    )?;
    Ok(())
}
