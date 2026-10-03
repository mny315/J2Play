use super::{
    GAMEPLAY_MIN_CONTROLS_HEIGHT, GAMEPLAY_MIN_FRAME_HEIGHT, GameScale, GameplayLayout, Rect, Vec2,
};
use frontend_core::MAX_PORTRAIT_FRAME_PERCENT;

const MIN_RESIZED_CONTROLS_HEIGHT: f32 = GAMEPLAY_MIN_CONTROLS_HEIGHT * 0.85;

// Keep the size saved by the former in-game handle until Game settings replaces
// it. Only presentation consumes this legacy value; no drag owns guest input.
#[allow(clippy::cast_precision_loss)]
pub(super) fn portrait_layout(
    available: Rect,
    dimensions: (u32, u32),
    scale: GameScale,
    percent: Option<u8>,
) -> Option<GameplayLayout> {
    if dimensions.0 == 0 || dimensions.1 == 0 || !available.is_finite() {
        return None;
    }
    let maximum_height = (available.height() - GAMEPLAY_MIN_CONTROLS_HEIGHT)
        .min(available.width() * dimensions.1 as f32 / dimensions.0 as f32);
    if maximum_height < GAMEPLAY_MIN_FRAME_HEIGHT {
        return None;
    }
    let minimum_height = (maximum_height * 0.25).max(GAMEPLAY_MIN_FRAME_HEIGHT);
    let limit_height = (maximum_height * f32::from(MAX_PORTRAIT_FRAME_PERCENT) / 100.0)
        .min(available.height() - MIN_RESIZED_CONTROLS_HEIGHT)
        .min(available.width() * dimensions.1 as f32 / dimensions.0 as f32);
    let frame_height = percent.map_or_else(
        || {
            let previous = super::gameplay_layout(available.size(), dimensions, false, false);
            super::layout::game_canvas_rect(
                Rect::from_min_size(
                    available.min,
                    Vec2::new(available.width(), previous.game_height),
                ),
                dimensions,
                scale,
            )
            .height()
            .min(maximum_height)
        },
        |percent| (maximum_height * f32::from(percent) / 100.0).clamp(minimum_height, limit_height),
    );
    // Split spare space above the bottom keypad evenly around the guest frame.
    // Its fitted size and the keypad's bottom anchor are independent of padding.
    let game_top_padding =
        ((available.height() - GAMEPLAY_MIN_CONTROLS_HEIGHT - frame_height) * 0.5).max(0.0);
    Some(GameplayLayout {
        game_top_padding,
        game_height: frame_height,
        controls_height: available.height() - game_top_padding - frame_height,
        gap: 0.0,
        controls_overlay: false,
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/gameplay_portrait.rs"]
mod tests;
