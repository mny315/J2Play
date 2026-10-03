use super::{
    Activity, Arc, Env, Global, JObject, JValue, Ordering, Result, active, call, lock, message,
    new, owner, service, static_call, string, super_call, text,
};
use frontend_ui::{HostAction, PlatformTextInputEvent as Input};

fn editor(state: &Activity) -> Result<Arc<Global<JObject<'static>>>> {
    lock(&state.editor)
        .clone()
        .ok_or_else(|| message("Android text editor is unavailable"))
}

fn editable<'local>(env: &mut Env<'local>, editor: &JObject<'_>) -> Result<JObject<'local>> {
    Ok(call(env, editor, "getText", "()Landroid/text/Editable;", &[])?.l()?)
}

pub(super) fn create(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let editor = new(
        env,
        "io/github/mny315/j2play/BridgeEditor",
        "(Lio/github/mny315/j2play/J2PlayActivity;)V",
        &[JValue::Object(&state.object)],
    )?;
    for (name, value) in [
        // TYPE_CLASS_TEXT | TYPE_TEXT_FLAG_MULTI_LINE. The bridge clears its
        // buffer after commits, so sentence capitalization would repeatedly
        // treat the next input as the start of a sentence.
        ("setInputType", 0x20001),
        ("setImeOptions", 0x1200_0000),
        ("setVisibility", 4),
        ("setImportantForAccessibility", 4),
    ] {
        call(env, &editor, name, "(I)V", &[JValue::Int(value)])?;
    }
    call(env, &editor, "setAlpha", "(F)V", &[JValue::Float(0.01)])?;
    call(
        env,
        &editor,
        "setCursorVisible",
        "(Z)V",
        &[JValue::Bool(false)],
    )?;
    call(
        env,
        &editor,
        "setFocusableInTouchMode",
        "(Z)V",
        &[JValue::Bool(true)],
    )?;
    let filter = new(
        env,
        "android/text/InputFilter$LengthFilter",
        "(I)V",
        &[JValue::Int(4096)],
    )?;
    let filters = env.new_object_array(1, jni::jni_str!("android/text/InputFilter"), &filter)?;
    call(
        env,
        &editor,
        "setFilters",
        "([Landroid/text/InputFilter;)V",
        &[JValue::Object(&filters)],
    )?;
    let layout = new(
        env,
        "android/view/ViewGroup$LayoutParams",
        "(II)V",
        &[JValue::Int(1), JValue::Int(1)],
    )?;
    call(
        env,
        &state.object,
        "addContentView",
        "(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V",
        &[JValue::Object(&editor), JValue::Object(&layout)],
    )?;
    *lock(&state.editor) = Some(Arc::new(env.new_global_ref(editor)?));
    Ok(())
}

pub(super) fn visibility(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let Some(editor) = lock(&state.editor).clone() else {
        return Ok(());
    };
    let show = state.ime_requested.load(Ordering::Acquire) && active(state);
    if state.ime_visible.swap(show, Ordering::AcqRel) == show {
        return Ok(());
    }
    let manager = service(env, state, "input_method")?;
    if manager.is_null() {
        return Err(message("Android input method service is unavailable"));
    }
    if show {
        call(env, &editor, "setVisibility", "(I)V", &[JValue::Int(0)])?;
        call(env, &editor, "requestFocus", "()Z", &[])?;
        call(
            env,
            &manager,
            "restartInput",
            "(Landroid/view/View;)V",
            &[JValue::Object(&editor)],
        )?;
        call(
            env,
            &manager,
            "showSoftInput",
            "(Landroid/view/View;I)Z",
            &[JValue::Object(&editor), JValue::Int(1)],
        )?;
    } else {
        state.bridge.input(Input::Cancel);
        clear(env, state)?;
        let token = call(
            env,
            &editor,
            "getWindowToken",
            "()Landroid/os/IBinder;",
            &[],
        )?
        .l()?;
        call(
            env,
            &manager,
            "hideSoftInputFromWindow",
            "(Landroid/os/IBinder;I)Z",
            &[JValue::Object(&token), JValue::Int(0)],
        )?;
        call(env, &editor, "clearFocus", "()V", &[])?;
        call(env, &editor, "setVisibility", "(I)V", &[JValue::Int(4)])?;
    }
    Ok(())
}

fn clear(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    *lock(&state.dead_accent) = 0;
    let editor = editor(state)?;
    let text = editable(env, &editor)?;
    call(env, &text, "clear", "()V", &[])?;
    Ok(())
}

fn caret(state: &Activity, offset: i32) {
    if offset != 0 {
        state.bridge.input(Input::MoveCaret {
            offset: i16::try_from(offset.clamp(-4096, 4096)).unwrap_or(0),
        });
    }
}

pub(super) fn commit(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let editor = editor(state)?;
    let editable = editable(env, &editor)?;
    let value = text(env, &editable, 16_384)?;
    if !value.is_empty() {
        let end = call(env, &editor, "getSelectionEnd", "()I", &[])?.i()?;
        let length = i32::try_from(value.encode_utf16().count()).unwrap_or(4096);
        state.bridge.input(Input::Commit(value));
        caret(state, end - length);
    }
    clear(env, state)
}

fn composing(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let editor = editor(state)?;
    let editable = editable(env, &editor)?;
    let start = static_call(
        env,
        "android/view/inputmethod/BaseInputConnection",
        "getComposingSpanStart",
        "(Landroid/text/Spannable;)I",
        &[JValue::Object(&editable)],
    )?
    .i()?;
    let end = static_call(
        env,
        "android/view/inputmethod/BaseInputConnection",
        "getComposingSpanEnd",
        "(Landroid/text/Spannable;)I",
        &[JValue::Object(&editable)],
    )?
    .i()?;
    let value = if start >= 0 && end >= start {
        let region = call(
            env,
            &editable,
            "subSequence",
            "(II)Ljava/lang/CharSequence;",
            &[JValue::Int(start), JValue::Int(end)],
        )?
        .l()?;
        text(env, &region, 16_384)?
    } else {
        String::new()
    };
    let selection_start = call(env, &editor, "getSelectionStart", "()I", &[])?.i()?;
    let selection_end = call(env, &editor, "getSelectionEnd", "()I", &[])?.i()?;
    state.bridge.input(Input::Composition {
        text: value,
        selection_start: usize::try_from((selection_start - start.max(0)).max(0)).unwrap_or(0),
        selection_end: usize::try_from((selection_end - start.max(0)).max(0)).unwrap_or(0),
    });
    Ok(())
}

pub(super) fn connection<'local>(
    env: &mut Env<'local>,
    state: &Activity,
    editor: &JObject<'_>,
    attributes: &JObject<'_>,
) -> Result<JObject<'local>> {
    let base = super_call(
        env,
        editor,
        "android/widget/EditText",
        "onCreateInputConnection",
        "(Landroid/view/inputmethod/EditorInfo;)Landroid/view/inputmethod/InputConnection;",
        &[JValue::Object(attributes)],
    )?
    .l()?;
    if base.is_null() {
        return Ok(base);
    }
    *lock(&state.connection) = Some(Arc::new(env.new_global_ref(&base)?));
    new(
        env,
        "io/github/mny315/j2play/EditorConnection",
        "(Landroid/view/inputmethod/InputConnection;Lio/github/mny315/j2play/J2PlayActivity;)V",
        &[JValue::Object(&base), JValue::Object(&state.object)],
    )
}

pub(super) fn change(
    env: &mut Env<'_>,
    object: &JObject<'_>,
    name: &str,
    signature: &str,
    args: &[JValue<'_>],
    commit_text: bool,
) -> Result<bool> {
    let state = owner(env, object)?;
    let changed = super_call(
        env,
        object,
        "android/view/inputmethod/InputConnectionWrapper",
        name,
        signature,
        args,
    )?
    .z()?;
    if changed {
        if commit_text {
            commit(env, &state)?;
        } else {
            composing(env, &state)?;
        }
    }
    Ok(changed)
}

pub(super) fn delete(
    env: &mut Env<'_>,
    object: &JObject<'_>,
    before: i32,
    after: i32,
    code_points: bool,
) -> Result<bool> {
    if before < 0 || after < 0 {
        return Ok(false);
    }
    let state = owner(env, object)?;
    let editor = editor(&state)?;
    let editable = editable(env, &editor)?;
    let first = call(env, &editor, "getSelectionStart", "()I", &[])?
        .i()?
        .max(0);
    let last = call(env, &editor, "getSelectionEnd", "()I", &[])?
        .i()?
        .max(0);
    let start = first.min(last);
    let end = first.max(last);
    let length = call(env, &editable, "length", "()I", &[])?.i()?;
    let count = |env: &mut Env<'_>, start, end| -> Result<i32> {
        Ok(static_call(
            env,
            "java/lang/Character",
            "codePointCount",
            "(Ljava/lang/CharSequence;II)I",
            &[
                JValue::Object(&editable),
                JValue::Int(start),
                JValue::Int(end),
            ],
        )?
        .i()?)
    };
    let local_before = if code_points {
        count(env, 0, start)?
    } else {
        start
    };
    let local_after = if code_points {
        count(env, end, length)?
    } else {
        length - end
    };
    let name = if code_points {
        "deleteSurroundingTextInCodePoints"
    } else {
        "deleteSurroundingText"
    };
    if !super_call(
        env,
        object,
        "android/view/inputmethod/InputConnectionWrapper",
        name,
        "(II)Z",
        &[JValue::Int(before), JValue::Int(after)],
    )?
    .z()?
    {
        return Ok(false);
    }
    composing(env, &state)?;
    let before = (before - local_before).clamp(0, 64);
    let after = (after - local_after).clamp(0, 64 - before);
    if before != 0 || after != 0 {
        let event = if code_points {
            Input::DeleteSurrounding {
                before_chars: usize::try_from(before).unwrap_or(0),
                after_chars: usize::try_from(after).unwrap_or(0),
            }
        } else {
            Input::DeleteUtf16 {
                before: u16::try_from(before).unwrap_or(0),
                after: u16::try_from(after).unwrap_or(0),
            }
        };
        state.bridge.input(event);
    }
    Ok(true)
}

fn navigation(key: i32) -> Option<HostAction> {
    match key {
        19 => Some(HostAction::Up),
        20 => Some(HostAction::Down),
        21 => Some(HostAction::Left),
        22 => Some(HostAction::Right),
        23 | 66 | 160 => Some(HostAction::Fire),
        _ => None,
    }
}

pub(super) fn key(
    env: &mut Env<'_>,
    state: &Activity,
    editor: &JObject<'_>,
    key: i32,
    event: &JObject<'_>,
    method: &str,
) -> Result<bool> {
    let fallback = |env: &mut Env<'_>| -> Result<bool> {
        Ok(super_call(
            env,
            editor,
            "android/widget/EditText",
            method,
            "(ILandroid/view/KeyEvent;)Z",
            &[JValue::Int(key), JValue::Object(event)],
        )?
        .z()?)
    };
    if method == "onKeyUp" {
        return if navigation(key).is_some() {
            Ok(true)
        } else {
            fallback(env)
        };
    }
    let Some(connection) = lock(&state.connection).clone() else {
        return fallback(env);
    };
    if !active(state) || !state.ime_requested.load(Ordering::Acquire) {
        return fallback(env);
    }
    let editable = editable(env, editor)?;
    let current_text = text(env, &editable, 16_384)?;
    let meta = call(env, event, "getMetaState", "()I", &[])?.i()?;
    let unicode = call(env, event, "getUnicodeChar", "(I)I", &[JValue::Int(meta)])?.i()?;
    if method == "onKeyPreIme" {
        if navigation(key).is_none()
            && key != 67
            && key != 112
            && (current_text.is_empty() || unicode == 0)
        {
            return fallback(env);
        }
        if call(env, event, "getAction", "()I", &[])?.i()? == 1 {
            return Ok(true);
        }
    }
    if let Some(action) = navigation(key) {
        commit(env, state)?;
        state.bridge.input(Input::Key(action));
        return Ok(true);
    }
    if key == 67 || key == 112 {
        commit(env, state)?;
        state.bridge.input(Input::DeleteSurrounding {
            before_chars: usize::from(key == 67),
            after_chars: usize::from(key == 112),
        });
        return Ok(true);
    }
    if unicode == 0 {
        return fallback(env);
    }
    character(env, state, editor, &connection, unicode, &current_text)
}

fn character(
    env: &mut Env<'_>,
    state: &Activity,
    editor: &JObject<'_>,
    connection: &JObject<'_>,
    unicode: i32,
    current_text: &str,
) -> Result<bool> {
    if unicode < 0 {
        commit(env, state)?;
        let accent = unicode & i32::MAX;
        *lock(&state.dead_accent) = accent;
        if let Some(accent) = u32::try_from(accent).ok().and_then(char::from_u32) {
            let value = string(env, &accent.to_string())?;
            call(
                env,
                connection,
                "setComposingText",
                "(Ljava/lang/CharSequence;I)Z",
                &[JValue::Object(&value), JValue::Int(1)],
            )?;
            composing(env, state)?;
        }
        return Ok(true);
    }
    let Some(character) = u32::try_from(unicode)
        .ok()
        .and_then(char::from_u32)
        .filter(|_| unicode != 0)
    else {
        return Ok(false);
    };
    let mut committed = character.to_string();
    let accent = *lock(&state.dead_accent);
    if accent != 0 {
        let combined = static_call(
            env,
            "android/view/KeyEvent",
            "getDeadChar",
            "(II)I",
            &[JValue::Int(accent), JValue::Int(unicode)],
        )?
        .i()?;
        if let Some(character) = u32::try_from(combined)
            .ok()
            .and_then(char::from_u32)
            .filter(|_| combined != 0)
        {
            committed = character.to_string();
        } else if let Some(accent) = u32::try_from(accent).ok().and_then(char::from_u32) {
            committed.insert(0, accent);
        }
        clear(env, state)?;
    } else if !current_text.is_empty() {
        let first = call(env, editor, "getSelectionStart", "()I", &[])?
            .i()?
            .max(0);
        let last = call(env, editor, "getSelectionEnd", "()I", &[])?
            .i()?
            .max(0);
        let mut units: Vec<_> = current_text.encode_utf16().collect();
        let start = usize::try_from(first.min(last))
            .unwrap_or(0)
            .min(units.len());
        let end = usize::try_from(first.max(last))
            .unwrap_or(0)
            .min(units.len());
        units.splice(start..end, committed.encode_utf16());
        let replacement = string(env, &String::from_utf16_lossy(&units))?;
        call(
            env,
            connection,
            "setComposingText",
            "(Ljava/lang/CharSequence;I)Z",
            &[JValue::Object(&replacement), JValue::Int(1)],
        )?;
        let offset = i32::try_from((start + committed.encode_utf16().count()).min(units.len()))
            .unwrap_or(4096);
        call(env, editor, "setSelection", "(I)V", &[JValue::Int(offset)])?;
        composing(env, state)?;
        return Ok(true);
    } else {
        commit(env, state)?;
    }
    state.bridge.input(Input::Commit(committed));
    Ok(true)
}
