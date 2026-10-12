//! Product navigation uses stable rows/columns, including scrolled-out controls.

mod popup;
pub(super) use popup::{ComboMenu, active_layer, close_popup};

use super::egui::{self, FocusDirection, Id, LayerId, Rect};

const TARGETS_ID: &str = "product-navigation-targets";
const MAX_TARGETS: usize = 1024;

#[derive(Clone, Copy)]
struct Target {
    id: Id,
    rect: Rect,
    layer: LayerId,
    scroll: Option<Id>,
}

#[derive(Clone, Copy)]
struct Lane {
    focus: Id,
    vertical: bool,
    coordinate: f32,
}

#[derive(Clone, Default)]
struct Targets {
    pass: u64,
    items: Vec<Target>,
    lane: Option<Lane>,
}

pub(super) fn register(ui: &egui::Ui, response: &egui::Response) {
    if !response.enabled() || !response.sense.is_focusable() {
        return;
    }
    let target = Target {
        id: response.id,
        rect: response.rect,
        layer: response.layer_id,
        scroll: ui
            .stack()
            .iter()
            .find(|node| node.kind() == Some(egui::UiKind::ScrollArea))
            .map(|node| node.id),
    };
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|data| {
        let targets = data.get_temp_mut_or_default::<Targets>(Id::new(TARGETS_ID));
        if targets.pass != pass {
            targets.items.clear();
            targets.pass = pass;
        }
        if let Some(existing) = targets.items.iter_mut().find(|old| old.id == target.id) {
            *existing = target;
        } else if targets.items.len() < MAX_TARGETS {
            targets.items.push(target);
        }
    });
}

pub(super) fn restore_focus(ctx: &egui::Context, preferred: Option<Id>) {
    let Some(targets) = ctx.data(|data| data.get_temp::<Targets>(Id::new(TARGETS_ID))) else {
        return;
    };
    if targets.pass != ctx.cumulative_pass_nr() {
        return;
    }
    let popup_layer = popup::active_layer(ctx);
    let eligible = |target: &&Target| {
        popup_layer.is_none_or(|layer| target.layer == layer)
            && ctx.memory(|memory| memory.allows_interaction(target.layer))
    };
    let target = targets
        .items
        .iter()
        .filter(eligible)
        .find(|target| Some(target.id) == preferred)
        .or_else(|| targets.items.iter().find(eligible));
    if let Some(target) = target {
        ctx.memory_mut(|memory| memory.request_focus(target.id));
        ctx.request_repaint();
    }
}

fn score(current: Rect, candidate: Rect, direction: FocusDirection, lane: f32) -> Option<[f32; 4]> {
    let (gap, range) = match direction {
        FocusDirection::Up => (current.top() - candidate.bottom(), candidate.x_range()),
        FocusDirection::Down => (candidate.top() - current.bottom(), candidate.x_range()),
        FocusDirection::Left => (current.left() - candidate.right(), candidate.y_range()),
        FocusDirection::Right => (candidate.left() - current.right(), candidate.y_range()),
        _ => return None,
    };
    // A control in the same row is not a Down/Up destination, even if its
    // height or baseline differs slightly. Left/Right follows the same rule.
    if gap < -0.5 {
        return None;
    }
    let aligned = range.contains(lane);
    if matches!(direction, FocusDirection::Left | FocusDirection::Right) && !aligned {
        return None;
    }
    Some([
        // Visit the nearest row before preserving the column. Prioritizing
        // alignment first skips short rows in favor of distant wide buttons.
        gap.max(0.0),
        if aligned { 0.0 } else { 1.0 },
        (lane - lane.clamp(range.min, range.max)).abs(),
        (range.center() - lane).abs(),
    ])
}

pub(super) fn move_focus(ctx: &egui::Context, direction: FocusDirection) -> bool {
    let Some(id) = ctx.memory(egui::Memory::focused) else {
        return false;
    };
    let Some(targets) = ctx.data(|data| data.get_temp::<Targets>(Id::new(TARGETS_ID))) else {
        return false;
    };
    if ctx.cumulative_pass_nr().saturating_sub(targets.pass) > 1 {
        return false;
    }
    let Some(mut current) = targets.items.iter().find(|target| target.id == id).copied() else {
        return false;
    };
    if let Some(response) = ctx.read_response(id) {
        // A combined slider response can cover both the rail and numeric field;
        // start from the actual focused subwidget, not their combined bounds.
        current.rect = response.rect;
    }
    let popup_layer = popup::active_layer(ctx);
    if popup_layer.is_some_and(|layer| layer != current.layer)
        || !ctx.memory(|memory| memory.allows_interaction(current.layer))
    {
        return false;
    }
    let vertical = matches!(direction, FocusDirection::Up | FocusDirection::Down);
    let coordinate = targets
        .lane
        .filter(|lane| lane.focus == id && lane.vertical == vertical)
        .map_or_else(
            || {
                if vertical {
                    // Start at the leading edge of a control. The center of a
                    // full-width field points at the right-hand choice below.
                    current.rect.left()
                } else {
                    current.rect.center().y
                }
            },
            |lane| lane.coordinate,
        );
    let mut best: Option<(Target, [f32; 5])> = None;
    for target in targets
        .items
        .iter()
        .copied()
        .filter(|target| target.id != id && target.layer == current.layer)
    {
        let Some(rank) = score(current.rect, target.rect, direction, coordinate) else {
            continue;
        };
        // Finish a scrolling page before moving to its fixed footer. This also
        // makes the next offscreen row reachable instead of skipping to Save.
        let rank = [
            if target.scroll == current.scroll {
                0.0
            } else {
                1.0
            },
            rank[0],
            rank[1],
            rank[2],
            rank[3],
        ];
        if best.is_none_or(|(_, old)| {
            rank.into_iter()
                .zip(old)
                .map(|(a, b)| a.total_cmp(&b))
                .find(|order| !order.is_eq())
                .is_some_and(std::cmp::Ordering::is_lt)
        }) {
            best = Some((target, rank));
        }
    }
    if best.is_some_and(|(target, _)| {
        ctx.read_response(target.id).is_some_and(|response| {
            response.sense.senses_drag()
                && response.rect != target.rect
                && response.interact_rect.contains_rect(response.rect)
        })
    }) {
        // egui owns the internal IDs of composite controls. Let its navigation
        // choose between the slider rail and numeric field instead of forcing
        // the combined response's ID onto both halves. An offscreen slider must
        // first receive focus and scroll into view; egui's visible-only search
        // would otherwise jump straight to the fixed footer on short screens.
        return false;
    }
    ctx.memory_mut(|memory| {
        // Suppress egui's cone search even at the end of a row/list.
        memory.move_focus(FocusDirection::None);
        if let Some((target, _)) = best {
            memory.request_focus(target.id);
        }
    });
    if let Some((target, _)) = best {
        let range = if vertical {
            target.rect.x_range()
        } else {
            target.rect.y_range()
        };
        ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<Targets>(Id::new(TARGETS_ID))
                .lane = Some(Lane {
                focus: target.id,
                vertical,
                coordinate: coordinate.clamp(range.min, range.max),
            });
        });
    }
    ctx.request_repaint();
    true
}

pub(super) fn keyboard(ctx: &egui::Context) {
    if ctx.text_edit_focused() {
        return;
    }
    for (key, direction) in [
        (egui::Key::ArrowUp, FocusDirection::Up),
        (egui::Key::ArrowDown, FocusDirection::Down),
        (egui::Key::ArrowLeft, FocusDirection::Left),
        (egui::Key::ArrowRight, FocusDirection::Right),
    ] {
        if matches!(key, egui::Key::ArrowLeft | egui::Key::ArrowRight)
            && ctx
                .memory(egui::Memory::focused)
                .and_then(|id| ctx.read_response(id))
                .is_some_and(|response| response.sense.senses_drag())
        {
            continue;
        }
        if ctx.input(|input| input.key_pressed(key) && input.modifiers.is_none())
            && move_focus(ctx, direction)
        {
            ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, key);
            });
        }
    }
}
