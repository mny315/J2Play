//! The only Android FFI exports. `EnvUnowned` validates attachment and catches panics.

use super::{
    Error, JObject, JValue, Ordering, Result, create, current, destroy, document, ime, open_file,
    owner, physical_input, report, run, super_call, transient, window,
};
use jni::{Env, EnvUnowned};

fn callback<'local, T: Default>(
    env: &mut Env<'local>,
    body: impl FnOnce(&mut Env<'local>) -> Result<T>,
) -> Result<T> {
    match body(env) {
        Err(Error::Expired) => {
            env.exception_clear();
            Ok(T::default())
        }
        result => result,
    }
}

macro_rules! native {
    ($name:ident, ($($argument:ident: $kind:ty),*) -> $result:ty, |$env:ident, $this:ident| $body:block) => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "system" fn $name<'caller>(
            mut incoming: EnvUnowned<'caller>,
            $this: JObject<'caller>,
            $($argument: $kind),*
        ) -> $result {
            incoming.with_env(|environment| callback(environment, |$env| -> Result<$result> { $body }))
                .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
        }
    };
}

native!(Java_io_github_mny315_j2play_J2PlayActivity_onCreate, (saved: JObject<'caller>) -> (), |env, this| {
    create(env, &this, &saved)
});
native!(Java_io_github_mny315_j2play_J2PlayActivity_onResume, () -> (), |env, this| {
    super_call(env, &this, "android/app/NativeActivity", "onResume", "()V", &[])?;
    let state = current(env, &this)?;
    state.resumed.store(true, Ordering::Release);
    state.bridge.suspend(false);
    let result = window::theme(env, &state);
    report(env, &state, result);
    let result = window::text_scale(env, &state);
    report(env, &state, result);
    let result = window::configure(env, &state);
    report(env, &state, result);
    transient(env, &state);
    Ok(())
});
native!(Java_io_github_mny315_j2play_J2PlayActivity_onPause, () -> (), |env, this| {
    let state = current(env, &this)?;
    state.resumed.store(false, Ordering::Release);
    state.bridge.suspend(true);
    transient(env, &state);
    super_call(env, &this, "android/app/NativeActivity", "onPause", "()V", &[])?;
    Ok(())
});
native!(Java_io_github_mny315_j2play_J2PlayActivity_onWindowFocusChanged, (focus: jni::sys::jboolean) -> (), |env, this| {
    super_call(env, &this, "android/app/NativeActivity", "onWindowFocusChanged", "(Z)V", &[JValue::Bool(focus)])?;
    let state = current(env, &this)?;
    state.focused.store(focus, Ordering::Release);
    state.bridge.focus(focus);
    if focus {
        let result = window::configure(env, &state);
        report(env, &state, result);
    }
    transient(env, &state);
    Ok(())
});
native!(Java_io_github_mny315_j2play_J2PlayActivity_onConfigurationChanged, (configuration: JObject<'caller>) -> (), |env, this| {
    super_call(env, &this, "android/app/NativeActivity", "onConfigurationChanged", "(Landroid/content/res/Configuration;)V", &[JValue::Object(&configuration)])?;
    let state = current(env, &this)?;
    let result = window::theme(env, &state);
    report(env, &state, result);
    let result = window::text_scale(env, &state);
    report(env, &state, result);
    let result = window::configure(env, &state);
    report(env, &state, result);
    Ok(())
});
native!(Java_io_github_mny315_j2play_J2PlayActivity_onSaveInstanceState, (saved: JObject<'caller>) -> (), |env, this| {
    let state = current(env, &this)?;
    let result = document::save(env, &state, &saved);
    report(env, &state, result);
    super_call(env, &this, "android/app/NativeActivity", "onSaveInstanceState", "(Landroid/os/Bundle;)V", &[JValue::Object(&saved)])?;
    Ok(())
});
native!(Java_io_github_mny315_j2play_J2PlayActivity_onActivityResult, (request: i32, result: i32, data: JObject<'caller>) -> (), |env, this| {
    super_call(env, &this, "android/app/NativeActivity", "onActivityResult", "(IILandroid/content/Intent;)V", &[JValue::Int(request), JValue::Int(result), JValue::Object(&data)])?;
    let state = current(env, &this)?;
    document::result(env, &state, request, result, &data);
    Ok(())
});
native!(Java_io_github_mny315_j2play_J2PlayActivity_onDestroy, () -> (), |env, this| {
    let state = current(env, &this)?;
    destroy(env, &state)
});
native!(Java_io_github_mny315_j2play_J2PlayActivity_onNewIntent, (intent: JObject<'caller>) -> (), |env, this| {
    super_call(env, &this, "android/app/NativeActivity", "onNewIntent", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)])?;
    let state = current(env, &this)?;
    open_file::receive(env, &state, &intent);
    Ok(())
});
native!(Java_io_github_mny315_j2play_J2PlayActivity_onBackPressed, () -> (), |env, this| {
    current(env, &this)?.bridge.back.store(true, Ordering::Release);
    Ok(())
});
native!(Java_io_github_mny315_j2play_BackCallback_onBackInvoked, () -> (), |env, this| {
    owner(env, &this)?.bridge.back.store(true, Ordering::Release);
    Ok(())
});
native!(Java_io_github_mny315_j2play_Callbacks_run, () -> (), |env, this| {
    let state = owner(env, &this)?;
    run(env, &state);
    Ok(())
});
native!(Java_io_github_mny315_j2play_Callbacks_onInputDeviceAdded, (id: i32) -> (), |env, this| {
    let state = owner(env, &this)?;
    let result = physical_input::device_changed(env, &state, id);
    report(env, &state, result);
    Ok(())
});
native!(Java_io_github_mny315_j2play_Callbacks_onInputDeviceChanged, (id: i32) -> (), |env, this| {
    let state = owner(env, &this)?;
    let result = physical_input::device_changed(env, &state, id);
    report(env, &state, result);
    Ok(())
});
native!(Java_io_github_mny315_j2play_Callbacks_onInputDeviceRemoved, (id: i32) -> (), |env, this| {
    owner(env, &this)?.bridge.physical_device(u32::from_ne_bytes(id.to_ne_bytes()), None);
    Ok(())
});
native!(Java_io_github_mny315_j2play_Callbacks_onApplyWindowInsets, (view: JObject<'caller>, insets: JObject<'caller>) -> JObject<'caller>, |env, this| {
    let _ = view;
    if let Ok(state) = owner(env, &this) { window::insets(env, &state, &insets)?; }
    Ok(insets)
});

native!(Java_io_github_mny315_j2play_BridgeEditor_onKeyPreIme, (key: i32, event: JObject<'caller>) -> jni::sys::jboolean, |env, this| {
    let state = owner(env, &this)?;
    ime::key(env, &state, &this, key, &event, "onKeyPreIme")
});
native!(Java_io_github_mny315_j2play_BridgeEditor_onKeyDown, (key: i32, event: JObject<'caller>) -> jni::sys::jboolean, |env, this| {
    let state = owner(env, &this)?;
    ime::key(env, &state, &this, key, &event, "onKeyDown")
});
native!(Java_io_github_mny315_j2play_BridgeEditor_onKeyUp, (key: i32, event: JObject<'caller>) -> jni::sys::jboolean, |env, this| {
    let state = owner(env, &this)?;
    ime::key(env, &state, &this, key, &event, "onKeyUp")
});
native!(Java_io_github_mny315_j2play_BridgeEditor_onCreateInputConnection, (attributes: JObject<'caller>) -> JObject<'caller>, |env, this| {
    let state = owner(env, &this)?;
    ime::connection(env, &state, &this, &attributes)
});

native!(Java_io_github_mny315_j2play_EditorConnection_setComposingText, (text: JObject<'caller>, cursor: i32) -> jni::sys::jboolean, |env, this| {
    ime::change(env, &this, "setComposingText", "(Ljava/lang/CharSequence;I)Z", &[JValue::Object(&text), JValue::Int(cursor)], false)
});
native!(Java_io_github_mny315_j2play_EditorConnection_commitText, (text: JObject<'caller>, cursor: i32) -> jni::sys::jboolean, |env, this| {
    ime::change(env, &this, "commitText", "(Ljava/lang/CharSequence;I)Z", &[JValue::Object(&text), JValue::Int(cursor)], true)
});
native!(Java_io_github_mny315_j2play_EditorConnection_setComposingRegion, (start: i32, end: i32) -> jni::sys::jboolean, |env, this| {
    ime::change(env, &this, "setComposingRegion", "(II)Z", &[JValue::Int(start), JValue::Int(end)], false)
});
native!(Java_io_github_mny315_j2play_EditorConnection_setSelection, (start: i32, end: i32) -> jni::sys::jboolean, |env, this| {
    ime::change(env, &this, "setSelection", "(II)Z", &[JValue::Int(start), JValue::Int(end)], false)
});
native!(Java_io_github_mny315_j2play_EditorConnection_finishComposingText, () -> jni::sys::jboolean, |env, this| {
    let state = owner(env, &this)?;
    ime::commit(env, &state)?;
    Ok(super_call(env, &this, "android/view/inputmethod/InputConnectionWrapper", "finishComposingText", "()Z", &[])?.z()?)
});
native!(Java_io_github_mny315_j2play_EditorConnection_deleteSurroundingText, (before: i32, after: i32) -> jni::sys::jboolean, |env, this| {
    ime::delete(env, &this, before, after, false)
});
native!(Java_io_github_mny315_j2play_EditorConnection_deleteSurroundingTextInCodePoints, (before: i32, after: i32) -> jni::sys::jboolean, |env, this| {
    ime::delete(env, &this, before, after, true)
});
