use super::*;

fn device_with_axes(source: u32, virtual_device: bool, joystick_axes: &[i32]) -> bool {
    controller_device(source, virtual_device, |axis| {
        Ok::<_, ()>(joystick_axes.contains(&axis))
    })
    .unwrap()
}

#[test]
fn generic_sensor_axes_do_not_report_a_connected_controller() {
    for source in [
        JOYSTICK_SOURCE,
        JOYSTICK_SOURCE | 0x101,
        JOYSTICK_SOURCE | 0x1002,
    ] {
        assert!(!device_with_axes(source, false, &[]));
        assert!(!device_with_axes(source, false, &[32]));
        assert!(!device_with_axes(source, false, &[32, 33, 34]));
    }
    assert!(!device_with_axes(0x1002, false, &[0, 1]));
    assert!(!device_with_axes(0x101, false, &[0, 1]));
}

#[test]
fn controller_buttons_and_supported_joystick_axes_remain_discoverable() {
    for source in [0x401, 0x201, JOYSTICK_SOURCE | 0x401] {
        assert!(controller_device(source, false, |_| Err(())).unwrap());
        assert!(!controller_device(source, true, |_| Err(())).unwrap());
    }
    for axis in [0, 1, 11, 14, 15, 16, 17, 18, 22, 23] {
        assert!(device_with_axes(JOYSTICK_SOURCE, false, &[axis]));
        assert!(!device_with_axes(JOYSTICK_SOURCE, true, &[axis]));
    }
    assert_eq!(
        controller_device(JOYSTICK_SOURCE, false, |_| Err("range query failed")),
        Err("range query failed")
    );
}

#[test]
fn android_sources_preserve_keyboard_and_gamepad_identities() {
    assert!(!controller_source(0x1002));
    assert!(controller_source(0x0100_0411));
    assert_eq!(key_control(19, 0x101), Some(C::Keyboard { usage: 82 }));
    assert_eq!(
        key_control(19, 0x401),
        Some(C::Gamepad { button: B::DpadUp })
    );
    assert_eq!(
        key_control(96, 0x401),
        Some(C::Gamepad { button: B::South })
    );
    assert_eq!(
        key_control(104, 0x401),
        Some(C::Gamepad {
            button: B::LeftTrigger
        })
    );
    assert_eq!(key_control(29, 0x101), Some(C::Keyboard { usage: 4 }));
    assert_eq!(
        key_control(188, 0x401),
        Some(C::Gamepad {
            button: B::Extra(1)
        })
    );
    assert_eq!(
        key_control(203, 0x401),
        Some(C::Gamepad {
            button: B::Extra(16)
        })
    );
    assert_eq!(key_control(24, 0x101), Some(C::AndroidKey { code: 24 }));
    for key in [-1, 0, 3, 4, 26, 187, 219, 223, 224, 513, i32::MAX] {
        assert_eq!(key_control(key, 0x101), None);
    }
}
