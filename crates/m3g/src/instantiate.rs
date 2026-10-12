//! Atomic instantiation of an already validated M3G file into a suite runtime.

use crate::{Handle, M3gFile, Mat4, ObjectKind, ObjectType, Runtime, Vec3};
use diagnostics::{Category, EmuError};

mod objects;
use objects::decode_kind;

struct Common {
    user_id: i32,
    tracks: Vec<u32>,
    parameters: Vec<(u32, Vec<u8>)>,
}

/// Native objects and serialized user parameters produced by one atomic load.
#[derive(Debug)]
pub struct InstantiatedFile {
    /// Native handles indexed like [`M3gFile::objects`].
    pub handles: Vec<Option<Handle>>,
    /// User parameters indexed like [`M3gFile::objects`].
    pub user_parameters: Vec<Vec<(u32, Vec<u8>)>>,
}

struct Transformable {
    components: Option<([f32; 3], [f32; 3], [f32; 4])>,
    matrix: Option<Mat4>,
}

struct Node {
    rendering: bool,
    picking: bool,
    alpha: f32,
    scope: u32,
    alignment: Option<[(u8, u32); 2]>,
}

struct MeshData {
    vertices: u32,
    submeshes: Vec<u32>,
    appearances: Vec<u32>,
}

/// Instantiates all non-header objects in file order. The guest-reference slice must
/// have one entry per file object; external entries may be pre-resolved to an existing
/// native handle through `external_handles`.
pub fn instantiate_file(
    file: &M3gFile,
    runtime: &mut Runtime,
    guest_references: &[Option<u64>],
    external_handles: &[Option<Handle>],
) -> Result<InstantiatedFile, EmuError> {
    if guest_references.len() != file.objects.len()
        || external_handles.len() != file.objects.len()
        || file.objects.is_empty()
        || file.objects.iter().enumerate().any(|(slot, object)| {
            object.index as usize != slot + 1
                || (object.object_type == ObjectType::Header) != (slot == 0)
        })
    {
        return Err(error(
            "instantiate-shape",
            "object/guest/external tables have invalid lengths or object order",
        ));
    }
    let mut created = Vec::new();
    let result = (|| {
        let mut handles = vec![None; file.objects.len()];
        let mut user_parameters = vec![Vec::new(); file.objects.len()];
        for (slot, object) in file.objects.iter().enumerate().skip(1) {
            if object.object_type == ObjectType::ExternalReference {
                handles[slot] = external_handles[slot];
                continue;
            }
            let mut cursor = Cursor::new(&object.data);
            let common = cursor.common()?;
            let transformable = object
                .object_type
                .is_transformable()
                .then(|| cursor.transformable())
                .transpose()?;
            let node = object
                .object_type
                .is_node()
                .then(|| cursor.node())
                .transpose()?;
            let children = object
                .object_type
                .is_group()
                .then(|| cursor.references())
                .transpose()?;
            let mesh = object
                .object_type
                .is_mesh()
                .then(|| cursor.mesh())
                .transpose()?;
            let kind = decode_kind(
                object.object_type,
                &mut cursor,
                mesh.as_ref(),
                &handles,
                runtime,
            )?;
            cursor.finish()?;
            let handle = runtime.create(guest_references[slot], kind)?;
            created.push(handle);
            handles[slot] = Some(handle);
            runtime.set_user_id(handle, common.user_id)?;
            user_parameters[slot] = common.parameters;
            for track in common.tracks {
                runtime.add_animation_track(handle, resolve(&handles, track)?)?;
            }
            if let Some(transformable) = transformable {
                if let Some((translation, scale, orientation)) = transformable.components {
                    runtime.set_translation(
                        handle,
                        Vec3::new(translation[0], translation[1], translation[2]),
                    )?;
                    runtime.set_scale(handle, Vec3::new(scale[0], scale[1], scale[2]))?;
                    runtime.set_orientation(
                        handle,
                        orientation[0],
                        Vec3::new(orientation[1], orientation[2], orientation[3]),
                    )?;
                }
                if let Some(matrix) = transformable.matrix {
                    runtime.set_transform(handle, matrix)?;
                }
            }
            if let Some(node) = node {
                runtime.set_node_state(
                    handle,
                    node.rendering,
                    node.picking,
                    node.scope,
                    node.alpha,
                )?;
                if let Some(alignment) = node.alignment {
                    runtime.set_alignment(
                        handle,
                        0,
                        resolve_nullable(&handles, alignment[0].1)?,
                        i32::from(alignment[0].0),
                    )?;
                    runtime.set_alignment(
                        handle,
                        1,
                        resolve_nullable(&handles, alignment[1].1)?,
                        i32::from(alignment[1].0),
                    )?;
                }
            }
            if let Some(children) = children {
                for child in children {
                    runtime.add_child(handle, resolve(&handles, child)?)?;
                }
            }
        }
        for handle in created.iter().copied() {
            if matches!(runtime.kind(handle)?, ObjectKind::SkinnedMesh { .. }) {
                runtime.bind_skin_skeleton(handle)?;
            }
        }
        Ok(InstantiatedFile {
            handles,
            user_parameters,
        })
    })();
    if result.is_err() {
        runtime.rollback_created(&created);
    }
    result
}

fn resolve(handles: &[Option<Handle>], reference: u32) -> Result<Handle, EmuError> {
    if reference == 0 {
        return Err(error("null-reference", "required reference is null"));
    }
    handles
        .get(reference as usize - 1)
        .copied()
        .flatten()
        .ok_or_else(|| {
            error(
                "unresolved-reference",
                "referenced object was not instantiated",
            )
        })
}

fn resolve_nullable(
    handles: &[Option<Handle>],
    reference: u32,
) -> Result<Option<Handle>, EmuError> {
    if reference == 0 {
        Ok(None)
    } else {
        resolve(handles, reference).map(Some)
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn common(&mut self) -> Result<Common, EmuError> {
        let user_id = self.i32()?;
        let tracks = self.references()?;
        let parameters = self.u32()?;
        let mut user_parameters = Vec::new();
        for _ in 0..parameters {
            let id = self.u32()?;
            let length = self.u32()? as usize;
            user_parameters.push((id, self.take(length)?.to_vec()));
        }
        Ok(Common {
            user_id,
            tracks,
            parameters: user_parameters,
        })
    }
    fn transformable(&mut self) -> Result<Transformable, EmuError> {
        let components = self
            .boolean()?
            .then(|| {
                let mut values = [0.0; 10];
                for value in &mut values {
                    *value = self.f32()?;
                }
                Ok::<_, EmuError>((
                    [values[0], values[1], values[2]],
                    [values[3], values[4], values[5]],
                    [values[6], values[7], values[8], values[9]],
                ))
            })
            .transpose()?;
        let matrix = self.boolean()?.then(|| self.matrix()).transpose()?;
        Ok(Transformable { components, matrix })
    }
    fn node(&mut self) -> Result<Node, EmuError> {
        let rendering = self.boolean()?;
        let picking = self.boolean()?;
        let alpha = f32::from(self.byte()?) / 255.0;
        let scope = self.u32()?;
        let alignment = self
            .boolean()?
            .then(|| {
                let first_target = self.byte()?;
                let second_target = self.byte()?;
                let first_reference = self.u32()?;
                let second_reference = self.u32()?;
                Ok::<_, EmuError>([
                    (first_target, first_reference),
                    (second_target, second_reference),
                ])
            })
            .transpose()?;
        Ok(Node {
            rendering,
            picking,
            alpha,
            scope,
            alignment,
        })
    }
    fn references(&mut self) -> Result<Vec<u32>, EmuError> {
        let count = self.u32()?;
        (0..count).map(|_| self.u32()).collect()
    }
    fn record_count(&mut self, record_bytes: usize) -> Result<usize, EmuError> {
        let count = self.u32()? as usize;
        if count > (self.bytes.len() - self.offset) / record_bytes {
            return Err(error(
                "truncated-object-data",
                "record count exceeds decoded object payload",
            ));
        }
        Ok(count)
    }
    fn mesh(&mut self) -> Result<MeshData, EmuError> {
        let vertices = self.u32()?;
        let count = self.u32()?;
        let mut submeshes = Vec::new();
        let mut appearances = Vec::new();
        for _ in 0..count {
            submeshes.push(self.u32()?);
            appearances.push(self.u32()?);
        }
        Ok(MeshData {
            vertices,
            submeshes,
            appearances,
        })
    }
    fn matrix(&mut self) -> Result<Mat4, EmuError> {
        let mut values = [0.0; 16];
        for value in &mut values {
            *value = self.f32()?;
        }
        Mat4::from_row_major(values)
    }
    fn bytes_with_length(&mut self) -> Result<&'a [u8], EmuError> {
        let length = self.u32()? as usize;
        self.take(length)
    }
    fn argb(&mut self) -> Result<u32, EmuError> {
        let [red, green, blue, alpha] = self.array()?;
        Ok(u32::from(alpha) << 24 | u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue))
    }
    fn rgb(&mut self) -> Result<u32, EmuError> {
        let [red, green, blue] = self.array()?;
        Ok(u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue))
    }
    fn boolean(&mut self) -> Result<bool, EmuError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(error("invalid-boolean", "invalid boolean")),
        }
    }
    fn f32(&mut self) -> Result<f32, EmuError> {
        crate::loader::decode_f32(self.u32()?)
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
    fn take(&mut self, count: usize) -> Result<&'a [u8], EmuError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| error("length-overflow", "object offset overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| error("truncated-object-data", "truncated decoded object"))?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], EmuError> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }
    fn finish(&self) -> Result<(), EmuError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(error(
                "extra-object-data",
                "decoded object has trailing bytes",
            ))
        }
    }
}

fn error(code: &'static str, message: &'static str) -> EmuError {
    EmuError::new(Category::M3g, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/m3g/instantiate/mod.rs"]
mod tests;
