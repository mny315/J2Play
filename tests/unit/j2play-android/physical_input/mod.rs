use super::*;

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
