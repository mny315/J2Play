use super::{Activity, Env, JValue, Result, call, service, static_call, text};
use crate::physical_input::{controller_source, key_control};
use android_activity::input::{Axis, InputEvent, KeyAction, MotionAction};
use frontend_core::physical_input::{GamepadAxis, PhysicalInputEvent};
use jni::objects::JIntArray;

/// Called by winit before it translates joystick motion into touch events.
pub(crate) fn native_event(event: &InputEvent<'_>) -> bool {
    let Ok(bridge) = super::bridge() else {
        return false;
    };
    match event {
        InputEvent::KeyEvent(key) => {
            let pressed = match key.action() {
                KeyAction::Down => true,
                KeyAction::Up => false,
                _ => return false,
            };
            let code = i32::try_from(u32::from(key.key_code())).unwrap_or(-1);
            let Some(control) = key_control(code, key.source().into()) else {
                return false;
            };
            let device = u32::from_ne_bytes(key.device_id().to_ne_bytes());
            if !bridge.route_physical_key(device, control, pressed) {
                return false;
            }
            if key.repeat_count() == 0 || !pressed {
                bridge.physical_event(PhysicalInputEvent::Button {
                    device,
                    control,
                    pressed,
                });
            }
            true
        }
        InputEvent::MotionEvent(motion)
            if u32::from(motion.source()) & 0x0100_0010 == 0x0100_0010 =>
        {
            let device = u32::from_ne_bytes(motion.device_id().to_ne_bytes());
            if motion.action() == MotionAction::Cancel {
                bridge.physical_event(PhysicalInputEvent::Disconnected { device });
                return true;
            }
            if motion.pointer_count() == 0 {
                return true;
            }
            let pointer = motion.pointer_at_index(0);
            for (axis, native) in [
                (GamepadAxis::LeftX, Axis::X),
                (GamepadAxis::LeftY, Axis::Y),
                (GamepadAxis::RightX, Axis::Z),
                (GamepadAxis::RightY, Axis::Rz),
                (GamepadAxis::HatX, Axis::HatX),
                (GamepadAxis::HatY, Axis::HatY),
            ] {
                bridge.physical_event(PhysicalInputEvent::Axis {
                    device,
                    axis,
                    value: pointer.axis_value(native),
                });
            }
            for (axis, native, fallback) in [
                (GamepadAxis::LeftTrigger, Axis::Ltrigger, Axis::Brake),
                (GamepadAxis::RightTrigger, Axis::Rtrigger, Axis::Gas),
            ] {
                bridge.physical_event(PhysicalInputEvent::Axis {
                    device,
                    axis,
                    value: pointer.axis_value(native).max(pointer.axis_value(fallback)),
                });
            }
            true
        }
        _ => false,
    }
}

pub(super) fn device_changed(env: &mut Env<'_>, state: &Activity, id: i32) -> Result<()> {
    env.with_local_frame(16, |env| -> Result<()> {
        let device = static_call(
            env,
            "android/view/InputDevice",
            "getDevice",
            "(I)Landroid/view/InputDevice;",
            &[JValue::Int(id)],
        )?
        .l()?;
        let name = if device.is_null() {
            None
        } else {
            let sources = call(env, &device, "getSources", "()I", &[])?.i()?;
            if controller_source(u32::from_ne_bytes(sources.to_ne_bytes()))
                && !call(env, &device, "isVirtual", "()Z", &[])?.z()?
            {
                let name = call(env, &device, "getName", "()Ljava/lang/String;", &[])?.l()?;
                Some(text(env, &name, 512)?)
            } else {
                None
            }
        };
        state
            .bridge
            .physical_device(u32::from_ne_bytes(id.to_ne_bytes()), name);
        Ok(())
    })
}

pub(super) fn install(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let manager = service(env, state, "input")?;
    call(
        env,
        &manager,
        "registerInputDeviceListener",
        "(Landroid/hardware/input/InputManager$InputDeviceListener;Landroid/os/Handler;)V",
        &[
            JValue::Object(&state.callbacks),
            JValue::Object(&jni::objects::JObject::null()),
        ],
    )?;
    let ids = static_call(env, "android/view/InputDevice", "getDeviceIds", "()[I", &[])?.l()?;
    let ids = JIntArray::cast_local(env, ids)?;
    let count = ids.len(env)?.clamp(0, 64);
    let mut values = vec![0; count];
    ids.get_region(env, 0, &mut values)?;
    for id in values {
        device_changed(env, state, id)?;
    }
    Ok(())
}

pub(super) fn uninstall(env: &mut Env<'_>, state: &Activity) -> Result<()> {
    let manager = service(env, state, "input")?;
    call(
        env,
        &manager,
        "unregisterInputDeviceListener",
        "(Landroid/hardware/input/InputManager$InputDeviceListener;)V",
        &[JValue::Object(&state.callbacks)],
    )?;
    Ok(())
}
