//! Typed validation of serialized scene objects and their references.

use super::{EmuError, ObjectType, ParsedObject, loader_error};
use crate::geometry::MAX_TRIANGLE_STRIP_INDICES;

pub(super) fn validate_object_payloads(objects: &[ParsedObject]) -> Result<(), EmuError> {
    for object in objects.iter().skip(1) {
        if object.external_uri()?.is_some() {
            continue;
        }
        let mut cursor = ObjectCursor::new(object, objects);
        cursor.object3d()?;
        if object.object_type.is_transformable() {
            cursor.transformable()?;
        }
        if object.object_type.is_node() {
            cursor.node()?;
        }
        if object.object_type.is_group() {
            cursor.group()?;
        }
        if object.object_type.is_mesh() {
            cursor.mesh()?;
        }
        match object.object_type {
            ObjectType::Header | ObjectType::ExternalReference => unreachable!(),
            ObjectType::AnimationController => {
                cursor.f32()?;
                cursor.non_negative_f32()?;
                let start = cursor.i32()?;
                let end = cursor.i32()?;
                if start > end {
                    return Err(
                        cursor.error("invalid-active-interval", "animation interval is reversed")
                    );
                }
                cursor.f32()?;
                cursor.i32()?;
            }
            ObjectType::AnimationTrack => {
                cursor.reference(false)?;
                cursor.reference(true)?;
                if !(256..=276).contains(&cursor.u32()?) {
                    return Err(cursor.error("invalid-enum", "invalid AnimationTrack property"));
                }
            }
            ObjectType::Appearance => {
                cursor.byte()?;
                for _ in 0..4 {
                    cursor.reference(true)?;
                }
                let count = cursor.count(2, 4)?;
                for _ in 0..count {
                    cursor.reference(false)?;
                }
            }
            ObjectType::Background => {
                cursor.skip(4)?;
                cursor.reference(true)?;
                for _ in 0..2 {
                    if !matches!(cursor.byte()?, 32 | 33) {
                        return Err(cursor.error("invalid-enum", "invalid Background image mode"));
                    }
                }
                let crop = [cursor.i32()?, cursor.i32()?, cursor.i32()?, cursor.i32()?];
                crate::BackgroundState::validate_crop(crop)
                    .map_err(|error| cursor.error(error.code(), error.message()))?;
                cursor.boolean()?;
                cursor.boolean()?;
            }
            ObjectType::Camera => {
                let projection = cursor.byte()?;
                match projection {
                    48 => cursor.matrix()?,
                    49 | 50 => {
                        let first = cursor.f32()?;
                        let aspect = cursor.f32()?;
                        let near = cursor.f32()?;
                        let far = cursor.f32()?;
                        let parameters = if projection == 49 {
                            crate::CameraProjection::Parallel {
                                height: first,
                                aspect_ratio: aspect,
                                near,
                                far,
                            }
                        } else {
                            crate::CameraProjection::Perspective {
                                field_of_view: first,
                                aspect_ratio: aspect,
                                near,
                                far,
                            }
                        };
                        parameters
                            .validate()
                            .map_err(|error| cursor.error(error.code(), error.message()))?;
                    }
                    _ => return Err(cursor.error("invalid-enum", "invalid Camera projection type")),
                }
            }
            ObjectType::CompositingMode => {
                for _ in 0..4 {
                    cursor.boolean()?;
                }
                if !matches!(cursor.byte()?, 64..=68) {
                    return Err(cursor.error("invalid-enum", "invalid blending mode"));
                }
                cursor.byte()?;
                cursor.f32()?;
                cursor.f32()?;
            }
            ObjectType::PolygonMode => {
                if !matches!(cursor.byte()?, 160..=162) {
                    return Err(cursor.error("invalid-enum", "invalid PolygonMode culling"));
                }
                if !matches!(cursor.byte()?, 164 | 165) {
                    return Err(cursor.error("invalid-enum", "invalid PolygonMode shading"));
                }
                if !matches!(cursor.byte()?, 168 | 169) {
                    return Err(cursor.error("invalid-enum", "invalid PolygonMode winding"));
                }
                cursor.boolean()?;
                cursor.boolean()?;
                cursor.boolean()?;
            }
            ObjectType::Fog => {
                cursor.skip(3)?;
                match cursor.byte()? {
                    80 => {
                        cursor.non_negative_f32()?;
                    }
                    81 => {
                        cursor.f32()?;
                        cursor.f32()?;
                    }
                    _ => return Err(cursor.error("invalid-enum", "invalid Fog mode")),
                }
            }
            ObjectType::Group | ObjectType::Mesh => {}
            ObjectType::Image2D => cursor.image2d()?,
            ObjectType::TriangleStripArray => cursor.triangle_strips()?,
            ObjectType::Light => {
                let attenuation = [
                    cursor.non_negative_f32()?,
                    cursor.non_negative_f32()?,
                    cursor.non_negative_f32()?,
                ];
                if attenuation
                    .iter()
                    .all(|value| value.classify() == std::num::FpCategory::Zero)
                {
                    return Err(cursor.error("invalid-light", "all attenuation terms are zero"));
                }
                cursor.skip(3)?;
                if !(128..=131).contains(&cursor.byte()?) {
                    return Err(cursor.error("invalid-enum", "invalid Light mode"));
                }
                cursor.f32()?;
                if cursor.non_negative_f32()? > 90.0 || cursor.non_negative_f32()? > 128.0 {
                    return Err(cursor.error("invalid-light", "invalid spot parameters"));
                }
            }
            ObjectType::Material => {
                cursor.skip(13)?;
                if cursor.non_negative_f32()? > 128.0 {
                    return Err(cursor.error("invalid-material", "shininess exceeds 128"));
                }
                cursor.boolean()?;
            }
            ObjectType::MorphingMesh => {
                let count = cursor.count(262_144, 8)?;
                for _ in 0..count {
                    cursor.reference(false)?;
                    cursor.f32()?;
                }
            }
            ObjectType::SkinnedMesh => {
                cursor.reference(false)?;
                let count = cursor.count(262_144, 16)?;
                for _ in 0..count {
                    cursor.reference(false)?;
                    cursor.u32()?;
                    if cursor.u32()? == 0 || cursor.i32()? <= 0 {
                        return Err(
                            cursor.error("invalid-skinning", "invalid skin range or weight")
                        );
                    }
                }
            }
            ObjectType::Texture2D => {
                cursor.reference(false)?;
                cursor.skip(3)?;
                if !(224..=228).contains(&cursor.byte()?) {
                    return Err(cursor.error("invalid-enum", "invalid texture function"));
                }
                for _ in 0..2 {
                    if !matches!(cursor.byte()?, 240 | 241) {
                        return Err(cursor.error("invalid-enum", "invalid texture wrapping"));
                    }
                }
                if !(208..=210).contains(&cursor.byte()?) || !matches!(cursor.byte()?, 209 | 210) {
                    return Err(cursor.error("invalid-enum", "invalid texture filter"));
                }
            }
            ObjectType::Sprite3D => {
                cursor.reference(false)?;
                cursor.reference(true)?;
                cursor.boolean()?;
                for _ in 0..4 {
                    cursor.i32()?;
                }
            }
            ObjectType::KeyframeSequence => cursor.keyframes()?,
            ObjectType::VertexArray => cursor.vertex_array()?,
            ObjectType::VertexBuffer => cursor.vertex_buffer()?,
            ObjectType::World => {
                cursor.reference(true)?;
                cursor.reference(true)?;
            }
        }
        cursor.finish()?;
    }
    Ok(())
}

struct ObjectCursor<'a> {
    object: &'a ParsedObject,
    objects: &'a [ParsedObject],
    offset: usize,
}

impl<'a> ObjectCursor<'a> {
    const fn new(object: &'a ParsedObject, objects: &'a [ParsedObject]) -> Self {
        Self {
            object,
            objects,
            offset: 0,
        }
    }

    fn object3d(&mut self) -> Result<(), EmuError> {
        self.u32()?;
        let tracks = self.count(262_144, 4)?;
        for _ in 0..tracks {
            self.reference(false)?;
        }
        let parameters = self.count(16_384, 8)?;
        let mut ids = std::collections::BTreeSet::new();
        for _ in 0..parameters {
            if !ids.insert(self.u32()?) {
                return Err(self.error("duplicate-user-parameter", "duplicate user parameter ID"));
            }
            let bytes = self.count(8 * 1024 * 1024, 1)?;
            self.skip(bytes)?;
        }
        Ok(())
    }

    fn transformable(&mut self) -> Result<(), EmuError> {
        if self.boolean()? {
            for _ in 0..10 {
                self.f32()?;
            }
        }
        if self.boolean()? {
            self.matrix()?;
        }
        Ok(())
    }

    fn node(&mut self) -> Result<(), EmuError> {
        self.boolean()?;
        self.boolean()?;
        self.byte()?;
        self.u32()?;
        if self.boolean()? {
            for _ in 0..2 {
                if !(144..=148).contains(&self.byte()?) {
                    return Err(self.error("invalid-enum", "invalid Node alignment target"));
                }
            }
            self.reference(true)?;
            self.reference(true)?;
        }
        Ok(())
    }

    fn group(&mut self) -> Result<(), EmuError> {
        let children = self.count(16_384, 4)?;
        for _ in 0..children {
            self.reference(false)?;
        }
        Ok(())
    }

    fn mesh(&mut self) -> Result<(), EmuError> {
        self.reference(false)?;
        let submeshes = self.count(262_144, 8)?;
        if submeshes == 0 {
            return Err(self.error("invalid-mesh", "Mesh has no submeshes"));
        }
        for _ in 0..submeshes {
            self.reference(false)?;
            self.reference(true)?;
        }
        Ok(())
    }

    fn image2d(&mut self) -> Result<(), EmuError> {
        if !(96..=100).contains(&self.byte()?) {
            return Err(self.error("invalid-enum", "invalid Image2D format"));
        }
        let mutable = self.boolean()?;
        let width = self.u32()?;
        let height = self.u32()?;
        if width == 0 || height == 0 || width > 1_024 || height > 1_024 {
            return Err(self.error("resource-limit", "Image2D dimensions exceed profile"));
        }
        if !mutable {
            for _ in 0..2 {
                let bytes = self.count(8 * 1024 * 1024, 1)?;
                self.skip(bytes)?;
            }
        }
        Ok(())
    }

    fn triangle_strips(&mut self) -> Result<(), EmuError> {
        let explicit_indices = match self.byte()? {
            0 => {
                self.u32()?;
                None
            }
            1 => {
                self.byte()?;
                None
            }
            2 => {
                self.u16()?;
                None
            }
            128 => {
                let n = self.count(MAX_TRIANGLE_STRIP_INDICES, 4)?;
                self.skip(n * 4)?;
                Some(n)
            }
            129 => {
                let n = self.count(MAX_TRIANGLE_STRIP_INDICES, 1)?;
                self.skip(n)?;
                Some(n)
            }
            130 => {
                let n = self.count(MAX_TRIANGLE_STRIP_INDICES, 2)?;
                self.skip(n * 2)?;
                Some(n)
            }
            _ => return Err(self.error("invalid-enum", "invalid strip encoding")),
        };
        let strips = self.count(MAX_TRIANGLE_STRIP_INDICES / 3, 4)?;
        let mut indices = 0_usize;
        for _ in 0..strips {
            let length = self.u32()? as usize;
            if length < 3 {
                return Err(self.error("invalid-strips", "strip length is below three"));
            }
            indices = indices
                .checked_add(length)
                .filter(|count| *count <= MAX_TRIANGLE_STRIP_INDICES)
                .ok_or_else(|| {
                    self.error(
                        "invalid-strips",
                        "combined strip length exceeds the index limit",
                    )
                })?;
        }
        if explicit_indices.is_some_and(|available| indices > available) {
            return Err(self.error("invalid-strips", "strip lengths exceed explicit index data"));
        }
        Ok(())
    }

    fn keyframes(&mut self) -> Result<(), EmuError> {
        if !(176..=180).contains(&self.byte()?) || !matches!(self.byte()?, 192 | 193) {
            return Err(self.error("invalid-enum", "invalid keyframe mode"));
        }
        let encoding = self.byte()?;
        if encoding > 2 {
            return Err(self.error("invalid-enum", "invalid keyframe encoding"));
        }
        let duration = self.u32()?;
        let first = self.u32()?;
        let last = self.u32()?;
        let components = self.u32()? as usize;
        let count = self.u32()? as usize;
        if duration == 0
            || components == 0
            || components > 16
            || count == 0
            || count > 262_144
            || first as usize >= count
            || last as usize >= count
        {
            return Err(self.error("invalid-keyframes", "invalid keyframe shape or range"));
        }
        if encoding != 0 {
            for _ in 0..components * 2 {
                self.f32()?;
            }
        }
        for _ in 0..count {
            self.u32()?;
            match encoding {
                0 => {
                    for _ in 0..components {
                        self.f32()?;
                    }
                }
                1 => self.skip(components)?,
                _ => self.skip(components * 2)?,
            }
        }
        Ok(())
    }

    fn vertex_array(&mut self) -> Result<(), EmuError> {
        let size = self.byte()? as usize;
        let components = self.byte()? as usize;
        let encoding = self.byte()?;
        let vertices = self.u16()? as usize;
        if !matches!(size, 1 | 2) || !(2..=4).contains(&components) || encoding > 1 || vertices == 0
        {
            return Err(self.error("invalid-vertex-array", "invalid VertexArray shape"));
        }
        self.skip(size * components * vertices)
    }

    fn vertex_buffer(&mut self) -> Result<(), EmuError> {
        self.skip(4)?;
        self.reference(true)?;
        for _ in 0..4 {
            self.f32()?;
        }
        self.reference(true)?;
        self.reference(true)?;
        let count = self.count(2, 20)?;
        for _ in 0..count {
            self.reference(false)?;
            for _ in 0..4 {
                self.f32()?;
            }
        }
        Ok(())
    }

    fn reference(&mut self, nullable: bool) -> Result<u32, EmuError> {
        let reference = self.u32()?;
        if reference == 0 && !nullable {
            return Err(self.error("null-reference", "required object reference is null"));
        }
        if reference > self.object.index || reference as usize > self.objects.len() {
            return Err(self.error(
                "dangling-reference",
                "object reference points forward or outside file",
            ));
        }
        Ok(reference)
    }

    fn matrix(&mut self) -> Result<(), EmuError> {
        for _ in 0..16 {
            self.f32()?;
        }
        Ok(())
    }

    fn count(&mut self, limit: usize, element_size: usize) -> Result<usize, EmuError> {
        let count = self.u32()? as usize;
        if count > limit
            || count
                .checked_mul(element_size)
                .is_none_or(|bytes| bytes > self.remaining())
        {
            return Err(self.error(
                "resource-limit",
                "array count exceeds object payload or budget",
            ));
        }
        Ok(count)
    }

    fn boolean(&mut self) -> Result<bool, EmuError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.error("invalid-boolean", "boolean is not encoded as 0 or 1")),
        }
    }

    fn f32(&mut self) -> Result<f32, EmuError> {
        super::decode_f32(self.u32()?).map_err(|error| self.error(error.code(), error.message()))
    }

    fn non_negative_f32(&mut self) -> Result<f32, EmuError> {
        let value = self.f32()?;
        if value >= 0.0 {
            Ok(value)
        } else {
            Err(self.error("invalid-range", "float is negative"))
        }
    }

    fn i32(&mut self) -> Result<i32, EmuError> {
        Ok(self.u32()? as i32)
    }
    fn u32(&mut self) -> Result<u32, EmuError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u16(&mut self) -> Result<u16, EmuError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn byte(&mut self) -> Result<u8, EmuError> {
        let [value] = self.array()?;
        Ok(value)
    }
    fn skip(&mut self, count: usize) -> Result<(), EmuError> {
        self.take(count).map(|_| ())
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], EmuError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| self.error("length-overflow", "object field overflow"))?;
        let bytes =
            self.object.data.get(self.offset..end).ok_or_else(|| {
                self.error("truncated-object-data", "object field exceeds payload")
            })?;
        self.offset = end;
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], EmuError> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }
    fn remaining(&self) -> usize {
        self.object.data.len() - self.offset
    }
    fn finish(&self) -> Result<(), EmuError> {
        if self.offset == self.object.data.len() {
            Ok(())
        } else {
            Err(self.error("extra-object-data", "unconsumed object payload"))
        }
    }
    fn error(&self, code: &'static str, message: &str) -> EmuError {
        loader_error(
            code,
            format!(
                "section {} object {} type {:?} offset {}: {message}",
                self.object.section_offset, self.object.index, self.object.object_type, self.offset,
            ),
        )
    }
}
