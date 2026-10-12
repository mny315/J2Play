//! PCM mixing for bounded player and single-tone output.

use super::{Clip, OneShot, Player};

pub(super) fn mix_player(player: &Player, clip: &Clip, mixed: &mut [i32]) {
    if clip.samples.is_empty() || player.muted || player.volume == 0 {
        return;
    }
    let clip_frames = clip.samples.len();
    let plays = if player.loop_count == -1 {
        usize::MAX
    } else {
        usize::try_from(player.loop_count).unwrap_or(0)
    };
    // Short clips cannot amortize chunks. Cycle their samples without division
    // per output frame; saturated cursors retain the checked chunk path below.
    if clip_frames < 16 && player.rendered_frames.checked_add(mixed.len()).is_some() {
        let available = clip_frames.checked_mul(plays).map_or(mixed.len(), |end| {
            end.saturating_sub(player.rendered_frames).min(mixed.len())
        });
        let source = clip
            .samples
            .iter()
            .cycle()
            .skip(player.rendered_frames % clip_frames);
        for (output, sample) in mixed.iter_mut().take(available).zip(source) {
            *output = output.saturating_add(i32::from(*sample).saturating_mul(player.volume) / 100);
        }
        return;
    }
    if player.volume == 100 {
        mix_player_chunks(clip, player.rendered_frames, plays, mixed, i32::from);
    } else {
        mix_player_chunks(clip, player.rendered_frames, plays, mixed, |sample| {
            i32::from(sample).saturating_mul(player.volume) / 100
        });
    }
}

fn mix_player_chunks(
    clip: &Clip,
    mut relative: usize,
    plays: usize,
    mixed: &mut [i32],
    scale: impl Fn(i16) -> i32,
) {
    let clip_frames = clip.samples.len();
    let mut remaining = mixed;
    while !remaining.is_empty() {
        let completed = relative / clip_frames;
        if completed >= plays {
            break;
        }
        let source = relative % clip_frames;
        let frames = remaining
            .len()
            .min(clip_frames - source)
            .min((usize::MAX - relative).saturating_add(1));
        let (chunk, rest) = remaining.split_at_mut(frames);
        for (output, sample) in chunk.iter_mut().zip(&clip.samples[source..source + frames]) {
            *output = output.saturating_add(scale(*sample));
        }
        relative = relative.saturating_add(frames);
        remaining = rest;
    }
}

pub(super) fn mix_one_shot(one_shot: &OneShot, mixed: &mut [i32]) {
    if let Some(samples) = one_shot.samples.get(one_shot.rendered_frames..) {
        for (output, sample) in mixed.iter_mut().zip(samples) {
            *output = output.saturating_add(i32::from(*sample));
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/mmapi/mixing.rs"]
mod mixing_tests;
