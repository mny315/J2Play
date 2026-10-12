//! Decodes serialized object fields into native M3G state.

use super::{Cursor, MeshData, error, resolve, resolve_nullable};
use crate::{
    AnimationControllerState, AppearanceState, BackgroundState, CameraProjection,
    CompositingModeState, FogMode, FogState, Handle, Image2DState, ImageFormat, Interpolation,
    KeyframeSequenceState, LightState, MaterialObjectState, MeshState, NodeState, ObjectKind,
    ObjectType, PolygonModeState, RepeatMode, Runtime, SkinInfluence, SpriteState,
    TextureObjectState, TransformableState, TriangleStripArrayState, VertexArrayState,
    VertexBufferState, VertexComponent,
};
use diagnostics::EmuError;
use std::collections::BTreeMap;

pub(super) fn decode_kind(
    kind: ObjectType,
    cursor: &mut Cursor<'_>,
    mesh: Option<&MeshData>,
    handles: &[Option<Handle>],
    runtime: &Runtime,
) -> Result<ObjectKind, EmuError> {
    Ok(match kind {
        ObjectType::Header | ObjectType::ExternalReference => {
            return Err(error(
                "invalid-object-order",
                "header/external object reached decoder",
            ));
        }
        ObjectType::AnimationController => {
            let mut state = AnimationControllerState::default();
            let speed = cursor.f32()?;
            let weight = cursor.f32()?;
            let start = cursor.i32()?;
            let end = cursor.i32()?;
            let position = cursor.f32()?;
            let world_time = cursor.i32()?;
            state.set_active_interval(start, end)?;
            state.set_position(position, world_time)?;
            state.set_speed(speed, world_time)?;
            state.set_weight(weight)?;
            ObjectKind::AnimationController(state)
        }
        ObjectType::AnimationTrack => ObjectKind::AnimationTrack {
            sequence: resolve(handles, cursor.u32()?)?,
            controller: resolve_nullable(handles, cursor.u32()?)?,
            property: cursor.u32()? as i32,
        },
        ObjectType::Appearance => {
            let mut state = AppearanceState {
                layer: i32::from(cursor.byte()? as i8),
                compositing_mode: resolve_nullable(handles, cursor.u32()?)?,
                fog: resolve_nullable(handles, cursor.u32()?)?,
                polygon_mode: resolve_nullable(handles, cursor.u32()?)?,
                material: resolve_nullable(handles, cursor.u32()?)?,
                ..AppearanceState::default()
            };
            let count = cursor.u32()? as usize;
            for unit in 0..count {
                let texture = resolve(handles, cursor.u32()?)?;
                if let Some(slot) = state.textures.get_mut(unit) {
                    *slot = Some(texture);
                }
            }
            ObjectKind::Appearance(state)
        }
        ObjectType::Background => {
            let state = BackgroundState {
                color: cursor.argb()?,
                image: resolve_nullable(handles, cursor.u32()?)?,
                mode_x: i32::from(cursor.byte()?),
                mode_y: i32::from(cursor.byte()?),
                crop: [cursor.i32()?, cursor.i32()?, cursor.i32()?, cursor.i32()?],
                depth_clear: cursor.boolean()?,
                color_clear: cursor.boolean()?,
            };
            ObjectKind::Background(state)
        }
        ObjectType::Camera => {
            let projection = match cursor.byte()? {
                48 => CameraProjection::Generic(cursor.matrix()?),
                49 => CameraProjection::Parallel {
                    height: cursor.f32()?,
                    aspect_ratio: cursor.f32()?,
                    near: cursor.f32()?,
                    far: cursor.f32()?,
                },
                50 => CameraProjection::Perspective {
                    field_of_view: cursor.f32()?,
                    aspect_ratio: cursor.f32()?,
                    near: cursor.f32()?,
                    far: cursor.f32()?,
                },
                _ => return Err(error("invalid-enum", "invalid camera projection")),
            };
            projection.validate()?;
            ObjectKind::Camera {
                node: NodeState::default(),
                projection,
            }
        }
        ObjectType::CompositingMode => ObjectKind::CompositingMode(CompositingModeState {
            depth_test: cursor.boolean()?,
            depth_write: cursor.boolean()?,
            color_write: cursor.boolean()?,
            alpha_write: cursor.boolean()?,
            blending: i32::from(cursor.byte()?),
            alpha_threshold: f32::from(cursor.byte()?) / 255.0,
            depth_offset_factor: cursor.f32()?,
            depth_offset_units: cursor.f32()?,
        }),
        ObjectType::Fog => {
            let color = cursor.rgb()?;
            let mode = cursor.byte()?;
            let mut state = FogState {
                color,
                mode: if mode == 80 {
                    FogMode::Exponential
                } else {
                    FogMode::Linear
                },
                near: 0.0,
                far: 1.0,
                density: 1.0,
            };
            if mode == 80 {
                state.density = cursor.f32()?;
            } else {
                state.near = cursor.f32()?;
                state.far = cursor.f32()?;
            }
            ObjectKind::Fog(state)
        }
        ObjectType::PolygonMode => ObjectKind::PolygonMode(PolygonModeState {
            culling: i32::from(cursor.byte()?),
            shading: i32::from(cursor.byte()?),
            winding: i32::from(cursor.byte()?),
            two_sided_lighting: cursor.boolean()?,
            local_camera_lighting: cursor.boolean()?,
            perspective_correction: cursor.boolean()?,
        }),
        ObjectType::Group => ObjectKind::Group {
            node: NodeState::default(),
            children: Vec::new(),
        },
        ObjectType::Image2D => decode_image(cursor)?,
        ObjectType::TriangleStripArray => ObjectKind::TriangleStripArray(decode_strips(cursor)?),
        ObjectType::Light => ObjectKind::Light {
            node: NodeState::default(),
            light: LightState {
                attenuation: [cursor.f32()?, cursor.f32()?, cursor.f32()?],
                color: cursor.rgb()?,
                mode: i32::from(cursor.byte()?),
                intensity: cursor.f32()?,
                spot_angle: cursor.f32()?,
                spot_exponent: cursor.f32()?,
            },
        },
        ObjectType::Material => {
            let mut state = MaterialObjectState::default();
            state.material.ambient = cursor.rgb()?;
            state.material.diffuse = cursor.argb()?;
            state.material.emissive = cursor.rgb()?;
            state.material.specular = cursor.rgb()?;
            state.material.shininess = cursor.f32()?;
            state.vertex_color_tracking = cursor.boolean()?;
            ObjectKind::Material(state)
        }
        ObjectType::Mesh => ObjectKind::Mesh(resolve_mesh(mesh, handles)?),
        ObjectType::MorphingMesh => {
            let count = cursor.record_count(8)?;
            let mut targets = Vec::with_capacity(count);
            let mut weights = Vec::with_capacity(count);
            for _ in 0..count {
                targets.push(resolve(handles, cursor.u32()?)?);
                weights.push(cursor.f32()?);
            }
            ObjectKind::MorphingMesh {
                mesh: resolve_mesh(mesh, handles)?,
                targets,
                weights,
            }
        }
        ObjectType::SkinnedMesh => {
            let skeleton = resolve(handles, cursor.u32()?)?;
            let count = cursor.record_count(16)?;
            let mut bones = Vec::new();
            let mut bone_indices = BTreeMap::new();
            let mut influences = Vec::with_capacity(count);
            for _ in 0..count {
                let bone = resolve(handles, cursor.u32()?)?;
                let bone_index = *bone_indices.entry(bone).or_insert_with(|| {
                    let index = bones.len();
                    bones.push(bone);
                    index
                });
                influences.push(SkinInfluence {
                    bone: bone_index,
                    first_vertex: cursor.u32()? as usize,
                    vertex_count: cursor.u32()? as usize,
                    weight: cursor
                        .i32()?
                        .try_into()
                        .map_err(|_| error("invalid-skinning", "negative bone weight"))?,
                });
            }
            ObjectKind::SkinnedMesh {
                mesh: resolve_mesh(mesh, handles)?,
                skeleton,
                bones,
                bind_transforms: Vec::new(),
                influences,
            }
        }
        ObjectType::Texture2D => ObjectKind::Texture2D(TextureObjectState {
            transformable: TransformableState::default(),
            image: resolve(handles, cursor.u32()?)?,
            blend_color: cursor.rgb()?,
            blending: i32::from(cursor.byte()?),
            wrap_s: i32::from(cursor.byte()?),
            wrap_t: i32::from(cursor.byte()?),
            level_filter: i32::from(cursor.byte()?),
            image_filter: i32::from(cursor.byte()?),
        }),
        ObjectType::Sprite3D => ObjectKind::Sprite3D(SpriteState {
            node: NodeState::default(),
            image: resolve(handles, cursor.u32()?)?,
            appearance: resolve_nullable(handles, cursor.u32()?)?,
            scaled: cursor.boolean()?,
            crop: [cursor.i32()?, cursor.i32()?, cursor.i32()?, cursor.i32()?],
        }),
        ObjectType::KeyframeSequence => ObjectKind::KeyframeSequence(decode_keyframes(cursor)?),
        ObjectType::VertexArray => ObjectKind::VertexArray(decode_vertex_array(cursor)?),
        ObjectType::VertexBuffer => {
            let (state, arrays) = decode_vertex_buffer(cursor, handles, runtime)?;
            ObjectKind::VertexBuffer { state, arrays }
        }
        ObjectType::World => ObjectKind::World {
            node: NodeState::default(),
            children: Vec::new(),
            camera: resolve_nullable(handles, cursor.u32()?)?,
            background: resolve_nullable(handles, cursor.u32()?)?,
        },
    })
}

fn resolve_mesh(
    mesh: Option<&MeshData>,
    handles: &[Option<Handle>],
) -> Result<MeshState, EmuError> {
    let mesh = mesh.ok_or_else(|| error("missing-mesh-data", "mesh superclass data is absent"))?;
    Ok(MeshState {
        node: NodeState::default(),
        vertices: resolve(handles, mesh.vertices)?,
        submeshes: mesh
            .submeshes
            .iter()
            .map(|reference| resolve(handles, *reference))
            .collect::<Result<_, _>>()?,
        appearances: mesh
            .appearances
            .iter()
            .map(|reference| resolve_nullable(handles, *reference))
            .collect::<Result<_, _>>()?,
    })
}

fn decode_image(cursor: &mut Cursor<'_>) -> Result<ObjectKind, EmuError> {
    let format = match cursor.byte()? {
        96 => ImageFormat::Alpha,
        97 => ImageFormat::Luminance,
        98 => ImageFormat::LuminanceAlpha,
        99 => ImageFormat::Rgb,
        100 => ImageFormat::Rgba,
        _ => return Err(error("invalid-enum", "invalid image format")),
    };
    let mutable = cursor.boolean()?;
    let width = cursor.u32()?;
    let height = cursor.u32()?;
    let image = if mutable {
        Image2DState::mutable(format, width, height)?
    } else {
        let palette = cursor.bytes_with_length()?;
        let pixels = cursor.bytes_with_length()?;
        if palette.is_empty() {
            Image2DState::from_bytes(format, width, height, pixels)?
        } else {
            Image2DState::from_palette(format, width, height, pixels, palette)?
        }
    };
    Ok(ObjectKind::Image2D(image))
}

fn decode_strips(cursor: &mut Cursor<'_>) -> Result<TriangleStripArrayState, EmuError> {
    let encoding = cursor.byte()?;
    let indices = match encoding {
        0..=2 => {
            let first = match encoding {
                0 => cursor.u32()?,
                1 => u32::from(cursor.byte()?),
                _ => u32::from(cursor.u16()?),
            };
            let strips = read_strip_lengths(cursor)?;
            return TriangleStripArrayState::implicit(first, strips);
        }
        128 => (0..cursor.u32()?)
            .map(|_| cursor.u32())
            .collect::<Result<Vec<_>, _>>()?,
        129 => {
            let count = cursor.u32()?;
            (0..count)
                .map(|_| cursor.byte().map(u32::from))
                .collect::<Result<Vec<_>, _>>()?
        }
        130 => {
            let count = cursor.u32()?;
            (0..count)
                .map(|_| cursor.u16().map(u32::from))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => return Err(error("invalid-enum", "invalid strip encoding")),
    };
    let strips = read_strip_lengths(cursor)?;
    TriangleStripArrayState::new(indices, strips)
}

fn read_strip_lengths(cursor: &mut Cursor<'_>) -> Result<Vec<usize>, EmuError> {
    let count = cursor.u32()?;
    (0..count)
        .map(|_| cursor.u32().map(|value| value as usize))
        .collect()
}

fn decode_keyframes(cursor: &mut Cursor<'_>) -> Result<KeyframeSequenceState, EmuError> {
    let interpolation = match cursor.byte()? {
        176 => Interpolation::Linear,
        177 => Interpolation::Slerp,
        178 => Interpolation::Spline,
        179 => Interpolation::Squad,
        _ => Interpolation::Step,
    };
    let repeat = cursor.byte()?;
    let encoding = cursor.byte()?;
    let duration = cursor.u32()? as i32;
    let first = cursor.u32()? as usize;
    let last = cursor.u32()? as usize;
    let components = cursor.u32()? as usize;
    let count = cursor.u32()? as usize;
    let mut state = KeyframeSequenceState::new(count, components, interpolation)?;
    let (bias, scale) = if encoding == 0 {
        (Vec::new(), Vec::new())
    } else {
        (
            (0..components)
                .map(|_| cursor.f32())
                .collect::<Result<_, _>>()?,
            (0..components)
                .map(|_| cursor.f32())
                .collect::<Result<_, _>>()?,
        )
    };
    let mut value = vec![0.0; components];
    for index in 0..count {
        let time = cursor.u32()? as i32;
        for (component, destination) in value.iter_mut().enumerate() {
            let encoded = match encoding {
                0 => cursor.f32()?,
                1 => f32::from(cursor.byte()?),
                _ => f32::from(cursor.u16()?),
            };
            *destination = if encoding == 0 {
                encoded
            } else {
                let quantization_max = if encoding == 1 { 255.0 } else { 65_535.0 };
                (encoded / quantization_max).mul_add(scale[component], bias[component])
            };
        }
        state.set_keyframe(index, time, &value)?;
    }
    state.set_duration(duration)?;
    state.set_valid_range(first, last)?;
    state.set_repeat_mode(if repeat == 193 {
        RepeatMode::Loop
    } else {
        RepeatMode::Constant
    });
    Ok(state)
}

fn decode_vertex_array(cursor: &mut Cursor<'_>) -> Result<VertexArrayState, EmuError> {
    let size = cursor.byte()?;
    let components = cursor.byte()? as usize;
    let encoding = cursor.byte()?;
    let vertices = cursor.u16()? as usize;
    let component_type = if size == 1 {
        VertexComponent::Byte
    } else {
        VertexComponent::Short
    };
    let mut state = VertexArrayState::new(vertices, components, component_type)?;
    let mut previous = [0_i16; 4];
    let mut values = Vec::with_capacity(vertices * components);
    for _ in 0..vertices {
        for previous_component in previous.iter_mut().take(components) {
            let raw = if size == 1 {
                i16::from(cursor.byte()? as i8)
            } else {
                cursor.u16()? as i16
            };
            let value = if encoding == 1 {
                previous_component.wrapping_add(raw)
            } else {
                raw
            };
            *previous_component = value;
            values.push(value);
        }
    }
    if size == 1 {
        state.set_integers(0, vertices, values.iter().copied().map(i32::from))?;
    } else {
        state.set_shorts(0, vertices, &values)?;
    }
    Ok(state)
}

fn decode_vertex_buffer(
    cursor: &mut Cursor<'_>,
    handles: &[Option<Handle>],
    runtime: &Runtime,
) -> Result<(VertexBufferState, [Option<Handle>; 5]), EmuError> {
    let mut state = VertexBufferState::default();
    let mut arrays = [None; 5];
    state.set_default_color(cursor.argb()?);
    let positions = resolve_nullable(handles, cursor.u32()?)?;
    let bias = [cursor.f32()?, cursor.f32()?, cursor.f32()?];
    let scale = cursor.f32()?;
    if let Some(positions) = positions {
        state.set_positions(Some(vertex_array(runtime, positions)?.clone()), scale, bias)?;
        arrays[0] = Some(positions);
    }
    let normals = resolve_nullable(handles, cursor.u32()?)?;
    if let Some(normals) = normals {
        state.set_normals(Some(vertex_array(runtime, normals)?.clone()))?;
        arrays[1] = Some(normals);
    }
    let colors = resolve_nullable(handles, cursor.u32()?)?;
    if let Some(colors) = colors {
        state.set_colors(Some(vertex_array(runtime, colors)?.clone()))?;
        arrays[2] = Some(colors);
    }
    let count = cursor.u32()? as usize;
    for unit in 0..count {
        let reference = resolve(handles, cursor.u32()?)?;
        let bias = [cursor.f32()?, cursor.f32()?, cursor.f32()?];
        let scale = cursor.f32()?;
        let array = vertex_array(runtime, reference)?;
        if array.component_count() == 2 && bias[2] != 0.0 {
            return Err(error(
                "invalid-vertex-buffer",
                "two-component texture coordinates require a zero third bias",
            ));
        }
        state.set_texture_coordinates(unit, Some(array.clone()), scale, bias)?;
        arrays[3 + unit] = Some(reference);
    }
    Ok((state, arrays))
}

fn vertex_array(runtime: &Runtime, handle: Handle) -> Result<&VertexArrayState, EmuError> {
    match runtime.kind(handle)? {
        ObjectKind::VertexArray(array) => Ok(array),
        _ => Err(error(
            "reference-type",
            "vertex buffer references a non-VertexArray",
        )),
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/m3g/instantiate/objects.rs"]
mod tests;
