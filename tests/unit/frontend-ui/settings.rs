use super::*;

#[test]
fn settings_actions_stack_when_dpi_leaves_too_little_logical_width() {
    use crate::settings_actions::SettingsActions;

    egui::__run_test_ui(|ui| {
        for width in [144.0, 183.0, 184.0, 240.0, 279.0, 280.0, 360.0, 800.0] {
            for with_reset in [false, true] {
                let actions = SettingsActions::new(ui, width, with_reset);
                let panel = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, actions.height()));
                let buttons: Vec<_> = actions
                    .reset
                    .into_iter()
                    .chain([actions.cancel, actions.save])
                    .collect();
                for button in &buttons {
                    assert!(panel.contains_rect(*button), "{width}: {button:?}");
                    assert!(button.height() >= SETTINGS_TOUCH_TARGET_HEIGHT);
                    assert!((button.size() - actions.save.size()).length() < 0.001);
                }
                for pair in buttons.windows(2) {
                    assert!(!pair[0].intersects(pair[1]));
                    assert!(pair[0].right() <= pair[1].left() || pair[0].bottom() <= pair[1].top());
                }
                let settings = SettingsActions::new(ui, width, false);
                assert_eq!(settings.cancel.x_range(), actions.cancel.x_range());
                assert_eq!(settings.save.x_range(), actions.save.x_range());
            }
        }
    });
}

#[test]
fn slider_haptic_ticks_are_rate_limited() {
    let now = Instant::now();
    assert!(slider_haptic_due(None, now));
    assert!(!slider_haptic_due(Some(now), now));
    assert!(slider_haptic_due(
        Some(now.checked_sub(SLIDER_HAPTIC_MIN_INTERVAL).unwrap()),
        now
    ));
}

#[test]
fn vibration_slider_uses_zero_as_off_for_the_whole_keypad() {
    let mut vibration = VibrationSettings::default();
    crate::settings::apply_vibration_slider_value(&mut vibration, 0);
    assert!(!vibration.enabled);
    assert_eq!(crate::settings::vibration_slider_value(vibration), 0);

    crate::settings::apply_vibration_slider_value(&mut vibration, 37);
    assert!(vibration.enabled);
    assert_eq!(vibration.strength_percent, 37);
    assert_eq!(crate::settings::vibration_slider_value(vibration), 37);
}

#[test]
fn settings_slider_uses_available_width_except_for_its_value_field() {
    let available = 600.0;
    assert!(
        (settings_slider_width(available) - (available - SETTINGS_SLIDER_VALUE_RESERVE)).abs()
            < f32::EPSILON
    );
    assert!((settings_slider_width(40.0) - SETTINGS_TOUCH_TARGET_HEIGHT).abs() < f32::EPSILON);

    egui::__run_test_ui(|ui| {
        ui.set_width(available);
        apply_settings_style(ui);
        let mut value = 100_u16;
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = settings_slider_width(ui.available_width());
                ui.add(egui::Slider::new(&mut value, 25..=400).suffix("%"))
            })
            .inner;
        assert!(response.rect.width() > available * 0.9);
    });
}
