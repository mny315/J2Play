use super::{AccentColor, Color32, MaterialTheme, PlatformThemeMode};

impl MaterialTheme {
    pub(in crate::theme) fn preset(mode: PlatformThemeMode, accent: AccentColor) -> Self {
        let (light, dark) = match accent {
            AccentColor::System | AccentColor::Blue => ([35, 92, 170], [168, 199, 250]),
            AccentColor::Teal => ([0, 105, 105], [126, 215, 207]),
            AccentColor::Green => ([41, 108, 56], [154, 217, 162]),
            AccentColor::Amber => ([128, 88, 0], [242, 193, 89]),
            AccentColor::Orange => ([156, 71, 18], [255, 181, 131]),
            AccentColor::Red => ([174, 47, 48], [255, 179, 174]),
            AccentColor::Pink => ([158, 55, 108], [246, 177, 213]),
            AccentColor::Purple => ([112, 66, 165], [214, 186, 255]),
        };
        let light = super::rgb(light);
        let dark = super::rgb(dark);
        let mut theme = Self::neutral(mode);
        theme.primary = match mode {
            PlatformThemeMode::Light => light,
            PlatformThemeMode::Dark => dark,
        };
        match mode {
            PlatformThemeMode::Light => {
                theme.primary_container = tint(Color32::WHITE, light, 30);
                theme.on_primary_container = tint(Color32::BLACK, light, 32);
                theme.secondary_container = tint(Color32::from_gray(238), light, 18);
                theme.on_secondary_container = theme.on_primary_container;
            }
            PlatformThemeMode::Dark => {
                theme.primary_container = tint(Color32::from_gray(20), light, 45);
                theme.on_primary_container = tint(Color32::WHITE, dark, 35);
                theme.secondary_container = tint(Color32::from_gray(42), light, 20);
                theme.on_secondary_container = theme.on_primary_container;
            }
        }
        // Quietly tint neutral surfaces with the selected hue as well as buttons.
        for surface in [
            &mut theme.background,
            &mut theme.surface,
            &mut theme.surface_container,
            &mut theme.surface_container_high,
            &mut theme.surface_container_highest,
        ] {
            *surface = tint(*surface, theme.primary, 3);
        }
        theme
    }
}

fn tint(base: Color32, accent: Color32, percent: u8) -> Color32 {
    let mix = |base: u8, accent: u8| {
        let percent = u16::from(percent);
        let value = (u16::from(base) * (100 - percent) + u16::from(accent) * percent) / 100;
        u8::try_from(value).unwrap_or(255)
    };
    Color32::from_rgb(
        mix(base.r(), accent.r()),
        mix(base.g(), accent.g()),
        mix(base.b(), accent.b()),
    )
}
