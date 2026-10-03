use super::{EmuError, vm_error};

pub(super) fn jsr239_texture_retained_bytes(width: u32, height: u32) -> Option<usize> {
    if width == 0 || height == 0 {
        return None;
    }
    let mut width = usize::try_from(width).ok()?;
    let mut height = usize::try_from(height).ok()?;
    let mut pixels = 0_usize;
    loop {
        pixels = pixels.checked_add(width.checked_mul(height)?)?;
        if width == 1 && height == 1 {
            break;
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    pixels.checked_mul(std::mem::size_of::<u32>())
}

pub(super) fn jsr239_argb(red: f32, green: f32, blue: f32, alpha: f32) -> Result<u32, EmuError> {
    if ![red, green, blue, alpha].into_iter().all(f32::is_finite) {
        return Err(vm_error(
            "illegal-argument-exception",
            "OpenGL ES color must be finite",
        ));
    }
    let byte = |component: f32| (component.clamp(0.0, 1.0) * 255.0).round() as u32;
    Ok(byte(alpha) << 24 | byte(red) << 16 | byte(green) << 8 | byte(blue))
}

pub(super) fn jsr239_frustum(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> Result<m3g::Mat4, EmuError> {
    if ![left, right, bottom, top, near, far]
        .into_iter()
        .all(f32::is_finite)
        || left == right
        || bottom == top
        || near <= 0.0
        || far <= 0.0
        || far == near
    {
        return Err(vm_error(
            "illegal-argument-exception",
            "invalid OpenGL ES frustum",
        ));
    }
    let [left, right, bottom, top, near, far] =
        [left, right, bottom, top, near, far].map(f64::from);
    let width = right - left;
    let height = top - bottom;
    let depth = far - near;
    let matrix = [
        2.0 * near / width,
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 * near / height,
        0.0,
        0.0,
        (right + left) / width,
        (top + bottom) / height,
        -(far + near) / depth,
        -1.0,
        0.0,
        0.0,
        -(2.0 * far * near) / depth,
        0.0,
    ];
    m3g::Mat4::from_array(matrix.map(|value| value as f32))
}

pub(super) fn m3g_image_format(value: i32) -> Result<m3g::ImageFormat, EmuError> {
    match value {
        96 => Ok(m3g::ImageFormat::Alpha),
        97 => Ok(m3g::ImageFormat::Luminance),
        98 => Ok(m3g::ImageFormat::LuminanceAlpha),
        99 => Ok(m3g::ImageFormat::Rgb),
        100 => Ok(m3g::ImageFormat::Rgba),
        _ => Err(vm_error(
            "illegal-argument-exception",
            "unknown Image2D format",
        )),
    }
}

pub(super) fn command_value(commands: &[i32], cursor: &mut usize) -> Result<i32, EmuError> {
    let value = commands.get(*cursor).copied().ok_or_else(|| {
        vm_error(
            "illegal-argument-exception",
            "Micro3D command list is truncated",
        )
    })?;
    *cursor += 1;
    Ok(value)
}

pub(super) fn command_slice<'a>(
    commands: &'a [i32],
    cursor: &mut usize,
    count: usize,
) -> Result<&'a [i32], EmuError> {
    let end = cursor
        .checked_add(count)
        .ok_or_else(|| vm_error("resource-overflow", "Micro3D command length overflow"))?;
    let values = commands.get(*cursor..end).ok_or_else(|| {
        vm_error(
            "illegal-argument-exception",
            "Micro3D command list is truncated",
        )
    })?;
    *cursor = end;
    Ok(values)
}

pub(super) fn micro3d_command_scissor(
    target_width: i32,
    target_height: i32,
    target_scissor: [u32; 4],
    clip: [i32; 4],
) -> Result<[u32; 4], EmuError> {
    let [left, top, right, bottom] = clip;
    if left > right || top > bottom {
        return Err(vm_error(
            "illegal-argument-exception",
            "Micro3D command clip range is inverted",
        ));
    }
    let target_width = i64::from(target_width.max(0));
    let target_height = i64::from(target_height.max(0));
    let left = i64::from(left).clamp(0, target_width);
    let top = i64::from(top).clamp(0, target_height);
    let right = i64::from(right).clamp(0, target_width);
    let bottom = i64::from(bottom).clamp(0, target_height);
    let base_left = i64::from(target_scissor[0]);
    let base_top = i64::from(target_scissor[1]);
    let base_right = base_left.saturating_add(i64::from(target_scissor[2]));
    let base_bottom = base_top.saturating_add(i64::from(target_scissor[3]));
    let left = left.max(base_left);
    let top = top.max(base_top);
    let right = right.min(base_right).max(left);
    let bottom = bottom.min(base_bottom).max(top);
    Ok([
        u32::try_from(left).unwrap_or(u32::MAX),
        u32::try_from(top).unwrap_or(u32::MAX),
        u32::try_from(right.saturating_sub(left)).unwrap_or(u32::MAX),
        u32::try_from(bottom.saturating_sub(top)).unwrap_or(u32::MAX),
    ])
}

pub(super) fn micro3d_primitive_payload_counts(
    command: i32,
    primitive_count: usize,
) -> Result<(usize, usize, usize, usize), EmuError> {
    let counts = micro3d::primitive_payload_counts(command, primitive_count)
        .map_err(|error| vm_error("illegal-argument-exception", error.message()))?;
    Ok((
        counts.vertex_count,
        counts.normal_count,
        counts.texture_coordinate_count,
        counts.color_count,
    ))
}

pub(super) fn micro3d_preserve_translation(
    mut rotation: micro3d::AffineTrans,
    original: micro3d::AffineTrans,
) -> micro3d::AffineTrans {
    for index in [3, 7, 11] {
        rotation.values[index] = original.values[index];
    }
    rotation
}

pub(super) fn normalize_m3g_resource(base: Option<&str>, uri: &str) -> Result<String, EmuError> {
    if uri.is_empty() || uri.contains('\\') {
        return Err(vm_error(
            "invalid-external-reference",
            "M3G resource name is empty or contains a backslash",
        ));
    }
    if uri.contains(':') {
        return Err(vm_error(
            "security-exception",
            "external network and file URIs are disabled by the offline suite policy",
        ));
    }
    let mut components = Vec::new();
    if !uri.starts_with('/')
        && let Some(base) = base
        && let Some((directory, _)) = base.rsplit_once('/')
    {
        components.extend(directory.split('/').filter(|part| !part.is_empty()));
    }
    for component in uri.trim_start_matches('/').split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(vm_error(
                        "security-exception",
                        "M3G resource path escapes the suite root",
                    ));
                }
            }
            value => components.push(value),
        }
    }
    if components.is_empty() {
        return Err(vm_error(
            "invalid-external-reference",
            "M3G resource resolves to an empty path",
        ));
    }
    Ok(components.join("/"))
}

pub(super) fn m3g_png_format(bytes: &[u8]) -> Result<m3g::ImageFormat, EmuError> {
    if bytes.len() < 33 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(vm_error("image-format", "invalid PNG resource"));
    }
    let color_type = bytes[25];
    let mut transparent = false;
    let mut offset = 8_usize;
    while offset < bytes.len() {
        let header_end = offset
            .checked_add(8)
            .ok_or_else(|| vm_error("image-format", "PNG chunk offset overflow"))?;
        let header = bytes
            .get(offset..header_end)
            .ok_or_else(|| vm_error("image-format", "truncated PNG chunk header"))?;
        let length = u32::from_be_bytes(header[..4].try_into().expect("four-byte PNG length"));
        let end = header_end
            .checked_add(length as usize)
            .and_then(|value| value.checked_add(4))
            .ok_or_else(|| vm_error("image-format", "PNG chunk length overflow"))?;
        if end > bytes.len() {
            return Err(vm_error("image-format", "PNG chunk exceeds resource"));
        }
        transparent |= &header[4..8] == b"tRNS";
        offset = end;
        if &header[4..8] == b"IEND" {
            break;
        }
    }
    match (color_type, transparent) {
        (0, false) => Ok(m3g::ImageFormat::Luminance),
        (0, true) | (4, _) => Ok(m3g::ImageFormat::LuminanceAlpha),
        (2 | 3, false) => Ok(m3g::ImageFormat::Rgb),
        (2 | 3, true) | (6, _) => Ok(m3g::ImageFormat::Rgba),
        _ => Err(vm_error("image-format", "unsupported PNG color type")),
    }
}

pub(super) const fn m3g_object_class(kind: m3g::ObjectType) -> Option<&'static str> {
    Some(match kind {
        m3g::ObjectType::Header | m3g::ObjectType::ExternalReference => return None,
        m3g::ObjectType::AnimationController => "javax/microedition/m3g/AnimationController",
        m3g::ObjectType::AnimationTrack => "javax/microedition/m3g/AnimationTrack",
        m3g::ObjectType::Appearance => "javax/microedition/m3g/Appearance",
        m3g::ObjectType::Background => "javax/microedition/m3g/Background",
        m3g::ObjectType::Camera => "javax/microedition/m3g/Camera",
        m3g::ObjectType::CompositingMode => "javax/microedition/m3g/CompositingMode",
        m3g::ObjectType::Fog => "javax/microedition/m3g/Fog",
        m3g::ObjectType::PolygonMode => "javax/microedition/m3g/PolygonMode",
        m3g::ObjectType::Group => "javax/microedition/m3g/Group",
        m3g::ObjectType::Image2D => "javax/microedition/m3g/Image2D",
        m3g::ObjectType::TriangleStripArray => "javax/microedition/m3g/TriangleStripArray",
        m3g::ObjectType::Light => "javax/microedition/m3g/Light",
        m3g::ObjectType::Material => "javax/microedition/m3g/Material",
        m3g::ObjectType::Mesh => "javax/microedition/m3g/Mesh",
        m3g::ObjectType::MorphingMesh => "javax/microedition/m3g/MorphingMesh",
        m3g::ObjectType::SkinnedMesh => "javax/microedition/m3g/SkinnedMesh",
        m3g::ObjectType::Texture2D => "javax/microedition/m3g/Texture2D",
        m3g::ObjectType::Sprite3D => "javax/microedition/m3g/Sprite3D",
        m3g::ObjectType::KeyframeSequence => "javax/microedition/m3g/KeyframeSequence",
        m3g::ObjectType::VertexArray => "javax/microedition/m3g/VertexArray",
        m3g::ObjectType::VertexBuffer => "javax/microedition/m3g/VertexBuffer",
        m3g::ObjectType::World => "javax/microedition/m3g/World",
    })
}

pub(super) fn m3g_scaled_sprite_half_extent(
    projection: m3g::Mat4,
    model_view: m3g::Mat4,
    center_camera: m3g::Vec4,
) -> Result<(f32, f32), EmuError> {
    let normalize = |value: m3g::Vec4| {
        if !value.w.is_finite() || value.w == 0.0 {
            return Err(vm_error(
                "arithmetic-exception",
                "scaled Sprite3D has a singular homogeneous transform",
            ));
        }
        let inverse = value.w.recip();
        Ok(m3g::Vec3::new(
            value.x * inverse,
            value.y * inverse,
            value.z * inverse,
        ))
    };
    let origin = normalize(center_camera)?;
    let x_axis = normalize(model_view.transform(m3g::Vec4::new(1.0, 0.0, 0.0, 1.0)))?;
    let y_axis = normalize(model_view.transform(m3g::Vec4::new(0.0, 1.0, 0.0, 1.0)))?;
    let distance = |value: m3g::Vec3| {
        (value.x - origin.x)
            .hypot(value.y - origin.y)
            .hypot(value.z - origin.z)
    };
    let dx = distance(x_axis);
    let dy = distance(y_axis);
    let projected_origin = normalize(projection.transform(center_camera))?;
    let projected_x = normalize(projection.transform(m3g::Vec4::new(
        center_camera.x + dx,
        center_camera.y,
        center_camera.z,
        center_camera.w,
    )))?;
    let projected_y = normalize(projection.transform(m3g::Vec4::new(
        center_camera.x,
        center_camera.y + dy,
        center_camera.z,
        center_camera.w,
    )))?;
    let width = distance_from(projected_origin, projected_x);
    let height = distance_from(projected_origin, projected_y);
    if !width.is_finite() || !height.is_finite() {
        return Err(vm_error(
            "arithmetic-exception",
            "scaled Sprite3D projection is non-finite",
        ));
    }
    Ok((width * 0.5, height * 0.5))
}

pub(super) fn m3g_sprite_crop_coordinate(origin: i32, extent: i32, position: f32) -> f32 {
    let span = extent.unsigned_abs() as f32;
    let position = if extent < 0 { 1.0 - position } else { position };
    position.mul_add(span, origin as f32)
}

pub(super) fn m3g_sprite_crop_sample(origin: i32, extent: i32, position: f32) -> i32 {
    let span = extent.unsigned_abs();
    if span == 0 {
        return origin;
    }
    let position = if extent < 0 {
        1.0 - position.clamp(0.0, 1.0)
    } else {
        position.clamp(0.0, 1.0)
    };
    let offset = (position * span as f32).floor() as u32;
    let offset = offset.min(span - 1);
    i64::from(origin)
        .saturating_add(i64::from(offset))
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

pub(super) fn distance_from(first: m3g::Vec3, second: m3g::Vec3) -> f32 {
    (second.x - first.x)
        .hypot(second.y - first.y)
        .hypot(second.z - first.z)
}
