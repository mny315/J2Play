//! Lighting constants prepared once for a Figure or primitive batch.

use super::{EffectState, LightState, Vector3D};

pub(super) struct Lighting {
    direction: Vector3D,
    directional_intensity: i64,
    ambient_intensity: i64,
    toon: Option<[i64; 3]>,
}

impl Lighting {
    pub(super) fn new(mut light: LightState, effect: EffectState) -> Self {
        light.direction.unit();
        Self {
            direction: light.direction,
            directional_intensity: i64::from(light.directional_intensity),
            ambient_intensity: i64::from(light.ambient_intensity),
            toon: (effect.shading == 1).then(|| {
                [
                    i64::from(effect.toon_threshold.clamp(0, 255)),
                    i64::from(effect.toon_high.clamp(0, 255)) * 4096 / 255,
                    i64::from(effect.toon_low.clamp(0, 255)) * 4096 / 255,
                ]
            }),
        }
    }

    pub(super) fn shade(&self, color: u32, normal: Vector3D) -> u32 {
        let diffuse = (i64::from(normal.inner(self.direction)) / 4096).max(0);
        let directional_level = match self.toon {
            Some([threshold, high, low]) => {
                if diffuse * 255 / 4096 >= threshold {
                    high
                } else {
                    low
                }
            }
            None => diffuse,
        };
        let intensity = self
            .ambient_intensity
            .saturating_add(self.directional_intensity.saturating_mul(directional_level) / 4096);
        let channel = |shift: u32| {
            let source = i64::from((color >> shift) & 0xff_u32);
            ((source.saturating_mul(intensity) + 2048) / 4096).clamp(0, 255) as u32
        };
        (color & 0xff00_0000) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
    }
}
