//! Suite-isolated `Micro3D` objects and rendering state.

use crate::{
    ActionData, ActionTableData, AffineTrans, FigureData, LoaderLimits, TextureData, Vector3D, cos,
    sin,
};
use diagnostics::{Category, EmuError};
use m3g::{
    CullMode, FrameBlend, Image2DState, RenderLimits, RenderStats, SoftwareRenderer,
    Texture2DState, Vec4, Vertex, WrapMode,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

mod checkpoint;
mod figures;
mod lighting;
mod object_state;
mod point_sprites;
mod primitives;
mod projection;
mod rendering;

pub use primitives::{
    PrimitiveData, PrimitiveEnvironment, PrimitivePayloadCounts, primitive_payload_counts,
};

use lighting::Lighting;
use point_sprites::{point_sprite_parameters, point_sprite_vertices};
use projection::PreparedProjection;
use rendering::{
    estimated_bytes, figure_geometry, figure_pattern_visible, primitive_color, primitive_normal,
    projected_vertex, renderer_texture, runtime_error, sphere_texture,
};

const DEFAULT_MAX_OBJECTS: usize = 16_384;
const DEFAULT_MAX_BYTES: usize = 8 * 1024 * 1024;
const FIGURE_ATTR_TRANSPARENT: u32 = 0x01;
const FIGURE_ATTR_BLEND_MASK: u32 = 0x06;
const FIGURE_ATTR_BLEND_HALF: u32 = 0x02;
const FIGURE_ATTR_BLEND_ADD: u32 = 0x04;
const FIGURE_ATTR_BLEND_SUBTRACT: u32 = 0x06;
const FIGURE_ATTR_DOUBLE_FACE: u32 = 0x10;

/// Directional and ambient light state in documented `Micro3D` units.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LightState {
    pub direction: Vector3D,
    pub directional_intensity: i32,
    pub ambient_intensity: i32,
}

impl Default for LightState {
    fn default() -> Self {
        Self {
            direction: Vector3D::new(0, 0, 4096),
            directional_intensity: 4096,
            ambient_intensity: 0,
        }
    }
}

/// Rendering effects remain separate from the JSR-184 appearance model.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EffectState {
    pub light: Option<u64>,
    pub shading: i32,
    pub toon_threshold: i32,
    pub toon_high: i32,
    pub toon_low: i32,
    pub transparency: bool,
    pub sphere_texture: Option<u64>,
}

impl Default for EffectState {
    fn default() -> Self {
        Self {
            light: None,
            shading: 0,
            toon_threshold: 0,
            toon_high: 0,
            toon_low: 0,
            transparency: true,
            sphere_texture: None,
        }
    }
}

/// Projection selected by `FigureLayout`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Projection {
    ParallelScale,
    Parallel {
        width: i32,
        height: i32,
    },
    PerspectiveFov {
        near: i32,
        far: i32,
        angle: i32,
    },
    PerspectiveSize {
        near: i32,
        far: i32,
        width: i32,
        height: i32,
    },
}

/// Figure placement and camera projection state.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FigureLayoutState {
    pub affines: Vec<u64>,
    pub selected_affine: usize,
    pub scale: [i32; 2],
    pub center: [i32; 2],
    pub projection: Projection,
}

impl Default for FigureLayoutState {
    fn default() -> Self {
        Self {
            affines: Vec::new(),
            selected_affine: 0,
            scale: [512, 512],
            center: [0, 0],
            projection: Projection::ParallelScale,
        }
    }
}

/// Mutable associations and animation selection for one Figure.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FigureState {
    pub data: FigureData,
    pub textures: Vec<u64>,
    pub selected_texture: usize,
    pub pattern: i32,
    pub posture: Option<(u64, usize, i32)>,
}

/// Family-specific object variants owned by one suite runtime.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ObjectKind {
    Action(ActionTableData),
    Effect(EffectState),
    Figure(FigureState),
    Layout(FigureLayoutState),
    Light(LightState),
    Texture(TextureData),
    Graphics,
}

impl ObjectKind {
    fn guest_references(&self) -> impl Iterator<Item = u64> + '_ {
        let (list, first, second): (&[u64], Option<u64>, Option<u64>) = match self {
            Self::Figure(state) => (
                &state.textures,
                state.posture.map(|(action, _, _)| action),
                None,
            ),
            Self::Effect(state) => (&[], state.light, state.sphere_texture),
            Self::Layout(state) => (&state.affines, None, None),
            _ => (&[], None, None),
        };
        list.iter().copied().chain(first).chain(second)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct Object {
    kind: Option<ObjectKind>,
    bytes: usize,
}

/// Observable family-specific telemetry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Micro3dMetrics {
    pub live_bytes: usize,
    pub peak_bytes: usize,
    pub live_objects: usize,
    pub peak_objects: usize,
    pub loaded_figures: u64,
    pub loaded_actions: u64,
    pub loaded_textures: u64,
    pub render_calls: u64,
    pub rasterized_triangles: u64,
    pub shaded_pixels: u64,
}

/// Suite-owned object arena and bounded software rendering target.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Runtime {
    objects: BTreeMap<u64, Object>,
    max_objects: usize,
    max_bytes: usize,
    metrics: Micro3dMetrics,
    renderer: SoftwareRenderer,
    render_limits: RenderLimits,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_OBJECTS, DEFAULT_MAX_BYTES, 240, 320)
            .expect("default Micro3D limits and target are valid")
    }
}

impl Runtime {
    pub fn new(
        max_objects: usize,
        max_bytes: usize,
        width: u32,
        height: u32,
    ) -> Result<Self, EmuError> {
        Self::new_with_render_limits(
            max_objects,
            max_bytes,
            width,
            height,
            RenderLimits::default(),
        )
    }

    pub fn new_with_render_limits(
        max_objects: usize,
        max_bytes: usize,
        width: u32,
        height: u32,
        render_limits: RenderLimits,
    ) -> Result<Self, EmuError> {
        if max_objects == 0 || max_bytes == 0 {
            return Err(runtime_error(
                "resource-limit",
                "Micro3D limits must be non-zero",
            ));
        }
        Ok(Self {
            objects: BTreeMap::new(),
            max_objects,
            max_bytes,
            metrics: Micro3dMetrics::default(),
            renderer: SoftwareRenderer::new(width, height, render_limits)?,
            render_limits,
        })
    }

    pub fn create(&mut self, guest: u64, kind: ObjectKind) -> Result<(), EmuError> {
        if self.objects.contains_key(&guest) {
            return Err(runtime_error(
                "duplicate-object",
                "guest already owns Micro3D state",
            ));
        }
        let bytes = estimated_bytes(&kind);
        // Disposed wrappers retain their records until guest GC. Bound that
        // history separately from live objects using the native byte budget.
        if self.metrics.live_objects >= self.max_objects
            || self.objects.len() >= self.max_bytes / size_of::<ObjectKind>()
            || self
                .metrics
                .live_bytes
                .checked_add(bytes)
                .is_none_or(|total| total > self.max_bytes)
        {
            return Err(runtime_error(
                "resource-limit",
                "Micro3D native object budget exhausted",
            ));
        }
        match &kind {
            ObjectKind::Figure(_) => {
                self.metrics.loaded_figures = self.metrics.loaded_figures.saturating_add(1);
            }
            ObjectKind::Action(_) => {
                self.metrics.loaded_actions = self.metrics.loaded_actions.saturating_add(1);
            }
            ObjectKind::Texture(_) => {
                self.metrics.loaded_textures = self.metrics.loaded_textures.saturating_add(1);
            }
            _ => {}
        }
        self.objects.insert(
            guest,
            Object {
                kind: Some(kind),
                bytes,
            },
        );
        self.metrics.live_bytes += bytes;
        self.metrics.live_objects += 1;
        self.metrics.peak_bytes = self.metrics.peak_bytes.max(self.metrics.live_bytes);
        self.metrics.peak_objects = self.metrics.peak_objects.max(self.metrics.live_objects);
        Ok(())
    }

    pub fn create_figure(
        &mut self,
        guest: u64,
        bytes: &[u8],
        limits: LoaderLimits,
    ) -> Result<(), EmuError> {
        let data = FigureData::parse(bytes, limits)?;
        self.create(
            guest,
            ObjectKind::Figure(FigureState {
                data,
                textures: Vec::new(),
                selected_texture: 0,
                pattern: 0,
                posture: None,
            }),
        )
    }

    pub fn create_action(
        &mut self,
        guest: u64,
        bytes: &[u8],
        limits: LoaderLimits,
    ) -> Result<(), EmuError> {
        self.create(
            guest,
            ObjectKind::Action(ActionTableData::parse(bytes, limits)?),
        )
    }

    pub fn create_texture(
        &mut self,
        guest: u64,
        bytes: &[u8],
        for_model: bool,
        limits: LoaderLimits,
    ) -> Result<(), EmuError> {
        let data = TextureData::parse(bytes, for_model, limits)?;
        self.create(guest, ObjectKind::Texture(data))
    }

    pub fn kind(&self, guest: u64) -> Result<&ObjectKind, EmuError> {
        object_kind(&self.objects, guest)
    }

    // Dynamic buffers may change only through setters that reaccount their size.
    fn kind_mut(&mut self, guest: u64) -> Result<&mut ObjectKind, EmuError> {
        self.objects
            .get_mut(&guest)
            .and_then(|object| object.kind.as_mut())
            .ok_or_else(|| runtime_error("disposed-object", "Micro3D object is absent or disposed"))
    }

    pub fn dispose(&mut self, guest: u64) -> Result<(), EmuError> {
        let object = self
            .objects
            .get_mut(&guest)
            .ok_or_else(|| runtime_error("unknown-object", "Micro3D object is not registered"))?;
        if object.kind.take().is_some() {
            self.metrics.live_bytes = self.metrics.live_bytes.saturating_sub(object.bytes);
            self.metrics.live_objects = self.metrics.live_objects.saturating_sub(1);
            object.bytes = 0;
        }
        Ok(())
    }

    #[must_use]
    pub const fn metrics(&self) -> Micro3dMetrics {
        self.metrics
    }

    /// Work counters for the most recent render call, used to bound command lists.
    #[must_use]
    pub const fn render_stats(&self) -> RenderStats {
        self.renderer.stats()
    }

    /// Expands registered guest roots through native ownership. Ordinary Java
    /// roots are already traced by the VM; ordinary endpoints of native edges
    /// are returned even when they have no entry in this arena.
    #[must_use]
    pub fn guest_closure(&self, roots: impl IntoIterator<Item = u64>) -> Vec<u64> {
        // Ordinary Java roots have no outgoing edges in this arena. Avoid
        // inserting the whole Java heap into a second ordered reachability
        // set, especially when the suite does not use Micro3D at all.
        let mut pending = roots
            .into_iter()
            .filter(|guest| self.objects.contains_key(guest))
            .collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        while let Some(guest) = pending.pop() {
            if !visited.insert(guest) {
                continue;
            }
            let Some(kind) = self
                .objects
                .get(&guest)
                .and_then(|object| object.kind.as_ref())
            else {
                continue;
            };
            pending.extend(kind.guest_references());
        }
        // Native ownership can end at an ordinary Java object with no entry in
        // this arena. FigureLayout's AffineTrans is the canonical case: it is
        // heap-backed, but the layout is its only owner. Returning only arena
        // keys would silently drop that final edge during a guest collection.
        visited.into_iter().collect()
    }

    pub fn sweep_guest_objects(&mut self, mut is_live: impl FnMut(u64) -> bool) {
        self.objects.retain(|guest, object| {
            if is_live(*guest) {
                return true;
            }
            if object.kind.is_some() {
                self.metrics.live_bytes = self.metrics.live_bytes.saturating_sub(object.bytes);
                self.metrics.live_objects = self.metrics.live_objects.saturating_sub(1);
            }
            false
        });
    }

    pub fn load_target(&mut self, width: u32, height: u32, pixels: &[u32]) -> Result<(), EmuError> {
        self.load_target_clipped(width, height, pixels, [0, 0, width, height])
    }

    pub fn load_target_clipped(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u32],
        scissor: [u32; 4],
    ) -> Result<(), EmuError> {
        if self.renderer.width() != width || self.renderer.height() != height {
            self.renderer = SoftwareRenderer::new(width, height, self.render_limits)?;
        }
        self.renderer.load_pixels(pixels)?;
        self.prepare_target(width, height, scissor)
    }

    /// Starts a batch with an owned color buffer, avoiding a second pixel copy.
    pub fn replace_target_clipped(
        &mut self,
        width: u32,
        height: u32,
        pixels: Vec<u32>,
        scissor: [u32; 4],
    ) -> Result<(), EmuError> {
        if self.renderer.width() == width && self.renderer.height() == height {
            self.renderer.replace_pixels(pixels)?;
        } else {
            self.renderer =
                SoftwareRenderer::from_pixels(width, height, pixels, self.render_limits)?;
        }
        self.prepare_target(width, height, scissor)
    }

    fn prepare_target(
        &mut self,
        width: u32,
        height: u32,
        scissor: [u32; 4],
    ) -> Result<(), EmuError> {
        self.renderer.set_viewport(0, 0, width, height)?;
        self.renderer
            .set_scissor(scissor[0], scissor[1], scissor[2], scissor[3])?;
        self.renderer.clear(None, true);
        Ok(())
    }

    pub fn set_target_scissor(&mut self, scissor: [u32; 4]) -> Result<(), EmuError> {
        self.renderer
            .set_scissor(scissor[0], scissor[1], scissor[2], scissor[3])
    }

    #[must_use]
    pub fn target_pixels(&self) -> &[u32] {
        self.renderer.pixels()
    }

    fn configure_renderer(
        renderer: &mut SoftwareRenderer,
        texture: Option<&TextureData>,
        sphere_texture: Option<&TextureData>,
        effect: EffectState,
        primitive_command: Option<i32>,
        figure_color_key: bool,
    ) -> Result<(), EmuError> {
        let color_key =
            figure_color_key || primitive_command.is_some_and(|command| command & 0x10 != 0);
        let texture = renderer_texture(texture, color_key)?;
        let sphere_texture = renderer_texture(sphere_texture, false)?;
        renderer.set_texture_units([texture, sphere_texture])?;
        renderer.set_fragment_alpha_override(None);
        renderer.set_alpha_threshold(if color_key { 128 } else { 0 });
        let default_blending = if effect.transparency {
            FrameBlend::SourceOver
        } else {
            FrameBlend::Replace
        };
        let blending = primitive_command.map_or_else(
            || default_blending,
            |command| match command & 0x60 {
                0x20 => FrameBlend::Half,
                0x40 => FrameBlend::Add,
                0x60 => FrameBlend::Subtract,
                _ => default_blending,
            },
        );
        renderer.set_compositing_mode(true, true, true, true, blending, 0.0, 0.0)?;
        renderer.set_depth_test_allows_equal(true);
        Ok(())
    }

    fn record_render_stats(&mut self) {
        let RenderStats {
            rasterized_triangles,
            shaded_fragments,
            ..
        } = self.renderer.stats();
        self.metrics.render_calls = self.metrics.render_calls.saturating_add(1);
        self.metrics.rasterized_triangles = self
            .metrics
            .rasterized_triangles
            .saturating_add(rasterized_triangles);
        self.metrics.shaded_pixels = self.metrics.shaded_pixels.saturating_add(shaded_fragments);
    }
}

fn object_kind(objects: &BTreeMap<u64, Object>, guest: u64) -> Result<&ObjectKind, EmuError> {
    objects
        .get(&guest)
        .and_then(|object| object.kind.as_ref())
        .ok_or_else(|| runtime_error("disposed-object", "Micro3D object is absent or disposed"))
}

#[cfg(test)]
#[path = "../../../tests/unit/micro3d/runtime/mod.rs"]
mod tests;
