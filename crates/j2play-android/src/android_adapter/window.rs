use super::{
    Activity, Arc, Env, JNIString, JObject, JValue, Ordering, Result, active, call, field_int,
    lock, new, service, static_call, string,
};
use frontend_core::VibrationRequest;
use frontend_ui::{
    PlatformInsets, PlatformOrientation, PlatformTheme, PlatformThemeColors, PlatformThemeMode,
};

fn decor<'local>(env: &mut Env<'local>, state: &Activity) -> Result<JObject<'local>> {
    let window = call(
        env,
        &state.object,
        "getWindow",
        "()Landroid/view/Window;",
        &[],
    )?
    .l()?;
    Ok(call(env, &window, "getDecorView", "()Landroid/view/View;", &[])?.l()?)
}

fn dark(env: &mut Env<'_>, state: &Activity) -> Result<bool> {
    let resources = call(
        env,
        &state.object,
        "getResources",
        "()Landroid/content/res/Resources;",
        &[],
    )?
    .l()?;
    let config = call(
        env,
        &resources,
        "getConfiguration",
        "()Landroid/content/res/Configuration;",
        &[],
    )?
    .l()?;
    Ok(field_int(env, &config, "uiMode")? & 0x30 == 0x20)
}

pub(super) fn configure(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let window = call(
        env,
        &state.object,
        "getWindow",
        "()Landroid/view/Window;",
        &[],
    )?
    .l()?;
    let decor = call(env, &window, "getDecorView", "()Landroid/view/View;", &[])?.l()?;
    let light = match *lock(&state.theme_mode) {
        Some(mode) => mode == PlatformThemeMode::Light,
        None => !dark(env, state)?,
    };
    let fullscreen = state.fullscreen.load(Ordering::Acquire);
    for (name, value) in [
        ("setSoftInputMode", 0x30),
        ("setStatusBarColor", 0),
        ("setNavigationBarColor", 0),
    ] {
        call(env, &window, name, "(I)V", &[JValue::Int(value)])?;
    }
    if state.api >= 30 {
        system_bars(env, &window, light, fullscreen)?;
    } else {
        let mut visibility = 0x700;
        if light {
            visibility |= 0x2000;
            if state.api >= 26 {
                visibility |= 0x10;
            }
        }
        if fullscreen {
            visibility |= 0x1006;
        }
        call(
            env,
            &decor,
            "setSystemUiVisibility",
            "(I)V",
            &[JValue::Int(visibility)],
        )?;
    }
    if state.api >= 28 {
        let attributes = call(
            env,
            &window,
            "getAttributes",
            "()Landroid/view/WindowManager$LayoutParams;",
            &[],
        )?
        .l()?;
        env.set_field(
            &attributes,
            jni::jni_str!("layoutInDisplayCutoutMode"),
            jni::jni_sig!("I"),
            JValue::Int(1),
        )?;
        call(
            env,
            &window,
            "setAttributes",
            "(Landroid/view/WindowManager$LayoutParams;)V",
            &[JValue::Object(&attributes)],
        )?;
    }
    if state.api >= 29 {
        for name in [
            "setStatusBarContrastEnforced",
            "setNavigationBarContrastEnforced",
        ] {
            call(env, &window, name, "(Z)V", &[JValue::Bool(false)])?;
        }
    }
    call(env, &decor, "requestApplyInsets", "()V", &[])?;
    Ok(())
}

fn system_bars(
    env: &mut Env<'_>,
    window: &JObject<'_>,
    light: bool,
    fullscreen: bool,
) -> Result<()> {
    call(
        env,
        window,
        "setDecorFitsSystemWindows",
        "(Z)V",
        &[JValue::Bool(false)],
    )?;
    let controller = call(
        env,
        window,
        "getInsetsController",
        "()Landroid/view/WindowInsetsController;",
        &[],
    )?
    .l()?;
    if !controller.is_null() {
        call(
            env,
            &controller,
            "setSystemBarsAppearance",
            "(II)V",
            &[JValue::Int(if light { 24 } else { 0 }), JValue::Int(24)],
        )?;
        if fullscreen {
            call(
                env,
                &controller,
                "setSystemBarsBehavior",
                "(I)V",
                &[JValue::Int(2)],
            )?;
        }
        call(
            env,
            &controller,
            if fullscreen { "hide" } else { "show" },
            "(I)V",
            &[JValue::Int(7)],
        )?;
    }
    Ok(())
}

pub(super) fn install(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let decor = decor(env, state)?;
    call(
        env,
        &decor,
        "setOnApplyWindowInsetsListener",
        "(Landroid/view/View$OnApplyWindowInsetsListener;)V",
        &[JValue::Object(&state.callbacks)],
    )?;
    call(env, &decor, "requestApplyInsets", "()V", &[])?;
    if state.api >= 33 {
        let callback = new(
            env,
            "io/github/mny315/j2play/BackCallback",
            "(Lio/github/mny315/j2play/J2PlayActivity;)V",
            &[JValue::Object(&state.object)],
        )?;
        let dispatcher = call(
            env,
            &state.object,
            "getOnBackInvokedDispatcher",
            "()Landroid/window/OnBackInvokedDispatcher;",
            &[],
        )?
        .l()?;
        call(
            env,
            &dispatcher,
            "registerOnBackInvokedCallback",
            "(ILandroid/window/OnBackInvokedCallback;)V",
            &[JValue::Int(0), JValue::Object(&callback)],
        )?;
        *lock(&state.back) = Some(Arc::new(env.new_global_ref(callback)?));
    }
    Ok(())
}

pub(super) fn uninstall(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    if let Some(callback) = lock(&state.back).take() {
        let dispatcher = call(
            env,
            &state.object,
            "getOnBackInvokedDispatcher",
            "()Landroid/window/OnBackInvokedDispatcher;",
            &[],
        )?
        .l()?;
        call(
            env,
            &dispatcher,
            "unregisterOnBackInvokedCallback",
            "(Landroid/window/OnBackInvokedCallback;)V",
            &[JValue::Object(&callback)],
        )?;
    }
    Ok(())
}

pub(super) fn insets(env: &mut Env<'_>, state: &Activity, insets: &JObject<'_>) -> Result<()> {
    let mut values = [0; 4];
    if state.api >= 30 {
        let covered = call(
            env,
            insets,
            "getInsets",
            "(I)Landroid/graphics/Insets;",
            &[JValue::Int(7 | 8 | 128)],
        )?
        .l()?;
        for (index, name) in ["left", "top", "right", "bottom"].iter().enumerate() {
            values[index] = field_int(env, &covered, name)?;
        }
    } else {
        for (index, side) in ["Left", "Top", "Right", "Bottom"].iter().enumerate() {
            values[index] = call(
                env,
                insets,
                &format!("getSystemWindowInset{side}"),
                "()I",
                &[],
            )?
            .i()?;
        }
        if state.api >= 28 {
            let cutout = call(
                env,
                insets,
                "getDisplayCutout",
                "()Landroid/view/DisplayCutout;",
                &[],
            )?
            .l()?;
            if !cutout.is_null() {
                for (index, side) in ["Left", "Top", "Right", "Bottom"].iter().enumerate() {
                    values[index] = values[index]
                        .max(call(env, &cutout, &format!("getSafeInset{side}"), "()I", &[])?.i()?);
                }
            }
        }
    }
    let values = values.map(|value| u32::try_from(value.max(0)).unwrap_or(0).min(65_535));
    *lock(&state.bridge.insets) = Some(Ok(PlatformInsets {
        left: values[0],
        top: values[1],
        right: values[2],
        bottom: values[3],
    }));
    Ok(())
}

pub(super) fn theme(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let locale = static_call(
        env,
        "java/util/Locale",
        "getDefault",
        "()Ljava/util/Locale;",
        &[],
    )?
    .l()?;
    let tag = call(env, &locale, "toLanguageTag", "()Ljava/lang/String;", &[])?.l()?;
    let tag = super::text(env, &tag, 128)?;
    *lock(&state.bridge.language) = frontend_core::Language::from_locale(&tag);
    let dark = dark(env, state)?;
    let light_colors = theme_colors(env, state, false);
    let dark_colors = theme_colors(env, state, true);
    *lock(&state.bridge.theme) = Some(Ok(PlatformTheme {
        mode: if dark {
            PlatformThemeMode::Dark
        } else {
            PlatformThemeMode::Light
        },
        light_colors,
        dark_colors,
    }));
    Ok(())
}

pub(super) fn text_scale(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let resources = call(
        env,
        &state.object,
        "getResources",
        "()Landroid/content/res/Resources;",
        &[],
    )?
    .l()?;
    let metrics = call(
        env,
        &resources,
        "getDisplayMetrics",
        "()Landroid/util/DisplayMetrics;",
        &[],
    )?
    .l()?;
    let density = env
        .get_field(&metrics, jni::jni_str!("density"), jni::jni_sig!("F"))?
        .f()?;
    if !density.is_finite() || density <= 0.0 {
        return Err(super::message("Android returned invalid display density"));
    }
    let mut points = [0.0; frontend_ui::PlatformTextScale::SAMPLE_COUNT];
    for (size, point) in (1_u16..).zip(&mut points) {
        // Android 14+ applies a size-dependent accessibility curve. Convert SP
        // through the SDK, then remove density: winit already applies that once.
        let pixels = static_call(
            env,
            "android/util/TypedValue",
            "applyDimension",
            "(IFLandroid/util/DisplayMetrics;)F",
            &[
                JValue::Int(2),
                JValue::Float(f32::from(size)),
                JValue::Object(&metrics),
            ],
        )?
        .f()?;
        *point = pixels / density;
    }
    let scale = frontend_ui::PlatformTextScale::from_points(points)
        .ok_or_else(|| super::message("Android returned invalid font scaling metrics"))?;
    *lock(&state.bridge.text_scale) = scale;
    Ok(())
}

fn theme_colors(env: &mut Env<'_>, state: &Activity, dark: bool) -> Option<PlatformThemeColors> {
    crate::android_theme::palette(state.api, dark, |name| -> Result<[u8; 3]> {
        let resource = env
            .get_static_field(
                jni::jni_str!("android/R$color"),
                JNIString::from(format!("system_{name}")),
                jni::jni_sig!("I"),
            )?
            .i()?;
        let color = call(
            env,
            &state.object,
            "getColor",
            "(I)I",
            &[JValue::Int(resource)],
        )?
        .i()?
        .to_be_bytes();
        Ok([color[1], color[2], color[3]])
    })
    .unwrap_or_else(|_| {
        env.exception_clear();
        None
    })
}

pub(super) fn orientation(
    env: &mut Env<'_>,
    state: &Activity,
    orientation: PlatformOrientation,
) -> Result<()> {
    let value = match orientation {
        PlatformOrientation::Automatic => -1,
        PlatformOrientation::Portrait => 7,
        PlatformOrientation::Landscape => 6,
    };
    call(
        env,
        &state.object,
        "setRequestedOrientation",
        "(I)V",
        &[JValue::Int(value)],
    )?;
    Ok(())
}

pub(super) fn vibration(
    env: &mut Env<'_>,
    state: &Activity,
    request: VibrationRequest,
) -> Result<()> {
    let vibrator = service(env, state, "vibrator")?;
    if vibrator.is_null() {
        return Ok(());
    }
    call(env, &vibrator, "cancel", "()V", &[])?;
    if !active(state) {
        return Ok(());
    }
    let (continuous, duration, level) = match request {
        VibrationRequest::Stop => return Ok(()),
        VibrationRequest::Continuous { level } => (true, 1000, level),
        VibrationRequest::Timed {
            duration_millis,
            level,
        } => (false, duration_millis.clamp(1, 600_000), level),
    };
    if state.api >= 26 {
        let amplitude = level.map_or(-1, |value| 1 + i32::from(value.clamp(1, 100)) * 254 / 100);
        let effect = if continuous {
            let times = env.new_long_array(2)?;
            times.set_region(env, 0, &[0, i64::try_from(duration).unwrap_or(600_000)])?;
            let levels = env.new_int_array(2)?;
            levels.set_region(env, 0, &[0, amplitude])?;
            static_call(
                env,
                "android/os/VibrationEffect",
                "createWaveform",
                "([J[II)Landroid/os/VibrationEffect;",
                &[
                    JValue::Object(&times),
                    JValue::Object(&levels),
                    JValue::Int(0),
                ],
            )?
            .l()?
        } else {
            static_call(
                env,
                "android/os/VibrationEffect",
                "createOneShot",
                "(JI)Landroid/os/VibrationEffect;",
                &[
                    JValue::Long(i64::try_from(duration).unwrap_or(600_000)),
                    JValue::Int(amplitude),
                ],
            )?
            .l()?
        };
        call(
            env,
            &vibrator,
            "vibrate",
            "(Landroid/os/VibrationEffect;)V",
            &[JValue::Object(&effect)],
        )?;
    } else if continuous {
        let times = env.new_long_array(2)?;
        times.set_region(env, 0, &[0, i64::try_from(duration).unwrap_or(600_000)])?;
        call(
            env,
            &vibrator,
            "vibrate",
            "([JI)V",
            &[JValue::Object(&times), JValue::Int(0)],
        )?;
    } else {
        call(
            env,
            &vibrator,
            "vibrate",
            "(J)V",
            &[JValue::Long(i64::try_from(duration).unwrap_or(600_000))],
        )?;
    }
    Ok(())
}

pub(super) fn haptic(env: &mut Env<'_>, state: &Activity, strength: Option<u8>) -> Result<()> {
    if !active(state) {
        return Ok(());
    }
    if let Some(level) = strength.filter(|_| state.api >= 26) {
        vibration(
            env,
            state,
            VibrationRequest::Timed {
                duration_millis: 18,
                level: Some(level),
            },
        )
    } else {
        let decor = decor(env, state)?;
        call(
            env,
            &decor,
            "performHapticFeedback",
            "(I)Z",
            &[JValue::Int(4)],
        )?;
        Ok(())
    }
}

pub(super) fn open_url(env: &mut Env<'_>, state: &Activity, url: &str) -> Result<()> {
    let url = string(env, url)?;
    let uri = static_call(
        env,
        "android/net/Uri",
        "parse",
        "(Ljava/lang/String;)Landroid/net/Uri;",
        &[JValue::Object(&url)],
    )?
    .l()?;
    let action = string(env, "android.intent.action.VIEW")?;
    let intent = new(
        env,
        "android/content/Intent",
        "(Ljava/lang/String;Landroid/net/Uri;)V",
        &[JValue::Object(&action), JValue::Object(&uri)],
    )?;
    call(
        env,
        &state.object,
        "startActivity",
        "(Landroid/content/Intent;)V",
        &[JValue::Object(&intent)],
    )?;
    Ok(())
}
