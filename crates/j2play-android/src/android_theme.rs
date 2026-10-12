use frontend_ui::PlatformThemeColors;

/// Android 12 exposes neutral/accent tones; error tones became public in API 35.
/// Keep the dynamic palette on older systems using the standard error colors.
pub(crate) fn palette<E>(
    api: i32,
    dark: bool,
    mut read: impl FnMut(&str) -> Result<[u8; 3], E>,
) -> Result<Option<PlatformThemeColors>, E> {
    if api < 31 {
        return Ok(None);
    }
    let names = if dark {
        [
            "neutral1_900",
            "neutral1_900",
            "neutral1_800",
            "neutral1_800",
            "neutral1_700",
            "accent1_200",
            "accent1_800",
            "accent1_700",
            "accent1_100",
            "accent2_700",
            "accent2_100",
            "neutral1_100",
            "neutral2_200",
            "neutral2_400",
            "neutral2_700",
            "error_200",
            "error_700",
            "error_100",
        ]
    } else {
        [
            "neutral1_10",
            "neutral1_10",
            "neutral1_50",
            "neutral1_100",
            "neutral1_100",
            "accent1_600",
            "accent1_0",
            "accent1_100",
            "accent1_900",
            "accent2_100",
            "accent2_900",
            "neutral1_900",
            "neutral2_700",
            "neutral2_500",
            "neutral2_200",
            "error_600",
            "error_100",
            "error_900",
        ]
    };
    let errors = if dark {
        [[255, 180, 171], [147, 0, 10], [255, 218, 214]]
    } else {
        [[186, 26, 26], [255, 218, 214], [65, 0, 2]]
    };
    let mut colors = [[0; 3]; 18];
    for (index, name) in names.iter().enumerate() {
        colors[index] = if api < 35 && index >= 15 {
            errors[index - 15]
        } else {
            read(name)?
        };
    }
    Ok(Some(PlatformThemeColors {
        background: colors[0],
        surface: colors[1],
        surface_container: colors[2],
        surface_container_high: colors[3],
        surface_container_highest: colors[4],
        primary: colors[5],
        on_primary: colors[6],
        primary_container: colors[7],
        on_primary_container: colors[8],
        secondary_container: colors[9],
        on_secondary_container: colors[10],
        on_surface: colors[11],
        on_surface_variant: colors[12],
        outline: colors[13],
        outline_variant: colors[14],
        error: colors[15],
        error_container: colors[16],
        on_error_container: colors[17],
    }))
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-android/android_theme/mod.rs"]
mod tests;
