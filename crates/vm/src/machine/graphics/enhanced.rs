//! Host-only sample planes for mixed LCDUI/3D images.
//!
//! Four logical-sized planes represent the four samples of a 2x raster. LCDUI
//! Common LCDUI draws update samples while decoding/rasterizing their source once.
//! Other operations use the checked native raster routines on each plane. Guest
//! pixel geometry is retained and no guest bytecode is replayed.

use super::{
    Allocation, ArrayKind, EmuError, Handle, HashMap, HeapValue, Machine, heap_error, type_error,
};

mod samples;

use samples::SamplePlanes;

const MAX_SAMPLE_PIXELS: usize = 1_048_576;
const MAX_SAMPLE_TARGETS: usize = 16;

#[derive(Debug, Default)]
pub(in crate::machine) struct EnhancedGraphics {
    surfaces: HashMap<Handle, SampleSurface>,
    sample_pass: bool,
    presented: Option<(Handle, Vec<u32>)>,
    m3g_synced: Option<Handle>,
}

#[derive(Debug)]
pub(super) struct SampleSurface {
    width: u32,
    height: u32,
    planes: SamplePlanes,
    // Reused by checked image chunks, bounded to one cancellation interval.
    pub(super) row_scratch: Vec<i32>,
}

impl SampleSurface {
    pub(super) fn composite_pixel(&mut self, index: usize, color: i32) -> Result<(), EmuError> {
        self.planes.composite_pixel(index, color)
    }

    pub(super) fn fill(&mut self, start: usize, end: usize, color: i32) -> Result<(), EmuError> {
        self.planes.fill(start, end, color)
    }

    pub(super) fn composite_row(&mut self, start: usize, colors: &[i32]) -> Result<(), EmuError> {
        self.planes.composite_row(start, colors)
    }

    pub(super) fn copy_row(&mut self, start: usize, colors: &[i32]) -> Result<(), EmuError> {
        self.planes.copy_row(start, colors)
    }

    fn interleave(&self) -> Result<Vec<u32>, EmuError> {
        self.planes.interleave(self.width, self.height)
    }

    fn interleave_into(&self, pixels: &mut Vec<u32>) -> Result<(), EmuError> {
        self.planes.interleave_into(self.width, self.height, pixels)
    }
}

impl crate::machine::pixel_ops::ShapeSamples for &mut SampleSurface {
    #[inline]
    fn point(&mut self, index: usize, color: i32) -> Result<(), EmuError> {
        SampleSurface::composite_pixel(self, index, color)
    }
    #[inline]
    fn fill(&mut self, start: usize, end: usize, color: i32) -> Result<(), EmuError> {
        SampleSurface::fill(self, start, end, color)
    }
}

impl EnhancedGraphics {
    pub(super) fn target_mut(&mut self, target: Handle) -> Option<&mut SampleSurface> {
        if self.sample_pass {
            return None;
        }
        self.invalidate(target);
        self.surfaces.get_mut(&target)
    }

    pub(in crate::machine) fn m3g_is_synced(&self, target: Handle) -> bool {
        self.m3g_synced == Some(target) && self.surfaces.contains_key(&target)
    }

    pub(in crate::machine) fn sync_m3g(&mut self, target: Option<Handle>) {
        self.m3g_synced = target;
    }

    fn invalidate(&mut self, target: Handle) {
        if self.m3g_synced == Some(target) {
            self.m3g_synced = None;
        }
    }

    pub(in crate::machine) fn matches_presented(&self, target: Handle) -> bool {
        let Some(surface) = self.surfaces.get(&target) else {
            return true;
        };
        let Some((previous, pixels)) = &self.presented else {
            return false;
        };
        if *previous != target
            || pixels.len() != surface.width as usize * surface.height as usize * 4
        {
            return false;
        }
        surface.planes.matches(surface.width, pixels)
    }

    pub(in crate::machine) fn record_presented(&mut self, target: Handle, pixels: Vec<u32>) {
        self.presented = Some((target, pixels));
    }

    /// Reuses the previous successful frame's storage after the pacing
    /// decision. Only a successful sink call may
    /// restore the comparison baseline with `record_presented`.
    pub(in crate::machine) fn take_presentation_pixels(
        &mut self,
        target: Handle,
    ) -> Result<Option<Vec<u32>>, EmuError> {
        let Some(surface) = self.surfaces.get(&target) else {
            return Ok(None);
        };
        let mut pixels = self
            .presented
            .take()
            .map_or_else(Vec::new, |(_, pixels)| pixels);
        surface.interleave_into(&mut pixels)?;
        Ok(Some(pixels))
    }

    pub(in crate::machine) fn has_target(&self, target: Handle) -> bool {
        self.surfaces.contains_key(&target)
    }

    pub(in crate::machine) fn pixels(&self, target: Handle) -> Result<Option<Vec<u32>>, EmuError> {
        self.surfaces
            .get(&target)
            .map(SampleSurface::interleave)
            .transpose()
    }

    #[cfg(test)]
    pub(in crate::machine) fn store(
        &mut self,
        target: Handle,
        pixels: &[u32],
        clip: [u32; 4],
        opaque: bool,
    ) -> Result<(), EmuError> {
        self.store_resolved(target, pixels, clip, opaque, None)
    }

    pub(in crate::machine) fn store_resolved(
        &mut self,
        target: Handle,
        pixels: &[u32],
        clip: [u32; 4],
        opaque: bool,
        destination: Option<&mut [HeapValue]>,
    ) -> Result<(), EmuError> {
        self.invalidate(target);
        let Some(surface) = self.surfaces.get_mut(&target) else {
            return Ok(());
        };
        let [x, y, width, height] = clip;
        if pixels.len() != surface.width as usize * surface.height as usize * 4
            || x.checked_add(width).is_none_or(|end| end > surface.width)
            || y.checked_add(height).is_none_or(|end| end > surface.height)
            || destination
                .as_ref()
                .is_some_and(|v| v.len() != surface.planes.len())
        {
            return Err(type_error());
        }
        surface
            .planes
            .store(surface.width as usize, pixels, clip, opaque, destination);
        Ok(())
    }
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn enhanced_image_source_pending(
        &self,
        image: Handle,
    ) -> Result<bool, EmuError> {
        if !self.enhanced_2d_pending() {
            return Ok(false);
        }
        let source =
            self.graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")?;
        Ok(self.graphics.enhanced.has_target(source))
    }

    pub(in crate::machine) fn enhanced_2d_pending(&self) -> bool {
        !self.graphics.enhanced.sample_pass && !self.graphics.enhanced.surfaces.is_empty()
    }

    pub(in crate::machine) fn ensure_enhanced_target(
        &mut self,
        target: Handle,
        width: u32,
        height: u32,
    ) -> Result<bool, EmuError> {
        if self.native_context.render_3d_scale() != 2 {
            return Ok(false);
        }
        let (display_width, display_height) = self.native_context.lcd_dimensions();
        if u64::from(display_width) * u64::from(display_height) > MAX_SAMPLE_PIXELS as u64 {
            return Ok(false);
        }
        let count = (width as usize).checked_mul(height as usize);
        let Some(count) = count.filter(|count| *count != 0 && *count <= MAX_SAMPLE_PIXELS) else {
            return Ok(false);
        };
        self.graphics
            .enhanced
            .surfaces
            .retain(|handle, _| self.heap.managed.get(*handle).is_ok());
        if let Some(surface) = self.graphics.enhanced.surfaces.get(&target) {
            return Ok(surface.width == width && surface.height == height);
        }
        let retained: usize = self
            .graphics
            .enhanced
            .surfaces
            .values()
            .map(|surface| surface.planes.len())
            .sum();
        if retained + count > MAX_SAMPLE_PIXELS
            || self.graphics.enhanced.surfaces.len() >= MAX_SAMPLE_TARGETS
        {
            return Ok(false);
        }
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get(target).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if elements.len() != count || elements.iter().any(|v| !matches!(v, HeapValue::Int(_))) {
            return Err(type_error());
        }
        let Some(planes) = SamplePlanes::new(elements)? else {
            return Ok(false);
        };
        self.graphics.enhanced.surfaces.insert(
            target,
            SampleSurface {
                width,
                height,
                planes,
                row_scratch: Vec::new(),
            },
        );
        Ok(true)
    }

    /// Composites a native LCDUI operation into the original image and its sample planes.
    pub(in crate::machine) fn enhanced_2d(
        &mut self,
        graphics: Handle,
        source_image: Option<Handle>,
        transform: i32,
        mut draw: impl FnMut(&mut Self) -> Result<(), EmuError>,
    ) -> Result<(), EmuError> {
        let (target, width, height) = self.graphics_target(graphics)?;
        self.graphics.enhanced.invalidate(target);
        let source = source_image
            .map(|image| {
                self.graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
            })
            .transpose()?;
        // MIDP does not define drawing an image onto itself. Keep defensive
        // internal self-blits on the canonical backing, without stale samples.
        if source == Some(target) {
            self.graphics.enhanced.surfaces.remove(&target);
        }
        if source.is_some_and(|source| self.graphics.enhanced.has_target(source)) {
            self.ensure_enhanced_target(target, width.cast_unsigned(), height.cast_unsigned())?;
        }
        let host = &*self.native_context;
        if let Some(surface) = self.graphics.enhanced.surfaces.get_mut(&target) {
            surface
                .planes
                .prepare_guest_planes(|_| super::check_raster_cancellation(host, 0))?;
        }
        if let Some(surface) =
            source.and_then(|source| self.graphics.enhanced.surfaces.get_mut(&source))
        {
            surface
                .planes
                .prepare_guest_planes(|_| super::check_raster_cancellation(host, 0))?;
        }
        self.graphics.enhanced.sample_pass = true;
        let result = draw(self);
        self.graphics.enhanced.sample_pass = false;
        if let Err(error) = result {
            self.graphics.enhanced.surfaces.remove(&target);
            return Err(error);
        }
        let Some(mut surface) = self.graphics.enhanced.surfaces.remove(&target) else {
            return Ok(());
        };
        // Separate source planes preserve subpixel orientation for drawRegion.
        let mut source_surface = source
            .filter(|source| *source != target)
            .and_then(|source| self.graphics.enhanced.surfaces.remove(&source));
        let counters = self.execution.counters.clone();
        self.graphics.enhanced.sample_pass = true;
        let result = (|| {
            for phase in 0..4 {
                super::check_raster_cancellation(self.native_context, 0)?;
                let source_phase = transformed_phase(phase, transform);
                let mut original_source = None;
                if let (Some(source), Some(source_surface)) = (source, source_surface.as_mut()) {
                    original_source = Some(self.swap_sample_plane(
                        source,
                        source_surface.planes.guest_plane_mut(source_phase)?,
                    )?);
                }
                let swap = self.swap_sample_plane(target, surface.planes.guest_plane_mut(phase)?);
                let result = match swap {
                    Ok(mut original) => {
                        let result = draw(self);
                        let restored = self.swap_sample_plane(target, &mut original);
                        *surface.planes.guest_plane_mut(phase)? = restored?;
                        result
                    }
                    Err(error) => Err(error),
                };
                if let (Some(source), Some(source_surface), Some(mut original)) =
                    (source, source_surface.as_mut(), original_source)
                {
                    *source_surface.planes.guest_plane_mut(source_phase)? =
                        self.swap_sample_plane(source, &mut original)?;
                }
                result?;
            }
            Ok(())
        })();
        self.graphics.enhanced.sample_pass = false;
        self.execution.counters = counters;
        if let (Some(source), Some(source_surface)) = (source, source_surface) {
            self.graphics
                .enhanced
                .surfaces
                .insert(source, source_surface);
        }
        if result.is_ok() {
            self.graphics.enhanced.surfaces.insert(target, surface);
        }
        result
    }

    fn swap_sample_plane(
        &mut self,
        target: Handle,
        plane: &mut Vec<HeapValue>,
    ) -> Result<Vec<HeapValue>, EmuError> {
        let Allocation::Array {
            kind: ArrayKind::Int,
            elements,
        } = self.heap.managed.get_mut(target).map_err(heap_error)?
        else {
            return Err(type_error());
        };
        if elements.len() != plane.len() {
            return Err(type_error());
        }
        Ok(std::mem::replace(elements, std::mem::take(plane)))
    }
}

fn transformed_phase(phase: usize, transform: i32) -> usize {
    let (x, y) = (phase % 2, phase / 2);
    let (x, y) = match transform {
        0 => (x, y),
        1 => (x, 1 - y),
        2 => (1 - x, y),
        3 => (1 - x, 1 - y),
        4 => (y, x),
        5 => (y, 1 - x),
        6 => (1 - y, x),
        _ => (1 - y, 1 - x),
    };
    y * 2 + x
}
