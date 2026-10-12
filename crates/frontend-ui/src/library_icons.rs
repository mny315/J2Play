//! Lazy library thumbnails sized to the display under one fixed pixel budget.

use super::{FrontendApp, TextureHandle, egui};
use image::{ImageReader, Limits, imageops::FilterType};
use std::io::Cursor;
use std::time::Instant;

mod loader;
pub(crate) use loader::LibraryAssetLoader;

pub(super) const ICON_EDGE: u32 = 256;
pub(super) const LIST_ICON_POINTS: f32 = 56.0;
pub(super) const TILE_ICON_POINTS: f32 = 80.0;
const ICON_MAX_ALLOC_BYTES: u64 = 4 * 1024 * 1024;
// The previous cache held 64 full 256×256 RGBA images: at most 16 MiB.
const MAX_ICON_PIXELS: usize = 64 * 256 * 256;

pub(super) struct CachedIcon {
    pub(super) texture: Option<TextureHandle>,
    pub(super) last_used: Instant,
}

impl FrontendApp {
    pub(super) fn prepare_library_icons(
        &mut self,
        ctx: &egui::Context,
        visible_count: usize,
        tiles: bool,
    ) {
        let points = if tiles {
            TILE_ICON_POINTS
        } else {
            LIST_ICON_POINTS
        };
        let edge = thumbnail_edge(visible_count, points, ctx.pixels_per_point());
        if self.library_icon_edge != edge {
            self.icons.clear();
            self.library_assets.invalidate();
            self.library_icon_edge = edge;
        }
    }

    pub(super) fn load_library_assets(
        &mut self,
        ctx: &egui::Context,
        entry_index: usize,
    ) -> Option<TextureHandle> {
        if let Err(error) = self.receive_library_assets(ctx) {
            eprintln!("j2play: platform[library-assets]: {error}");
        }
        if let Some(icon) = self.icons.get_mut(self.entries[entry_index].id()) {
            icon.last_used = Instant::now();
            return icon.texture.clone();
        }
        if let Err(error) =
            self.library_assets
                .request(&self.entries[entry_index], self.library_icon_edge, ctx)
        {
            eprintln!("j2play: platform[library-assets]: {error}");
        }
        None
    }

    fn receive_library_assets(&mut self, ctx: &egui::Context) -> Result<(), diagnostics::EmuError> {
        let Some(ready) = self.library_assets.take_ready()? else {
            return Ok(());
        };
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.id() == ready.entry_id)
        else {
            return Ok(());
        };
        if let Some(info) = ready.info {
            // Applying only metadata preserves settings changed during the read.
            let _ = self.repository.cache_game_info(entry, info);
        }
        let capacity = (MAX_ICON_PIXELS / (self.library_icon_edge as usize).pow(2))
            .min(frontend_core::MAX_LIBRARY_ENTRIES);
        if self.icons.len() >= capacity
            && let Some(evicted) = self
                .icons
                .iter()
                .min_by_key(|(_, icon)| icon.last_used)
                .map(|(id, _)| id.clone())
        {
            self.icons.remove(&evicted);
        }
        let entry_id = ready.entry_id;
        let texture = ready.image.map(|image| {
            ctx.load_texture(
                format!("library-icon-{entry_id}"),
                image,
                egui::TextureOptions::NEAREST,
            )
        });
        self.icons.insert(
            entry_id,
            CachedIcon {
                texture,
                last_used: Instant::now(),
            },
        );
        Ok(())
    }
}

// UI scale is clamped before the conversion. All library entries, including the
// extra virtualized rows, must fit at once to avoid evicting visible thumbnails.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn thumbnail_edge(visible_count: usize, points: f32, pixels_per_point: f32) -> u32 {
    let mut edge = (points * pixels_per_point).ceil().clamp(1.0, 256.0) as u32;
    edge = edge.max(1);
    let count = visible_count.min(frontend_core::MAX_LIBRARY_ENTRIES);
    // Halving makes the budget cap stable across small viewport changes.
    while count * (edge as usize).pow(2) > MAX_ICON_PIXELS {
        edge /= 2;
    }
    edge
}

fn decode_icon(bytes: &[u8], edge: u32) -> Result<egui::ColorImage, image::ImageError> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(ICON_EDGE);
    limits.max_image_height = Some(ICON_EDGE);
    limits.max_alloc = Some(ICON_MAX_ALLOC_BYTES);
    reader.limits(limits);
    let image = reader.decode()?;
    let image = if image.width() > edge || image.height() > edge {
        image.resize(edge, edge, FilterType::Nearest)
    } else {
        image
    }
    .into_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/library_icons.rs"]
mod tests;
