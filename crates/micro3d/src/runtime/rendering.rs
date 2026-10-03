use super::{
    ActionData, AffineTrans, Arc, BTreeMap, Category, EmuError, FigureData, Image2DState, Object,
    ObjectKind, PreparedProjection, Texture2DState, TextureData, Vector3D, Vertex, WrapMode,
    object_kind,
};

pub(super) fn sphere_texture(
    objects: &BTreeMap<u64, Object>,
    guest: Option<u64>,
) -> Result<Option<&TextureData>, EmuError> {
    guest
        .map(|guest| match object_kind(objects, guest)? {
            ObjectKind::Texture(texture) if !texture.for_model => Ok(texture),
            ObjectKind::Texture(_) => Err(runtime_error(
                "texture-kind",
                "Effect3D sphere texture must be an environment texture",
            )),
            _ => Err(runtime_error(
                "type",
                "Effect3D sphere texture requires a Texture",
            )),
        })
        .transpose()
}

pub(super) fn renderer_texture(
    texture: Option<&TextureData>,
    color_key: bool,
) -> Result<Option<Texture2DState>, EmuError> {
    texture
        .map(|texture| {
            let image = Image2DState::from_shared_argb(
                texture.width,
                texture.height,
                Arc::clone(&texture.pixels),
            )?;
            let mut state = Texture2DState::new(image);
            state.set_wrapping(WrapMode::Clamp, WrapMode::Clamp);
            state.set_color_key(color_key.then_some(texture.color_key));
            Ok(state)
        })
        .transpose()
}

pub(super) fn figure_pattern_visible(pattern: i32, group: u16) -> bool {
    if group == 0 {
        return true;
    }
    let Some(bit) = 1_u32.checked_shl(u32::from(group - 1)) else {
        return false;
    };
    pattern.cast_unsigned() & bit != 0
}

pub(super) fn figure_geometry(
    figure: &FigureData,
    posture: Option<(&ActionData, i32)>,
    transform_normals: bool,
) -> Result<(Vec<Vector3D>, Vec<Vector3D>), EmuError> {
    if let Some((action, _)) = posture
        && action.segments.len() < figure.bones.len()
    {
        return Err(runtime_error(
            "action-bones",
            format!(
                "ActionTable bone count {} is shorter than Figure bone count {}",
                action.segments.len(),
                figure.bones.len()
            ),
        ));
    }
    let mut world: Vec<AffineTrans> = Vec::with_capacity(figure.bones.len());
    let mut vertices = Vec::with_capacity(figure.vertices.len());
    let mut normals = if transform_normals {
        Vec::with_capacity(figure.normals.len())
    } else {
        Vec::new()
    };
    if transform_normals
        && !figure.normals.is_empty()
        && figure.normals.len() != figure.vertices.len()
    {
        return Err(runtime_error(
            "figure-normal-count",
            "Figure normal count does not match its vertices",
        ));
    }
    // The loader also accepts static meshes without a skeleton. Their vertices
    // use model coordinates; normals still need the usual unit-length basis.
    if figure.bones.is_empty() {
        vertices.extend_from_slice(&figure.vertices);
        if transform_normals {
            normals.extend(
                figure
                    .normals
                    .iter()
                    .copied()
                    .map(AffineTrans::IDENTITY.normal_transform()),
            );
        }
        return Ok((vertices, normals));
    }
    let mut source_offset = 0_usize;
    for (index, bone) in figure.bones.iter().enumerate() {
        let local = posture.map_or(bone.transform, |(action, frame)| {
            bone.transform
                .multiplied(action.segments[index].sample_transform(frame))
        });
        let transform = if bone.parent < 0 {
            local
        } else {
            world
                .get(usize::try_from(bone.parent).unwrap_or(usize::MAX))
                .copied()
                .ok_or_else(|| {
                    runtime_error("figure-bone-parent", "Figure bone parent is invalid")
                })?
                .multiplied(local)
        };
        world.push(transform);
        let end = source_offset
            .checked_add(usize::from(bone.vertex_count))
            .ok_or_else(|| runtime_error("figure-bone-overflow", "Figure bone range overflow"))?;
        let source = figure.vertices.get(source_offset..end).ok_or_else(|| {
            runtime_error(
                "figure-bone-coverage",
                "Figure bone range exceeds its vertices",
            )
        })?;
        vertices.extend(
            source
                .iter()
                .copied()
                .map(|vertex| transform.transform(vertex)),
        );
        if transform_normals && !figure.normals.is_empty() {
            let source = figure.normals.get(source_offset..end).ok_or_else(|| {
                runtime_error(
                    "figure-normal-coverage",
                    "Figure bone range exceeds its normals",
                )
            })?;
            normals.extend(source.iter().copied().map(transform.normal_transform()));
        }
        source_offset = end;
    }
    if source_offset != figure.vertices.len() {
        return Err(runtime_error(
            "figure-bone-coverage",
            "Figure bone ranges do not cover its vertices",
        ));
    }
    Ok((vertices, normals))
}

pub(super) fn projected_vertex(
    source: Vector3D,
    uv: [u32; 2],
    projection: &PreparedProjection,
    texture: Option<&TextureData>,
    sphere_coordinates: Option<(f32, f32)>,
    color: u32,
) -> Vertex {
    let position = projection.project(source);
    let (s, t) = normalized_uv(uv, texture);
    let (sphere_s, sphere_t) = sphere_coordinates.unwrap_or((0.5, 0.5));
    Vertex::with_textures(position, color, [[s, t, 0.0], [sphere_s, sphere_t, 0.0]])
}

pub(super) fn normalized_uv(uv: [u32; 2], texture: Option<&TextureData>) -> (f32, f32) {
    let Some(texture) = texture else {
        return (0.0, 0.0);
    };
    (
        uv[0] as f32 / texture.width.max(1) as f32,
        uv[1] as f32 / texture.height.max(1) as f32,
    )
}

pub(super) fn sphere_uv(mut normal: Vector3D) -> (f32, f32) {
    normal.unit();
    (
        (normal.x as f32 / 4096.0 + 1.0) * 0.5,
        (1.0 - normal.y as f32 / 4096.0) * 0.5,
    )
}

pub(super) fn primitive_color(command: i32, primitive: usize, colors: &[i32]) -> u32 {
    let selector = command & 0x0c00;
    let rgb = match selector {
        0x0400 => colors.first().copied().unwrap_or(0x00ff_ffff),
        0x0800 => colors.get(primitive).copied().unwrap_or(0x00ff_ffff),
        _ => 0x00ff_ffff,
    };
    0xff00_0000 | rgb.cast_unsigned() & 0x00ff_ffff
}

pub(super) fn primitive_normal(
    command: i32,
    primitive: usize,
    vertex: usize,
    normals: &[i32],
) -> Option<Vector3D> {
    let offset = match command & 0x0300 {
        0x0200 => primitive.saturating_mul(3),
        0x0300 => vertex.saturating_mul(3),
        _ => return None,
    };
    let normal: [i32; 3] = normals
        .get(offset..offset.saturating_add(3))?
        .try_into()
        .ok()?;
    Some(Vector3D::new(normal[0], normal[1], normal[2]))
}

pub(super) fn estimated_bytes(kind: &ObjectKind) -> usize {
    size_of::<ObjectKind>().saturating_add(match kind {
        ObjectKind::Figure(state) => state
            .data
            .allocated_bytes()
            .saturating_add(state.textures.capacity().saturating_mul(size_of::<u64>())),
        ObjectKind::Action(state) => state.allocated_bytes(),
        ObjectKind::Texture(state) => state.allocated_bytes(),
        ObjectKind::Layout(state) => state.affines.capacity().saturating_mul(size_of::<u64>()),
        _ => 0,
    })
}

pub(super) fn runtime_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Micro3d, code, message)
}
