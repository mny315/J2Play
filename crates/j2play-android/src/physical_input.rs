//! Android input facts only; assignments and axis state live in frontend-core.
use frontend_core::physical_input::{GamepadButton as B, PhysicalControl as C};

pub(crate) const fn controller_source(source: u32) -> bool {
    source & 0x0000_0401 == 0x0000_0401
        || source & 0x0100_0010 == 0x0100_0010
        || source & 0x0000_0201 == 0x0000_0201
}

pub(crate) fn key_control(code: i32, source: u32) -> Option<C> {
    let button = match code {
        19 if controller_source(source) => Some(B::DpadUp),
        20 if controller_source(source) => Some(B::DpadDown),
        21 if controller_source(source) => Some(B::DpadLeft),
        22 if controller_source(source) => Some(B::DpadRight),
        23 if controller_source(source) => Some(B::South),
        96 => Some(B::South),
        97 => Some(B::East),
        99 => Some(B::West),
        100 => Some(B::North),
        102 => Some(B::LeftShoulder),
        103 => Some(B::RightShoulder),
        104 => Some(B::LeftTrigger),
        105 => Some(B::RightTrigger),
        106 => Some(B::LeftStick),
        107 => Some(B::RightStick),
        108 => Some(B::Start),
        109 => Some(B::Select),
        188..=203 => Some(B::Extra(u8::try_from(code - 187).ok()?)),
        _ => None,
    };
    if let Some(button) = button {
        return Some(C::Gamepad { button });
    }
    let usage = match code {
        29..=54 => u16::try_from(code - 25).ok()?,
        8..=16 => u16::try_from(code + 22).ok()?,
        7 => 39,
        66 => 40,
        111 => 41,
        67 => 42,
        61 => 43,
        62 => 44,
        69 => 45,
        70 => 46,
        71 => 47,
        72 => 48,
        73 => 49,
        74 => 51,
        75 => 52,
        68 => 53,
        55 => 54,
        56 => 55,
        76 => 56,
        115 => 57,
        131..=142 => u16::try_from(code - 73).ok()?,
        120 => 70,
        116 => 71,
        121 => 72,
        124 => 73,
        122 => 74,
        92 => 75,
        112 => 76,
        123 => 77,
        93 => 78,
        22 => 79,
        21 => 80,
        20 => 81,
        19 => 82,
        143 => 83,
        154 => 84,
        155 => 85,
        156 => 86,
        157 => 87,
        160 => 88,
        145..=153 => u16::try_from(code - 56).ok()?,
        144 => 98,
        158 => 99,
        113 => 224,
        59 => 225,
        57 => 226,
        117 => 227,
        114 => 228,
        60 => 229,
        58 => 230,
        118 => 231,
        _ => {
            let control = C::AndroidKey {
                code: u16::try_from(code).ok()?,
            };
            return control.valid().then_some(control);
        }
    };
    Some(C::Keyboard { usage })
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-android/physical_input/mod.rs"]
mod tests;
