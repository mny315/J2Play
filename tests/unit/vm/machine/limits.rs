use super::*;

#[test]
fn runtime_limits_reject_invalid_eager_allocation_configuration() {
    let invalid = [
        Limits {
            lcd_width: 0,
            ..Limits::default()
        },
        Limits {
            lcd_width: 4_096,
            lcd_height: 1_025,
            ..Limits::default()
        },
        Limits {
            lcd_normal_width: i32::MAX.cast_unsigned().saturating_add(1),
            lcd_normal_height: 1,
            ..Limits::default()
        },
        Limits {
            micro3d_objects: 0,
            ..Limits::default()
        },
        Limits {
            micro3d_bytes: 0,
            ..Limits::default()
        },
        Limits {
            m3g_max_lights: m3g::MAX_LIGHTS + 1,
            ..Limits::default()
        },
        Limits {
            m3g_num_texture_units: m3g::MAX_TEXTURE_UNITS + 1,
            ..Limits::default()
        },
        Limits {
            m3g_render: m3g::RenderLimits {
                max_viewport_width: 0,
                ..m3g::RenderLimits::default()
            },
            ..Limits::default()
        },
        Limits {
            lcd_width: 240,
            lcd_height: 320,
            lcd_normal_width: 320,
            lcd_normal_height: 240,
            ..Limits::default()
        },
    ];
    for limits in invalid {
        assert_eq!(limits.validate().unwrap_err().code(), "invalid-limits");
    }

    Limits {
        lcd_width: 2_048,
        lcd_height: 2_048,
        ..Limits::default()
    }
    .validate()
    .unwrap();
}
