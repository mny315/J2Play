use super::{
    CONTROL_POSITION_UNITS, Color32, ControlTransform, DirectionIcon, PlatformThemeMode, Pos2,
    Rect, Vec2, VirtualControlGeometry, VirtualControlIcon, VirtualControlLayout,
    VirtualControlMetrics, egui,
};

pub(super) mod stick;

impl DirectionIcon {
    pub(super) const fn accessibility_label(self, two_key_diagonals: bool) -> &'static str {
        if two_key_diagonals {
            match self {
                Self::UpRight => return "Up right (2 + 6)",
                Self::DownRight => return "Down right (8 + 6)",
                Self::DownLeft => return "Down left (8 + 4)",
                Self::UpLeft => return "Up left (2 + 4)",
                _ => {}
            }
        }
        match self {
            Self::Up => "Up",
            Self::UpRight => "Up right (3)",
            Self::Down => "Down",
            Self::DownRight => "Down right (9)",
            Self::Left => "Left",
            Self::DownLeft => "Down left (7)",
            Self::Right => "Right",
            Self::UpLeft => "Up left (1)",
        }
    }
}

pub(super) fn virtual_control_metrics(rect: Rect) -> Option<VirtualControlMetrics> {
    if !rect.width().is_finite()
        || !rect.height().is_finite()
        || rect.width() <= 0.0
        || rect.height() <= 0.0
    {
        return None;
    }
    let keypad_scale = responsive_keypad_scale(rect.width());
    let direction_target = VIRTUAL_CONTROL_TARGET_SIZE * keypad_scale;
    let number_target = NUMBER_BUTTON_TARGET_SIZE * keypad_scale;
    let primary_width_scale =
        primary_control_width_scale(rect.width(), direction_target, SOFT_KEY_TARGET_SIZE);
    let number_width_scale = number_control_width_scale(rect.width(), number_target);
    let fixed_height = virtual_control_fixed_height();
    let requested_height = virtual_control_variable_height(
        direction_target,
        number_target,
        primary_width_scale,
        number_width_scale,
    );
    let height_scale = ((rect.height() - fixed_height) / requested_height).clamp(0.0, 1.0);
    if primary_width_scale * height_scale < MIN_VIRTUAL_CONTROL_SCALE
        || number_width_scale * height_scale < MIN_VIRTUAL_CONTROL_SCALE
    {
        return None;
    }
    let metrics = VirtualControlMetrics {
        direction: direction_target * primary_width_scale * height_scale,
        number: number_target * number_width_scale * height_scale,
        soft: SOFT_KEY_TARGET_SIZE * primary_width_scale * height_scale,
    };
    let minimum = rect.top()
        + CONTROLS_TOP_GUARD
        + metrics.soft
        + SOFT_TO_DIRECTION_GAP
        + metrics.direction * 1.5;
    let maximum =
        number_row_top_y(rect, metrics.number) - PRIMARY_TO_NUMBER_GAP - metrics.direction * 1.5;
    let direction_right = rect.left()
        + BACK_GESTURE_GUARD
        + metrics.direction * 3.0 * (1.0 + DIRECTION_PAD_RIGHT_SHIFT_FACTOR);
    let action_size = metrics.direction;
    let action_center = rect.right()
        - RIGHT_GESTURE_GUARD
        - action_size / 2.0
        - action_size * FIRE_LEFT_SHIFT_FACTOR;
    let action_left = action_center - fire_button_size(metrics.direction) / 2.0;
    (metrics.direction > 0.0
        && metrics.number > 0.0
        && metrics.soft > 0.0
        // Scaling to the available height can leave the two bounds a few
        // floating-point units apart even when the controls fit exactly.
        && minimum <= maximum + 0.01
        && direction_right <= action_left + 0.01)
        .then_some(metrics)
}

pub(super) fn virtual_control_geometry(
    rect: Rect,
    layout: &VirtualControlLayout,
) -> Option<VirtualControlGeometry> {
    control_geometry(rect, layout, false)
}

pub(super) fn portrait_virtual_control_geometry(
    rect: Rect,
    layout: &VirtualControlLayout,
) -> Option<VirtualControlGeometry> {
    control_geometry(bottom_controls_rect(rect), layout, true)
}

pub(super) fn bottom_controls_rect(mut rect: Rect) -> Rect {
    // The panel extends to the Canvas seam, but keypad positions and normalized
    // offsets belong to a fixed bottom reserve. Extra space above must not move
    // the keys when a guest frame or its presentation scale becomes smaller.
    // Enlarging the Canvas may still compress this reserve to avoid overlap.
    if rect.height() > super::GAMEPLAY_MIN_CONTROLS_HEIGHT {
        rect.min.y = rect.bottom() - super::GAMEPLAY_MIN_CONTROLS_HEIGHT;
    }
    rect
}

pub(super) fn gameplay_control_geometry(
    rect: Rect,
    layout: &VirtualControlLayout,
    canvas: Option<Rect>,
) -> Option<VirtualControlGeometry> {
    if let Some(canvas) = canvas {
        let mut geometry = virtual_control_geometry(rect, layout)?;
        split_landscape_number_keys(&mut geometry, rect, canvas, layout);
        Some(geometry)
    } else {
        portrait_virtual_control_geometry(rect, layout)
    }
}

fn control_geometry(
    rect: Rect,
    layout: &VirtualControlLayout,
    portrait: bool,
) -> Option<VirtualControlGeometry> {
    let metrics = virtual_control_metrics(rect)?;
    let mut soft_left = soft_left_center(rect, metrics);
    let mut soft_right = soft_right_center(rect, metrics);
    if portrait {
        // Keep the default soft-key row with the primary controls.
        // User offsets stay independent within the same bottom reserve.
        let y = direction_pad_center(rect, metrics).y
            - metrics.direction * 1.5
            - SOFT_TO_DIRECTION_GAP
            - metrics.soft / 2.0;
        soft_left.y = y;
        soft_right.y = y;
    }
    let (direction_pad_center, direction_button_size) = adjusted_control(
        direction_pad_center(rect, metrics),
        metrics.direction,
        layout.direction_pad,
        rect,
        1.5,
    );
    let (fire_center, fire_size) = adjusted_control(
        fire_center(rect, metrics),
        fire_button_size(metrics.direction),
        layout.fire,
        rect,
        0.5,
    );
    let (left_soft_key_center, left_soft_key_size) =
        adjusted_control(soft_left, metrics.soft, layout.left_soft_key, rect, 0.5);
    let (right_soft_key_center, right_soft_key_size) =
        adjusted_control(soft_right, metrics.soft, layout.right_soft_key, rect, 0.5);
    let number_keys = std::array::from_fn(|index| {
        adjusted_control(
            number_key_center(rect, metrics, u8::try_from(index).unwrap_or(0)),
            metrics.number,
            layout.number_keys[index],
            rect,
            0.5,
        )
    });
    Some(VirtualControlGeometry {
        corner_radius_percent: layout.corner_radius_percent,
        direction_pad_center,
        direction_button_size,
        fire_center,
        fire_size,
        left_soft_key_center,
        left_soft_key_size,
        right_soft_key_center,
        right_soft_key_size,
        number_keys,
    })
}

pub(super) const fn virtual_controls_visible(
    pointer_events: bool,
    layout: &VirtualControlLayout,
) -> bool {
    !pointer_events && layout.visible
}

pub(super) fn split_landscape_number_keys(
    geometry: &mut VirtualControlGeometry,
    controls: Rect,
    canvas: Rect,
    layout: &VirtualControlLayout,
) {
    let gap = 8.0;
    let Some(base) = virtual_control_geometry(controls, &VirtualControlLayout::default()) else {
        return;
    };
    let left_edge = (base.direction_pad_center.x + base.direction_button_size * 1.5)
        .max(base.left_soft_key_center.x + base.left_soft_key_size * 0.5);
    let right_edge = (base.fire_center.x - base.fire_size * 0.5)
        .min(base.right_soft_key_center.x - base.right_soft_key_size * 0.5);
    let left = (left_edge + gap, canvas.left() - gap);
    let right = (canvas.right() + gap, right_edge - gap);
    let width = (left.1 - left.0).min(right.1 - right.0);
    let height = (controls.height() - gap * 2.0).max(0.0);
    let size = 44.0_f32
        .min((width - gap) * 0.5)
        .min((height - gap * 2.0) / 3.0);
    // Wide guest canvases may leave no side
    // corridor. Keep their existing keypad instead of overlapping hit targets.
    if size < 24.0 {
        return;
    }
    let center_y = clamp_control_axis(
        (base.direction_pad_center.y + base.fire_center.y) * 0.5,
        controls.top() + gap + size * 1.5 + gap,
        controls.bottom() - gap - size * 1.5 - gap,
        controls.center().y,
    );
    for (index, key) in geometry.number_keys.iter_mut().enumerate() {
        let corridor = if index < 6 { left } else { right };
        let column = f32::from(u8::try_from(index % 2).unwrap_or(0));
        let row = f32::from(u8::try_from((index % 6) / 2).unwrap_or(0));
        let center = Pos2::new(
            (corridor.0 + corridor.1) * 0.5 + (column - 0.5) * (size + gap),
            center_y + (row - 1.0) * (size + gap),
        );
        *key = adjusted_control(center, size, layout.number_keys[index], controls, 0.5);
    }
}

pub(super) fn landscape_controls_fit(controls: Rect, canvas: Rect) -> bool {
    let layout = VirtualControlLayout::default();
    let Some(mut geometry) = virtual_control_geometry(controls, &layout) else {
        return false;
    };
    split_landscape_number_keys(&mut geometry, controls, canvas, &layout);
    [
        (
            geometry.direction_pad_center,
            geometry.direction_button_size * 3.0,
        ),
        (geometry.fire_center, geometry.fire_size),
        (geometry.left_soft_key_center, geometry.left_soft_key_size),
        (geometry.right_soft_key_center, geometry.right_soft_key_size),
    ]
    .into_iter()
    .chain(geometry.number_keys)
    .all(|(center, size)| !Rect::from_center_size(center, Vec2::splat(size)).intersects(canvas))
}

pub(super) fn control_corner_radius(default_radius: f32, percent: u8) -> f32 {
    default_radius * f32::from(percent) / 100.0
}

fn adjusted_control(
    default_center: Pos2,
    default_size: f32,
    transform: ControlTransform,
    bounds: Rect,
    extent_factor: f32,
) -> (Pos2, f32) {
    let requested_size = default_size * f32::from(transform.size_percent) / 100.0;
    let maximum_size = bounds.width().min(bounds.height()) / (extent_factor * 2.0);
    let size = requested_size.min(maximum_size).max(0.0);
    let offset = Vec2::new(
        bounds.width() * f32::from(transform.offset_x) / f32::from(CONTROL_POSITION_UNITS),
        bounds.height() * f32::from(transform.offset_y) / f32::from(CONTROL_POSITION_UNITS),
    );
    let requested = default_center + offset;
    let half_extent = size * extent_factor;
    let center = Pos2::new(
        clamp_control_axis(
            requested.x,
            bounds.left() + half_extent,
            bounds.right() - half_extent,
            bounds.center().x,
        ),
        clamp_control_axis(
            requested.y,
            bounds.top() + half_extent,
            bounds.bottom() - half_extent,
            bounds.center().y,
        ),
    );
    (center, size)
}

fn clamp_control_axis(value: f32, minimum: f32, maximum: f32, fallback: f32) -> f32 {
    if minimum <= maximum {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}

pub(super) const NUMBER_ROW_GAP: f32 = 2.0;
pub(super) const NUMBER_KEY_COUNT: f32 = 12.0;
pub(super) const NUMBER_ROW_GAP_COUNT: f32 = NUMBER_KEY_COUNT - 1.0;
pub(super) const NUMBER_ROW_SIDE_INSET: f32 = 4.0;
pub(super) const NUMBER_BUTTON_TARGET_SIZE: f32 = 30.0;
pub(super) const NUMBER_ROW_VISUAL_INSET: f32 = 1.0;
pub(super) const NUMBER_ROW_OUTLINE_WIDTH: f32 = 0.5;
pub(super) const VIRTUAL_CONTROL_TARGET_SIZE: f32 = 46.8;
pub(super) const SOFT_KEY_TARGET_SIZE: f32 = 48.0;
pub(super) const MIN_VIRTUAL_CONTROL_SCALE: f32 = 0.5;
pub(super) const BACK_GESTURE_GUARD: f32 = 32.0;
pub(super) const DIRECTION_PAD_RIGHT_SHIFT_FACTOR: f32 = 0.05;
pub(super) const DIRECTION_PAD_RAISE_FACTOR: f32 = 0.20;
pub(super) const CONTROLS_TOP_GUARD: f32 = 4.0;
pub(super) const PRIMARY_TO_NUMBER_GAP: f32 = 8.0;
pub(super) const SOFT_TO_DIRECTION_GAP: f32 = 8.0;
pub(super) const SOFT_KEY_EDGE_SHIFT_FACTOR: f32 = 0.25;
pub(super) const BOTTOM_GESTURE_GUARD: f32 = 24.0;
pub(super) const RIGHT_GESTURE_GUARD: f32 = 30.0;
pub(super) const VIRTUAL_BUTTON_CORNER_RADIUS: u8 = 16;
// Reference Fire size/offset and its reserved fitting envelope, in UI points.
// The envelope keeps responsive compression stable when Fire itself is resized.
const FIRE_LAYOUT_SIZE_FACTOR: f32 = 74.3337 / VIRTUAL_CONTROL_TARGET_SIZE;
pub(super) const FIRE_SIZE_FACTOR: f32 = 69.13034 / VIRTUAL_CONTROL_TARGET_SIZE;
const FIRE_LAYOUT_LEFT_SHIFT_FACTOR: f32 = 49.9077 / VIRTUAL_CONTROL_TARGET_SIZE;
pub(super) const FIRE_LEFT_SHIFT_FACTOR: f32 = 45.068_577 / VIRTUAL_CONTROL_TARGET_SIZE;
pub(super) const FIRE_RAISE_FACTOR: f32 = 0.18;
pub(super) const KEYPAD_RESPONSIVE_REFERENCE_WIDTH: f32 = 400.0;
pub(super) const KEYPAD_RESPONSIVE_MAX_SCALE: f32 = 1.20;

pub(super) fn responsive_keypad_scale(width: f32) -> f32 {
    (width / KEYPAD_RESPONSIVE_REFERENCE_WIDTH).clamp(1.0, KEYPAD_RESPONSIVE_MAX_SCALE)
}

fn number_control_width_scale(width: f32, target_size: f32) -> f32 {
    let number_capacity =
        ((width - NUMBER_ROW_SIDE_INSET * 2.0 - NUMBER_ROW_GAP * NUMBER_ROW_GAP_COUNT)
            / NUMBER_KEY_COUNT)
            .max(0.0);
    (number_capacity / target_size).clamp(0.0, 1.0)
}

fn primary_control_width_scale(width: f32, direction_target: f32, soft_target: f32) -> f32 {
    let direction_and_fire_factor = 3.0 * (1.0 + DIRECTION_PAD_RIGHT_SHIFT_FACTOR)
        + 0.5
        + FIRE_LAYOUT_LEFT_SHIFT_FACTOR
        + FIRE_LAYOUT_SIZE_FACTOR / 2.0;
    let direction_capacity =
        ((width - BACK_GESTURE_GUARD - RIGHT_GESTURE_GUARD) / direction_and_fire_factor).max(0.0);
    let soft_fixed_width = BACK_GESTURE_GUARD * 2.0 * (1.0 - SOFT_KEY_EDGE_SHIFT_FACTOR);
    let soft_scaled_width =
        soft_target + direction_target * 3.0 * (1.0 - SOFT_KEY_EDGE_SHIFT_FACTOR);
    let soft_capacity = ((width - soft_fixed_width) / soft_scaled_width).max(0.0);
    (direction_capacity / direction_target)
        .min(soft_capacity)
        .clamp(0.0, 1.0)
}

fn virtual_control_fixed_height() -> f32 {
    CONTROLS_TOP_GUARD + SOFT_TO_DIRECTION_GAP + PRIMARY_TO_NUMBER_GAP + BOTTOM_GESTURE_GUARD
}

fn virtual_control_variable_height(
    direction_target: f32,
    number_target: f32,
    primary_scale: f32,
    number_scale: f32,
) -> f32 {
    (direction_target * 3.0 + SOFT_KEY_TARGET_SIZE) * primary_scale + number_target * number_scale
}

pub(super) fn virtual_control_outline(
    mode: PlatformThemeMode,
    outline: Color32,
    background: Color32,
) -> Color32 {
    if mode == PlatformThemeMode::Light {
        return outline;
    }
    let blend = |foreground: u8, base: u8| {
        let value = u16::from(foreground) * 2 + u16::from(base) * 3;
        u8::try_from(value / 5).unwrap_or(u8::MAX)
    };
    Color32::from_rgb(
        blend(outline.r(), background.r()),
        blend(outline.g(), background.g()),
        blend(outline.b(), background.b()),
    )
}

pub(super) fn direction_arrow_segments(rect: Rect, direction: DirectionIcon) -> [[Pos2; 2]; 3] {
    let center = rect.center();
    let extent = rect.width().min(rect.height()) * 0.24;
    let head = extent * 0.58;
    let diagonal = std::f32::consts::FRAC_1_SQRT_2;
    let direction = match direction {
        DirectionIcon::Up => Vec2::new(0.0, -1.0),
        DirectionIcon::UpRight => Vec2::new(diagonal, -diagonal),
        DirectionIcon::Right => Vec2::new(1.0, 0.0),
        DirectionIcon::DownRight => Vec2::new(diagonal, diagonal),
        DirectionIcon::Down => Vec2::new(0.0, 1.0),
        DirectionIcon::DownLeft => Vec2::new(-diagonal, diagonal),
        DirectionIcon::Left => Vec2::new(-1.0, 0.0),
        DirectionIcon::UpLeft => Vec2::new(-diagonal, -diagonal),
    };
    let perpendicular = Vec2::new(-direction.y, direction.x);
    let tip = center + direction * extent;
    let tail = center - direction * extent;
    let head_center = tip - direction * head;
    [
        [tail, tip],
        [head_center - perpendicular * head, tip],
        [head_center + perpendicular * head, tip],
    ]
}

pub(super) fn paint_direction_icon(
    painter: &egui::Painter,
    rect: Rect,
    direction: DirectionIcon,
    color: Color32,
) {
    let stroke = egui::Stroke::new(
        (rect.width().min(rect.height()) * 0.065).clamp(2.0, 3.0),
        color,
    );
    for segment in direction_arrow_segments(rect, direction) {
        painter.line_segment(segment, stroke);
    }
}

pub(super) fn paint_virtual_control_icon(
    painter: &egui::Painter,
    rect: Rect,
    icon: VirtualControlIcon,
    color: Color32,
) {
    let shortest = rect.width().min(rect.height());
    let stroke = egui::Stroke::new((shortest * 0.052).clamp(2.0, 2.8), color);
    match icon {
        VirtualControlIcon::Fire => {
            let (radius, sights, dot_radius) = fire_icon_geometry(rect);
            painter.circle_stroke(rect.center(), radius, stroke);
            for segment in sights {
                painter.line_segment(segment, stroke);
            }
            painter.circle_filled(rect.center(), dot_radius, color);
        }
        VirtualControlIcon::SoftLeft | VirtualControlIcon::SoftRight => {
            let right = icon == VirtualControlIcon::SoftRight;
            let (screen, indicator) = soft_key_icon_geometry(rect, right);
            painter.rect_stroke(screen, shortest * 0.055, stroke, egui::StrokeKind::Middle);
            painter.line_segment(
                indicator,
                egui::Stroke::new((stroke.width * 1.35).min(3.2), color),
            );
        }
    }
}

pub(super) fn fire_icon_geometry(rect: Rect) -> (f32, [[Pos2; 2]; 4], f32) {
    let shortest = rect.width().min(rect.height());
    let center = rect.center();
    let radius = shortest * 0.17;
    let inner = radius * 1.28;
    let outer = radius * 1.72;
    let point = |x: f32, y: f32| center + Vec2::new(x, y);
    (
        radius,
        [
            [point(0.0, -outer), point(0.0, -inner)],
            [point(inner, 0.0), point(outer, 0.0)],
            [point(0.0, inner), point(0.0, outer)],
            [point(-outer, 0.0), point(-inner, 0.0)],
        ],
        (shortest * 0.055).max(1.5),
    )
}

pub(super) fn soft_key_icon_geometry(rect: Rect, right: bool) -> (Rect, [Pos2; 2]) {
    let shortest = rect.width().min(rect.height());
    let screen = Rect::from_center_size(rect.center(), Vec2::new(shortest * 0.45, shortest * 0.34));
    let inset = shortest * 0.065;
    let half_length = shortest * 0.075;
    let center_x = if right {
        screen.right() - inset - half_length
    } else {
        screen.left() + inset + half_length
    };
    let y = screen.bottom() - inset;
    (
        screen,
        [
            Pos2::new(center_x - half_length, y),
            Pos2::new(center_x + half_length, y),
        ],
    )
}

pub(super) fn button_rect(center_x: f32, center_y: f32, size: f32) -> Rect {
    Rect::from_center_size(Pos2::new(center_x, center_y), Vec2::splat(size))
}

pub(super) fn fire_button_size(direction_button_size: f32) -> f32 {
    direction_button_size * FIRE_SIZE_FACTOR
}

pub(super) fn number_row_width(button_size: f32) -> f32 {
    button_size * NUMBER_KEY_COUNT + NUMBER_ROW_GAP * NUMBER_ROW_GAP_COUNT
}

pub(super) fn number_button_size(button_size: f32) -> f32 {
    (button_size - NUMBER_ROW_VISUAL_INSET * 2.0).max(0.0)
}

pub(super) fn number_row_center_y(rect: Rect, button_size: f32) -> f32 {
    rect.bottom() - BOTTOM_GESTURE_GUARD - button_size / 2.0
}

pub(super) fn number_row_top_y(rect: Rect, button_size: f32) -> f32 {
    number_row_center_y(rect, button_size) - button_size / 2.0
}

pub(super) fn number_key_center(rect: Rect, metrics: VirtualControlMetrics, index: u8) -> Pos2 {
    let row_left = rect.center().x - number_row_width(metrics.number) / 2.0;
    Pos2::new(
        row_left + metrics.number / 2.0 + f32::from(index) * (metrics.number + NUMBER_ROW_GAP),
        number_row_center_y(rect, metrics.number),
    )
}

pub(super) fn primary_controls_center_y(rect: Rect, metrics: VirtualControlMetrics) -> f32 {
    let minimum = rect.top()
        + CONTROLS_TOP_GUARD
        + metrics.soft
        + SOFT_TO_DIRECTION_GAP
        + metrics.direction * 1.5;
    let maximum =
        number_row_top_y(rect, metrics.number) - PRIMARY_TO_NUMBER_GAP - metrics.direction * 1.5;
    rect.center().y.clamp(minimum, maximum.max(minimum))
}

pub(super) fn direction_pad_center(rect: Rect, metrics: VirtualControlMetrics) -> Pos2 {
    let minimum_y = rect.top()
        + CONTROLS_TOP_GUARD
        + metrics.soft
        + SOFT_TO_DIRECTION_GAP
        + metrics.direction * 1.5;
    let center_y = (primary_controls_center_y(rect, metrics)
        - metrics.direction * DIRECTION_PAD_RAISE_FACTOR)
        .max(minimum_y);
    Pos2::new(
        direction_pad_base_center_x(rect, metrics)
            + metrics.direction * 3.0 * DIRECTION_PAD_RIGHT_SHIFT_FACTOR,
        center_y,
    )
}

pub(super) fn direction_pad_base_center_x(rect: Rect, metrics: VirtualControlMetrics) -> f32 {
    rect.left() + BACK_GESTURE_GUARD + metrics.direction * 1.5
}

pub(super) fn fire_center(rect: Rect, metrics: VirtualControlMetrics) -> Pos2 {
    let action_size = metrics.direction;
    Pos2::new(
        rect.right()
            - RIGHT_GESTURE_GUARD
            - action_size / 2.0
            - action_size * FIRE_LEFT_SHIFT_FACTOR,
        primary_controls_center_y(rect, metrics) - action_size * FIRE_RAISE_FACTOR,
    )
}

pub(super) fn soft_key_center_y(rect: Rect, metrics: VirtualControlMetrics) -> f32 {
    rect.top() + CONTROLS_TOP_GUARD + metrics.soft / 2.0
}

pub(super) fn soft_left_center(rect: Rect, metrics: VirtualControlMetrics) -> Pos2 {
    let base_inset = direction_pad_base_center_x(rect, metrics) - rect.left();
    let edge_inset = base_inset * (1.0 - SOFT_KEY_EDGE_SHIFT_FACTOR);
    Pos2::new(rect.left() + edge_inset, soft_key_center_y(rect, metrics))
}

pub(super) fn soft_right_center(rect: Rect, metrics: VirtualControlMetrics) -> Pos2 {
    let left = soft_left_center(rect, metrics);
    Pos2::new(rect.right() - (left.x - rect.left()), left.y)
}
