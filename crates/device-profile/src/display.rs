//! Evidence-backed display modes, Canvas geometry and logical fonts.

use serde::{Deserialize, Deserializer};

use crate::{Confidence, Evidence};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Dimensions {
    pub(super) width: u32,
    pub(super) height: u32,
}
impl Dimensions {
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
}

/// One evidence-backed logical display mode within a handset family.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenMode {
    pub(super) id: String,
    pub(super) fullscreen_canvas: Dimensions,
    pub(super) non_fullscreen_drawable_area: Option<Dimensions>,
    #[serde(default)]
    pub(super) rotated_non_fullscreen_drawable_area: Option<Dimensions>,
    pub(super) confidence: Confidence,
    pub(super) sources: Vec<String>,
}

impl ScreenMode {
    /// Stable mode identifier, normally its portrait full-screen dimensions.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Full-screen Canvas dimensions in the mode's declared orientation.
    #[must_use]
    pub const fn fullscreen_canvas(&self) -> Dimensions {
        self.fullscreen_canvas
    }

    /// Ordinary Canvas drawable area in the mode's declared orientation, when
    /// it was measured independently from the full-screen Canvas.
    #[must_use]
    pub const fn non_fullscreen_drawable_area(&self) -> Option<Dimensions> {
        self.non_fullscreen_drawable_area
    }

    /// Ordinary Canvas drawable area after rotating this mode, when the
    /// device UI does not use the simple transposed geometry.
    #[must_use]
    pub const fn rotated_non_fullscreen_drawable_area(&self) -> Option<Dimensions> {
        self.rotated_non_fullscreen_drawable_area
    }

    /// Confidence assigned to this display mode.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Evidence source identifiers for this display mode.
    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    pub(super) const fn fullscreen_dimensions(&self) -> (u32, u32) {
        (self.fullscreen_canvas.width, self.fullscreen_canvas.height)
    }

    pub(super) const fn non_fullscreen_dimensions(&self, transposed: bool) -> (u32, u32) {
        if transposed {
            match self.rotated_non_fullscreen_drawable_area {
                Some(dimensions) => (dimensions.width, dimensions.height),
                None => match self.non_fullscreen_drawable_area {
                    Some(dimensions) => (dimensions.height, dimensions.width),
                    None => (self.fullscreen_canvas.height, self.fullscreen_canvas.width),
                },
            }
        } else {
            match self.non_fullscreen_drawable_area {
                Some(dimensions) => (dimensions.width, dimensions.height),
                None => self.fullscreen_dimensions(),
            }
        }
    }
}

/// Device-selected line heights returned by the MIDP logical font sizes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LcdUiFontHeights {
    pub(super) small: u32,
    pub(super) medium: u32,
    pub(super) large: u32,
}

impl LcdUiFontHeights {
    #[must_use]
    pub const fn small(self) -> u32 {
        self.small
    }

    #[must_use]
    pub const fn medium(self) -> u32 {
        self.medium
    }

    #[must_use]
    pub const fn large(self) -> u32 {
        self.large
    }

    pub(super) const fn values(self) -> [u32; 3] {
        [self.small, self.medium, self.large]
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayProfile {
    pub(super) physical_width: Evidence<u32>,
    pub(super) physical_height: Evidence<u32>,
    pub(super) orientation: Evidence<Orientation>,
    pub(super) physical_colors: Evidence<u32>,
    pub(super) fullscreen_canvas_width: Evidence<u32>,
    pub(super) fullscreen_canvas_height: Evidence<u32>,
    pub(super) non_fullscreen_drawable_area: Evidence<Dimensions>,
    #[serde(default)]
    pub(super) lcd_ui_font_heights: Option<Evidence<LcdUiFontHeights>>,
    #[serde(default, deserialize_with = "deserialize_present")]
    pub(super) default_screen_mode: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present")]
    pub(super) screen_modes: Option<Vec<ScreenMode>>,
    pub(super) framebuffer_format: Evidence<String>,
}

fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

impl DisplayProfile {
    #[must_use]
    pub const fn physical_width(&self) -> &Evidence<u32> {
        &self.physical_width
    }
    #[must_use]
    pub const fn physical_height(&self) -> &Evidence<u32> {
        &self.physical_height
    }
    #[must_use]
    pub const fn orientation(&self) -> &Evidence<Orientation> {
        &self.orientation
    }
    #[must_use]
    pub const fn physical_colors(&self) -> &Evidence<u32> {
        &self.physical_colors
    }
    #[must_use]
    pub const fn fullscreen_canvas_width(&self) -> &Evidence<u32> {
        &self.fullscreen_canvas_width
    }
    #[must_use]
    pub const fn fullscreen_canvas_height(&self) -> &Evidence<u32> {
        &self.fullscreen_canvas_height
    }
    #[must_use]
    pub const fn non_fullscreen_drawable_area(&self) -> &Evidence<Dimensions> {
        &self.non_fullscreen_drawable_area
    }
    /// Evidence-backed MIDP line heights for the three logical font sizes.
    #[must_use]
    pub const fn lcd_ui_font_heights(&self) -> Option<&Evidence<LcdUiFontHeights>> {
        self.lcd_ui_font_heights.as_ref()
    }
    /// Identifier of the mode represented by the profile's base dimensions.
    #[must_use]
    pub fn default_screen_mode(&self) -> Option<&str> {
        self.default_screen_mode.as_deref()
    }
    /// Evidence-backed logical screen modes available within this family.
    #[must_use]
    pub fn screen_modes(&self) -> &[ScreenMode] {
        self.screen_modes.as_deref().unwrap_or_default()
    }
    #[must_use]
    pub const fn framebuffer_format(&self) -> &Evidence<String> {
        &self.framebuffer_format
    }

    pub(super) fn screen_mode_for_dimensions(
        &self,
        dimensions: (u32, u32),
    ) -> Option<(&ScreenMode, bool)> {
        self.screen_modes()
            .iter()
            .find(|mode| mode.fullscreen_dimensions() == dimensions)
            .map(|mode| (mode, false))
            .or_else(|| {
                self.screen_modes()
                    .iter()
                    .find(|mode| {
                        let (width, height) = mode.fullscreen_dimensions();
                        width != height && (height, width) == dimensions
                    })
                    .map(|mode| (mode, true))
            })
    }
}
