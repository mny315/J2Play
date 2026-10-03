use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, Vec2};
use image::codecs::png::PngDecoder;
use image::{ColorType, ImageDecoder, Limits};
use std::io::{Cursor, Error, ErrorKind};
use std::time::{Duration, Instant};

const SPLASH_BYTES: &[u8] = include_bytes!("../assets/startup-emblem.png");
const SPLASH_WIDTH: u32 = 1_254;
const SPLASH_HEIGHT: u32 = 1_254;
const SPLASH_MAX_COMPRESSED_BYTES: usize = 1024 * 1024;
const SPLASH_MAX_ALLOC_BYTES: u64 = 8 * 1024 * 1024;
const SPLASH_DURATION: Duration = Duration::from_millis(1_400);
const SPLASH_FADE_IN_SECONDS: f32 = 0.38;
const SPLASH_FADE_OUT_SECONDS: f32 = 0.18;
const SPLASH_VIEWPORT_FRACTION: f32 = 0.68;
const SPLASH_MAX_LOGICAL_SIZE: f32 = 420.0;
const SPLASH_MAX_FRAME_TIME: Duration = Duration::from_millis(100);

pub(super) struct StartupSplash {
    pending_image: Option<egui::ColorImage>,
    texture: Option<TextureHandle>,
    elapsed: Duration,
    last_paint: Option<Instant>,
}

impl StartupSplash {
    pub(super) fn load() -> Result<Self, image::ImageError> {
        if SPLASH_BYTES.len() > SPLASH_MAX_COMPRESSED_BYTES {
            return Err(invalid_splash(
                "embedded splash exceeds its compressed-byte bound",
            ));
        }

        let mut decoder = PngDecoder::new(Cursor::new(SPLASH_BYTES))?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(SPLASH_WIDTH);
        limits.max_image_height = Some(SPLASH_HEIGHT);
        limits.max_alloc = Some(SPLASH_MAX_ALLOC_BYTES);
        decoder.set_limits(limits)?;
        if decoder.dimensions() != (SPLASH_WIDTH, SPLASH_HEIGHT) {
            return Err(invalid_splash("embedded splash has unexpected dimensions"));
        }
        if decoder.color_type() != ColorType::Rgba8 {
            return Err(invalid_splash(
                "embedded splash must have a transparent RGBA canvas",
            ));
        }

        let decoded_bytes = decoder.total_bytes();
        if decoded_bytes > SPLASH_MAX_ALLOC_BYTES {
            return Err(invalid_splash(
                "embedded splash exceeds its decoded-byte bound",
            ));
        }
        let allocation = usize::try_from(decoded_bytes)
            .map_err(|_| invalid_splash("embedded splash allocation cannot fit usize"))?;
        let mut bytes = vec![0; allocation];
        decoder.read_image(&mut bytes)?;
        let pixels = bytes
            .chunks_exact(4)
            .map(|pixel| Color32::from_rgba_unmultiplied(pixel[0], pixel[1], pixel[2], pixel[3]))
            .collect();
        let width = usize::try_from(SPLASH_WIDTH)
            .map_err(|_| invalid_splash("embedded splash width cannot fit usize"))?;
        let height = usize::try_from(SPLASH_HEIGHT)
            .map_err(|_| invalid_splash("embedded splash height cannot fit usize"))?;

        Ok(Self {
            pending_image: Some(egui::ColorImage::new([width, height], pixels)),
            texture: None,
            elapsed: Duration::ZERO,
            last_paint: None,
        })
    }

    pub(super) fn update(&mut self, ctx: &egui::Context) -> bool {
        if self.elapsed >= SPLASH_DURATION {
            self.texture = None;
            return false;
        }
        // Steam can keep rendering an unfocused window behind its launch UI;
        // eframe can also tick logic without drawing a hidden/minimized window.
        if ctx.input(|input| input.focused && input.viewport().visible().unwrap_or(true)) {
            ctx.request_repaint();
        } else {
            self.last_paint = None;
        }
        if let Some(image) = self.pending_image.take() {
            self.texture = Some(ctx.load_texture(
                "j2play-startup-emblem",
                image,
                egui::TextureOptions::LINEAR,
            ));
        }
        true
    }

    pub(super) fn paint(&mut self, ui: &egui::Ui, background: Color32) {
        if ui.input(|input| input.focused && input.viewport().visible().unwrap_or(true)) {
            self.advance(Instant::now());
        }
        let viewport = ui.max_rect();
        ui.painter().rect_filled(viewport, 0.0, background);
        let Some(texture) = &self.texture else {
            return;
        };
        let (scale, opacity) = splash_animation(self.elapsed);
        let available = viewport.width().min(viewport.height());
        let side = (available * SPLASH_VIEWPORT_FRACTION).min(SPLASH_MAX_LOGICAL_SIZE) * scale;
        let destination = Rect::from_center_size(viewport.center(), Vec2::splat(side));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        ui.painter().image(
            texture.id(),
            destination,
            uv,
            Color32::WHITE.gamma_multiply(opacity),
        );
    }

    fn advance(&mut self, now: Instant) {
        if let Some(previous) = self.last_paint.replace(now) {
            // A blocked first present or a long compositor pause must not
            // consume the whole animation between two displayed frames.
            self.elapsed = (self.elapsed
                + now
                    .saturating_duration_since(previous)
                    .min(SPLASH_MAX_FRAME_TIME))
            .min(SPLASH_DURATION);
        }
    }
}

fn splash_animation(elapsed: Duration) -> (f32, f32) {
    let enter = (elapsed.as_secs_f32() / SPLASH_FADE_IN_SECONDS).clamp(0.0, 1.0);
    let eased_enter = enter * enter * (3.0 - 2.0 * enter);
    let remaining = SPLASH_DURATION.saturating_sub(elapsed).as_secs_f32();
    let exit = (remaining / SPLASH_FADE_OUT_SECONDS).clamp(0.0, 1.0);
    (0.88 + 0.12 * eased_enter, eased_enter * exit)
}

fn invalid_splash(message: &'static str) -> image::ImageError {
    Error::new(ErrorKind::InvalidData, message).into()
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/startup_splash/mod.rs"]
mod tests;
