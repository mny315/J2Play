//! Receive local JARs from file managers and share sheets, on cold and warm starts.

use super::{
    Activity, Arc, Env, JObject, JValue, Result, call, document, message, new, platform_error,
    string, text,
};
use frontend_ui::DocumentKind;

pub(super) fn receive(env: &mut Env<'_>, state: &Arc<Activity>, intent: &JObject<'_>) {
    if let Err(error) = receive_inner(env, state, intent) {
        env.exception_clear();
        state
            .bridge
            .error(platform_error("document-open", error.to_string()));
    }
}

fn receive_inner(env: &mut Env<'_>, state: &Arc<Activity>, intent: &JObject<'_>) -> Result<()> {
    if intent.is_null() {
        return Ok(());
    }
    let action = call(env, intent, "getAction", "()Ljava/lang/String;", &[])?.l()?;
    if action.is_null() {
        return Ok(());
    }
    let action = text(env, &action, 128)?;
    if !matches!(
        action.as_str(),
        "android.intent.action.VIEW" | "android.intent.action.SEND"
    ) {
        return Ok(());
    }
    // Do not replay the original launch intent after Activity recreation.
    let empty = new(env, "android/content/Intent", "()V", &[])?;
    call(
        env,
        &state.object,
        "setIntent",
        "(Landroid/content/Intent;)V",
        &[JValue::Object(&empty)],
    )?;
    if let Err(error) = state.bridge.begin_external_document() {
        state.bridge.error(error);
        return Ok(());
    }
    let result = (|| {
        let uri = if action == "android.intent.action.VIEW" {
            call(env, intent, "getData", "()Landroid/net/Uri;", &[])?.l()?
        } else {
            let key = string(env, "android.intent.extra.STREAM")?;
            let stream = call(
                env,
                intent,
                "getParcelableExtra",
                "(Ljava/lang/String;)Landroid/os/Parcelable;",
                &[JValue::Object(&key)],
            )?
            .l()?;
            if stream.is_null() {
                clip_uri(env, intent)?
            } else {
                stream
            }
        };
        if uri.is_null() || !env.is_instance_of(&uri, jni::jni_str!("android/net/Uri"))? {
            return Err(message("The sender did not provide a readable JAR file"));
        }
        let scheme = call(env, &uri, "getScheme", "()Ljava/lang/String;", &[])?.l()?;
        if scheme.is_null() || !matches!(text(env, &scheme, 32)?.as_str(), "content" | "file") {
            return Err(message("Only local content and file URIs can be imported"));
        }
        document::start(env, state, DocumentKind::Jar, &uri)
    })();
    if result.is_err() {
        env.exception_clear();
        // Provider exceptions may contain private URI/path information.
        state.bridge.document_result(Err(platform_error(
            "document-open",
            "Could not read the shared JAR file. Open it again from the file manager.",
        )));
    }
    Ok(())
}

fn clip_uri<'local>(env: &mut Env<'local>, intent: &JObject<'_>) -> Result<JObject<'local>> {
    let clip = call(
        env,
        intent,
        "getClipData",
        "()Landroid/content/ClipData;",
        &[],
    )?
    .l()?;
    if clip.is_null() || call(env, &clip, "getItemCount", "()I", &[])?.i()? != 1 {
        return Err(message("Share one JAR file at a time"));
    }
    let item = call(
        env,
        &clip,
        "getItemAt",
        "(I)Landroid/content/ClipData$Item;",
        &[JValue::Int(0)],
    )?
    .l()?;
    call(env, &item, "getUri", "()Landroid/net/Uri;", &[])?
        .l()
        .map_err(Into::into)
}
