use super::*;

#[test]
fn compressed_keypad_fits_after_fractional_viewport_offsets() {
    for width in [320.0, 453.0, 800.0, 1080.0] {
        for top in [0.0, 8.0, 111.5, 285.875] {
            let rect = Rect::from_min_size(
                Pos2::new(13.25, top),
                Vec2::new(width, GAMEPLAY_MIN_CONTROLS_HEIGHT),
            );
            let geometry = virtual_control_geometry(rect, &VirtualControlLayout::default())
                .unwrap_or_else(|| panic!("controls must fit {rect:?}"));
            assert!(rect.contains(geometry.direction_pad_center));
            assert!(rect.contains(geometry.fire_center));
            assert!(geometry.direction_button_size >= VIRTUAL_CONTROL_TARGET_SIZE * 0.5);
        }
    }
}

#[test]
fn portrait_soft_keys_stay_with_primary_controls_when_the_panel_expands() {
    let layout = VirtualControlLayout::default();
    let mut reference = None;
    for top in [0.0, 180.0, 360.0, 520.0] {
        let rect = Rect::from_min_max(Pos2::new(10.0, top), Pos2::new(463.0, 960.0));
        let geometry = portrait_virtual_control_geometry(rect, &layout).unwrap();
        let expected = reference.get_or_insert(geometry.direction_pad_center);
        assert_eq!(geometry.direction_pad_center, *expected);
        let direction_top = geometry.direction_pad_center.y - geometry.direction_button_size * 1.5;
        let soft_bottom = geometry.left_soft_key_center.y + geometry.left_soft_key_size / 2.0;
        assert!((direction_top - soft_bottom - SOFT_TO_DIRECTION_GAP).abs() < 0.01);
        assert!((geometry.left_soft_key_center.y - geometry.right_soft_key_center.y).abs() < 0.01);
        assert!(geometry.left_soft_key_center.y - geometry.left_soft_key_size / 2.0 >= rect.top());

        let landscape = virtual_control_geometry(rect, &layout).unwrap();
        assert!(
            (landscape.left_soft_key_center.y
                - rect.top()
                - CONTROLS_TOP_GUARD
                - landscape.left_soft_key_size / 2.0)
                .abs()
                < 0.01
        );
        let mut custom = layout.clone();
        custom.left_soft_key.offset_y = 500;
        custom.direction_pad.offset_y = 1_000;
        let adjusted = portrait_virtual_control_geometry(rect, &custom).unwrap();
        assert!(
            (adjusted.left_soft_key_center.y
                - geometry.left_soft_key_center.y
                - GAMEPLAY_MIN_CONTROLS_HEIGHT * 0.05)
                .abs()
                < 0.01
        );
        assert_eq!(
            adjusted.right_soft_key_center,
            geometry.right_soft_key_center
        );
    }
}

#[test]
fn portrait_keypad_spreads_controls_around_the_vertical_middle() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0));
    let metrics = virtual_control_metrics(rect).expect("regular keypad area must fit controls");
    let keypad_scale = responsive_keypad_scale(rect.width());
    assert!((metrics.direction - VIRTUAL_CONTROL_TARGET_SIZE * keypad_scale).abs() < 0.01);
    assert!((metrics.number - NUMBER_BUTTON_TARGET_SIZE * keypad_scale).abs() < 0.01);
    assert!((metrics.soft - SOFT_KEY_TARGET_SIZE).abs() < f32::EPSILON);
    assert!((fire_button_size(metrics.direction) - 69.13034 * keypad_scale).abs() < 0.01);

    let direction = direction_pad_center(rect, metrics);
    let direction_left = direction.x - metrics.direction * 1.5;
    let direction_right = direction.x + metrics.direction * 1.5;
    assert!(
        (direction_left
            - rect.left()
            - BACK_GESTURE_GUARD
            - metrics.direction * 3.0 * DIRECTION_PAD_RIGHT_SHIFT_FACTOR)
            .abs()
            < 0.01
    );
    let fire = fire_center(rect, metrics);
    let action_size = metrics.direction;
    let fire_size = fire_button_size(metrics.direction);
    let expected_fire_x =
        rect.right() - RIGHT_GESTURE_GUARD - action_size / 2.0 - 45.068_577 * keypad_scale;
    assert!((fire.x - expected_fire_x).abs() < 0.01);
    let fire_left = fire.x - fire_size / 2.0;
    assert!((fire_size - action_size * FIRE_SIZE_FACTOR).abs() < 0.01);
    assert!(
        (rect.right()
            - fire.x
            - RIGHT_GESTURE_GUARD
            - action_size / 2.0
            - action_size * FIRE_LEFT_SHIFT_FACTOR)
            .abs()
            < 0.01
    );
    assert!(direction.y <= rect.center().y);
    assert!(
        (direction.y + metrics.direction * DIRECTION_PAD_RAISE_FACTOR
            - fire.y
            - action_size * FIRE_RAISE_FACTOR)
            .abs()
            < 0.01
    );

    let soft_left = soft_left_center(rect, metrics);
    let soft_right = soft_right_center(rect, metrics);
    let direction_base_inset = BACK_GESTURE_GUARD + metrics.direction * 1.5;
    let expected_soft_inset = direction_base_inset * (1.0 - SOFT_KEY_EDGE_SHIFT_FACTOR);
    assert!((soft_left.x - rect.left() - expected_soft_inset).abs() < 0.01);
    assert!(soft_left.x < direction.x);
    assert!(soft_right.x > fire.x);
    assert!((soft_left.x - rect.left() - (rect.right() - soft_right.x)).abs() < 0.01);
    assert!(
        soft_left.y + metrics.soft / 2.0 + SOFT_TO_DIRECTION_GAP
            <= direction.y - metrics.direction * 1.5 + 0.01
    );
    assert!((soft_left.y - soft_right.y).abs() < f32::EPSILON);
    assert!(soft_left.y - metrics.soft / 2.0 >= rect.top() + CONTROLS_TOP_GUARD);

    let first_number = number_key_center(rect, metrics, 0);
    let second_number = number_key_center(rect, metrics, 1);
    let last_number = number_key_center(rect, metrics, 11);
    assert!(first_number.x - metrics.number / 2.0 >= rect.left() + NUMBER_ROW_SIDE_INSET);
    assert!(last_number.x + metrics.number / 2.0 <= rect.right() - NUMBER_ROW_SIDE_INSET);
    assert!((first_number.y - last_number.y).abs() < f32::EPSILON);
    let visual_gap = second_number.x - first_number.x - number_button_size(metrics.number);
    assert!(visual_gap > NUMBER_ROW_OUTLINE_WIDTH);
    let number_bottom = last_number.y + metrics.number / 2.0;
    assert!((rect.bottom() - number_bottom - BOTTOM_GESTURE_GUARD).abs() < 0.01);
    let number_top = number_row_top_y(rect, metrics.number);
    assert!(number_top + 0.01 >= direction.y + metrics.direction * 1.5 + PRIMARY_TO_NUMBER_GAP);
    assert!(fire_left > direction_right);
}

#[test]
fn default_control_layout_preserves_the_existing_geometry() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 350.0));
    let metrics = virtual_control_metrics(rect).unwrap();
    let default_layout = VirtualControlLayout::default();
    let geometry = virtual_control_geometry(rect, &default_layout).unwrap();

    assert!(virtual_controls_visible(false, &default_layout));
    assert!(!virtual_controls_visible(true, &default_layout));
    assert_eq!(geometry.corner_radius_percent, 100);
    assert!(
        (control_corner_radius(16.0, geometry.corner_radius_percent) - 16.0).abs() < f32::EPSILON
    );
    assert!(control_corner_radius(16.0, 0).abs() < f32::EPSILON);
    assert_eq!(
        geometry.direction_pad_center,
        direction_pad_center(rect, metrics)
    );
    assert!((geometry.direction_button_size - metrics.direction).abs() < 0.01);
    assert_eq!(geometry.fire_center, fire_center(rect, metrics));
    assert!((geometry.fire_size - fire_button_size(metrics.direction)).abs() < 0.01);
    assert_eq!(
        geometry.left_soft_key_center,
        soft_left_center(rect, metrics)
    );
    assert_eq!(
        geometry.right_soft_key_center,
        soft_right_center(rect, metrics)
    );
    for (_, _, index) in NUMBER_ROW_KEYS {
        let (center, size) = geometry.number_keys[usize::from(index)];
        assert_eq!(center, number_key_center(rect, metrics, index));
        assert!((size - metrics.number).abs() < f32::EPSILON);
    }
}

#[test]
fn hidden_control_layout_gives_the_frame_the_full_gameplay_area() {
    let layout = VirtualControlLayout {
        visible: false,
        ..VirtualControlLayout::default()
    };
    assert!(!virtual_controls_visible(false, &layout));

    let available = Vec2::new(453.0, 800.0);
    let gameplay = gameplay_layout(
        available,
        (240, 320),
        !virtual_controls_visible(false, &layout),
        false,
    );
    assert!((gameplay.game_height - available.y).abs() < f32::EPSILON);
    assert!(gameplay.controls_height.abs() < f32::EPSILON);
    assert!(gameplay.gap.abs() < f32::EPSILON);
}

#[test]
fn customized_control_geometry_scales_moves_and_stays_inside_its_area() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(453.0, 350.0));
    let default_geometry =
        virtual_control_geometry(rect, &VirtualControlLayout::default()).unwrap();
    let mut layout = VirtualControlLayout::default();
    layout.fire.offset_x = -1_000;
    layout.fire.offset_y = 1_000;
    layout.fire.size_percent = 150;
    layout.direction_pad.offset_x = CONTROL_POSITION_UNITS;
    layout.direction_pad.offset_y = -CONTROL_POSITION_UNITS;
    layout.number_keys[0].offset_x = -CONTROL_POSITION_UNITS;
    layout.number_keys[0].offset_y = CONTROL_POSITION_UNITS;
    layout.number_keys[0].size_percent = MAX_CONTROL_SIZE_PERCENT;
    let geometry = virtual_control_geometry(rect, &layout).unwrap();

    assert!(geometry.fire_center.x < default_geometry.fire_center.x);
    assert!(geometry.fire_center.y > default_geometry.fire_center.y);
    assert!((geometry.fire_size - default_geometry.fire_size * 1.5).abs() < 0.01);
    let dpad = Rect::from_center_size(
        geometry.direction_pad_center,
        Vec2::splat(geometry.direction_button_size * 3.0),
    );
    assert!(rect.contains_rect(dpad));
    for (center, size) in geometry.number_keys {
        assert!(rect.contains_rect(button_rect(center.x, center.y, size)));
    }
}

#[test]
fn reserved_keypad_area_fits_enlarged_primary_controls_on_narrow_screens() {
    let width = 320.0;
    let height = GAMEPLAY_MIN_CONTROLS_HEIGHT;
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
    let metrics = virtual_control_metrics(rect).expect("minimum keypad area must fit controls");
    assert!(metrics.direction >= VIRTUAL_CONTROL_TARGET_SIZE * MIN_VIRTUAL_CONTROL_SCALE);
    assert!(metrics.direction <= VIRTUAL_CONTROL_TARGET_SIZE);
    assert!(metrics.soft <= SOFT_KEY_TARGET_SIZE);
    assert!(metrics.number < NUMBER_BUTTON_TARGET_SIZE);

    let direction = direction_pad_center(rect, metrics);
    let soft = soft_left_center(rect, metrics);
    assert!(soft.y - metrics.soft / 2.0 >= rect.top() + CONTROLS_TOP_GUARD);
    assert!(
        soft.y + metrics.soft / 2.0 + SOFT_TO_DIRECTION_GAP
            <= direction.y - metrics.direction * 1.5 + 0.01
    );
    assert!(
        direction.y + metrics.direction * 1.5 + PRIMARY_TO_NUMBER_GAP
            <= number_row_top_y(rect, metrics.number) + 0.01
    );
}

#[test]
fn virtual_controls_follow_android_density_until_the_viewport_needs_compression() {
    // Leave enough room for the enlarged targets at either density.
    let physical_size = Vec2::new(2_160.0, 2_800.0);
    let regular_density = 408.0 / 160.0;
    let high_density = 560.0 / 160.0;
    let metrics_at = |pixels_per_point: f32| {
        virtual_control_metrics(Rect::from_min_size(
            Pos2::ZERO,
            physical_size / pixels_per_point,
        ))
        .unwrap()
    };
    let regular = metrics_at(regular_density);
    let high = metrics_at(high_density);

    assert!(high.direction * high_density > regular.direction * regular_density);
    assert!(high.number * high_density > regular.number * regular_density);
    assert!(high.soft * high_density > regular.soft * regular_density);
}

#[test]
fn wide_low_density_viewports_enlarge_keypad_controls_with_a_bounded_scale() {
    let metrics_at = |width| {
        virtual_control_metrics(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 400.0))).unwrap()
    };
    let reference = metrics_at(KEYPAD_RESPONSIVE_REFERENCE_WIDTH);
    let wide = metrics_at(KEYPAD_RESPONSIVE_REFERENCE_WIDTH * 1.5);

    assert!(reference.direction <= VIRTUAL_CONTROL_TARGET_SIZE);
    assert!((reference.number - NUMBER_BUTTON_TARGET_SIZE).abs() < f32::EPSILON);
    assert!(
        (wide.direction - VIRTUAL_CONTROL_TARGET_SIZE * KEYPAD_RESPONSIVE_MAX_SCALE).abs() < 0.01
    );
    assert!((wide.number / reference.number - KEYPAD_RESPONSIVE_MAX_SCALE).abs() < 0.01);
    assert!(wide.soft >= reference.soft);
}

#[test]
fn direction_pad_hit_regions_map_arrows_and_diagonals_to_phone_actions() {
    let scratch = test_storage::Scratch::new();
    let mut app = FrontendApp::new(&scratch.0, Box::new(UnavailablePlatformBridge)).unwrap();
    let geometry = virtual_control_geometry(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)),
        &VirtualControlLayout::default(),
    )
    .unwrap();
    egui::__run_test_ui(|ui| app.draw_direction_pad(ui, geometry, false));
    assert_eq!(app.control_regions.len(), 8);
    for (column, row, action) in [
        (-1.0, -1.0, HostAction::Num1),
        (0.0, -1.0, HostAction::Up),
        (1.0, -1.0, HostAction::Num3),
        (-1.0, 0.0, HostAction::Left),
        (1.0, 0.0, HostAction::Right),
        (-1.0, 1.0, HostAction::Num7),
        (0.0, 1.0, HostAction::Down),
        (1.0, 1.0, HostAction::Num9),
    ] {
        let point =
            geometry.direction_pad_center + Vec2::new(column, row) * geometry.direction_button_size;
        assert_eq!(
            app.action_at(point),
            Some(crate::TouchOwner::VirtualKey(Some(action)))
        );
    }
    assert_eq!(app.action_at(geometry.direction_pad_center), None);
}

#[test]
fn constrained_keypad_area_is_rejected_without_invalid_clamp_bounds() {
    for size in [
        Vec2::ZERO,
        Vec2::new(1.0, 1.0),
        Vec2::new(320.0, 40.0),
        Vec2::new(80.0, GAMEPLAY_MIN_CONTROLS_HEIGHT),
    ] {
        assert!(virtual_control_metrics(Rect::from_min_size(Pos2::ZERO, size)).is_none());
    }
}

#[test]
fn direction_icons_have_geometry_without_font_glyphs() {
    let rect = Rect::from_center_size(Pos2::ZERO, Vec2::splat(44.0));
    let up = direction_arrow_segments(rect, DirectionIcon::Up);
    assert!(up[0][1].y < up[0][0].y);
    assert!(up[1][0].x < up[0][1].x);
    assert!(up[2][0].x > up[0][1].x);

    let right = direction_arrow_segments(rect, DirectionIcon::Right);
    assert!(right[0][1].x > right[0][0].x);
    assert!(right[1][0].y < right[0][1].y);
    assert!(right[2][0].y > right[0][1].y);

    let up_right = direction_arrow_segments(rect, DirectionIcon::UpRight);
    assert!(up_right[0][1].x > up_right[0][0].x);
    assert!(up_right[0][1].y < up_right[0][0].y);
}

#[test]
fn fire_and_soft_key_icons_have_mirrored_vector_geometry() {
    let rect = Rect::from_center_size(Pos2::ZERO, Vec2::splat(52.0));
    let (radius, sights, dot_radius) = fire_icon_geometry(rect);
    assert!(radius > dot_radius);
    assert!(sights[0][0].y < sights[0][1].y);
    assert!(sights[1][0].x < sights[1][1].x);

    let (left_screen, left_indicator) = soft_key_icon_geometry(rect, false);
    let (right_screen, right_indicator) = soft_key_icon_geometry(rect, true);
    assert_eq!(left_screen, right_screen);
    assert!(left_indicator[1].x < rect.center().x);
    assert!(right_indicator[0].x > rect.center().x);
    assert!(
        (left_indicator[0].x + right_indicator[1].x - rect.center().x * 2.0).abs() < f32::EPSILON
    );
}

#[test]
fn virtual_control_outlines_only_mute_the_dark_theme() {
    let light = MaterialTheme::fallback(PlatformThemeMode::Light);
    assert_eq!(
        virtual_control_outline(light.mode, light.outline, light.surface_container_high),
        light.outline
    );

    let dark = MaterialTheme::fallback(PlatformThemeMode::Dark);
    let muted = virtual_control_outline(dark.mode, dark.outline, dark.surface_container_high);
    assert_ne!(muted, dark.outline);
    assert_ne!(muted, dark.surface_container_high);
}
