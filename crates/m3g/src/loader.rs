//! Strict, bounded parser for the JSR-184 file container.

use diagnostics::{Category, EmuError};
use std::sync::Arc;

mod header;
mod object_validation;
mod sections;

use header::parse_header_section;
use object_validation::validate_object_payloads;
use sections::{parse_objects, parse_section};

#[cfg(test)]
use sections::adler32;

/// Twelve-byte M3G file identifier from the normative file-format specification.
pub const FILE_IDENTIFIER: [u8; 12] = [
    0xab, b'J', b'S', b'R', b'1', b'8', b'4', 0xbb, 0x0d, 0x0a, 0x1a, 0x0a,
];

/// Parser and decompression limits checked before allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoaderLimits {
    /// Maximum input file size.
    pub file_bytes: usize,
    /// Maximum sum of uncompressed object payloads.
    pub decompressed_bytes: usize,
    /// Maximum number of sections.
    pub sections: usize,
    /// Maximum number of serialized objects.
    pub objects: usize,
    /// Maximum data payload for one object.
    pub object_bytes: usize,
}

impl Default for LoaderLimits {
    fn default() -> Self {
        Self {
            file_bytes: 8 * 1024 * 1024,
            decompressed_bytes: 16 * 1024 * 1024,
            sections: 1_024,
            objects: 16_384,
            object_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Serializable object type in the M3G 1.0/1.1 file format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ObjectType {
    /// File header.
    Header = 0,
    /// Animation controller.
    AnimationController = 1,
    /// Animation track.
    AnimationTrack = 2,
    /// Appearance.
    Appearance = 3,
    /// Background.
    Background = 4,
    /// Camera.
    Camera = 5,
    /// Compositing mode.
    CompositingMode = 6,
    /// Fog.
    Fog = 7,
    /// Polygon mode.
    PolygonMode = 8,
    /// Group.
    Group = 9,
    /// `Image2D`.
    Image2D = 10,
    /// Triangle strip array.
    TriangleStripArray = 11,
    /// Light.
    Light = 12,
    /// Material.
    Material = 13,
    /// Mesh.
    Mesh = 14,
    /// Morphing mesh.
    MorphingMesh = 15,
    /// Skinned mesh.
    SkinnedMesh = 16,
    /// `Texture2D`.
    Texture2D = 17,
    /// `Sprite3D`.
    Sprite3D = 18,
    /// Keyframe sequence.
    KeyframeSequence = 19,
    /// Vertex array.
    VertexArray = 20,
    /// Vertex buffer.
    VertexBuffer = 21,
    /// World.
    World = 22,
    /// External reference URI.
    ExternalReference = 255,
}

impl ObjectType {
    pub(crate) const fn is_transformable(self) -> bool {
        matches!(
            self,
            Self::Camera
                | Self::Group
                | Self::Light
                | Self::Mesh
                | Self::MorphingMesh
                | Self::SkinnedMesh
                | Self::Texture2D
                | Self::Sprite3D
                | Self::World
        )
    }

    pub(crate) const fn is_node(self) -> bool {
        self.is_transformable() && !matches!(self, Self::Texture2D)
    }

    pub(crate) const fn is_group(self) -> bool {
        matches!(self, Self::Group | Self::World)
    }

    pub(crate) const fn is_mesh(self) -> bool {
        matches!(self, Self::Mesh | Self::MorphingMesh | Self::SkinnedMesh)
    }
}

impl TryFrom<u8> for ObjectType {
    type Error = EmuError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Header,
            1 => Self::AnimationController,
            2 => Self::AnimationTrack,
            3 => Self::Appearance,
            4 => Self::Background,
            5 => Self::Camera,
            6 => Self::CompositingMode,
            7 => Self::Fog,
            8 => Self::PolygonMode,
            9 => Self::Group,
            10 => Self::Image2D,
            11 => Self::TriangleStripArray,
            12 => Self::Light,
            13 => Self::Material,
            14 => Self::Mesh,
            15 => Self::MorphingMesh,
            16 => Self::SkinnedMesh,
            17 => Self::Texture2D,
            18 => Self::Sprite3D,
            19 => Self::KeyframeSequence,
            20 => Self::VertexArray,
            21 => Self::VertexBuffer,
            22 => Self::World,
            255 => Self::ExternalReference,
            _ => {
                return Err(loader_error(
                    "unknown-object-type",
                    format!("reserved M3G object type {value}"),
                ));
            }
        })
    }
}

/// Validated file header object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileHeader {
    /// Major file format version; must be 1.
    pub major_version: u8,
    /// Minor file format version; must be 0.
    pub minor_version: u8,
    /// Whether section 1 contains external references.
    pub has_external_references: bool,
    /// Exact serialized file size.
    pub total_file_size: u32,
    /// Approximate content size hint.
    pub approximate_content_size: u32,
    /// NUL-terminated UTF-8 authoring string.
    pub authoring_field: String,
}

/// One validated object record. `data` is retained for the typed validation phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedObject {
    /// One-based file object index.
    pub index: u32,
    /// Normative object type.
    pub object_type: ObjectType,
    /// Exact uncompressed data shared by the section and file object indexes.
    pub data: Arc<[u8]>,
    /// Byte offset of the enclosing section in the source file.
    pub section_offset: usize,
}

impl ParsedObject {
    /// Returns the validated external-reference URI, when this is such an object.
    pub fn external_uri(&self) -> Result<Option<&str>, EmuError> {
        if self.object_type != ObjectType::ExternalReference {
            return Ok(None);
        }
        let end = self
            .data
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| object_error(self, "unterminated-string", "external URI has no NUL"))?;
        if end + 1 != self.data.len() {
            return Err(object_error(
                self,
                "invalid-uri",
                "external URI has trailing bytes",
            ));
        }
        std::str::from_utf8(&self.data[..end])
            .map(Some)
            .map_err(|_| object_error(self, "invalid-uri", "external URI is not UTF-8"))
    }
}

/// One checksum-verified M3G section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    /// Byte offset in the source file.
    pub offset: usize,
    /// Whether the object stream used zlib compression.
    pub compressed: bool,
    /// Declared section length including its checksum.
    pub total_length: u32,
    /// Declared and verified uncompressed object-stream length.
    pub uncompressed_length: u32,
    /// Objects parsed from this section.
    pub objects: Vec<ParsedObject>,
}

/// Completely parsed container. Construction is atomic: errors return no partial file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M3gFile {
    /// Parsed header.
    pub header: FileHeader,
    /// All sections including the header section.
    pub sections: Vec<Section>,
    /// All objects in one-based reference order.
    pub objects: Vec<ParsedObject>,
}

impl M3gFile {
    /// Parses, inflates and checksum-verifies a complete M3G byte stream.
    pub fn parse(bytes: &[u8], limits: LoaderLimits) -> Result<Self, EmuError> {
        if bytes.len() > limits.file_bytes {
            return Err(loader_error(
                "resource-limit",
                "M3G file exceeds configured byte budget",
            ));
        }
        let (file, consumed) = Self::parse_prefix(bytes, limits)?;
        if consumed != bytes.len() {
            return Err(loader_error(
                "file-size-mismatch",
                "header total file size does not match the byte stream",
            ));
        }
        Ok(file)
    }

    /// Parses one M3G file at the start of a larger byte buffer.
    ///
    /// `Loader.load(byte[], offset)` permits unrelated bytes to follow the
    /// serialized file. The exact file boundary is carried by the header
    /// object, so callers must not treat the remainder of the array as M3G
    /// sections.
    pub fn parse_prefix(bytes: &[u8], limits: LoaderLimits) -> Result<(Self, usize), EmuError> {
        if !bytes.starts_with(&FILE_IDENTIFIER) {
            return Err(loader_error(
                "invalid-identifier",
                "missing or malformed M3G file identifier",
            ));
        }
        if limits.sections == 0 {
            return Err(loader_error(
                "resource-limit",
                "M3G section count exceeds configured budget",
            ));
        }

        let mut decompressed_total = 0_usize;
        let header_section = parse_section(
            bytes,
            FILE_IDENTIFIER.len(),
            limits,
            &mut decompressed_total,
            0,
        )?;
        let header = parse_header_section(&header_section)?;
        let file_size = usize::try_from(header.total_file_size)
            .map_err(|_| loader_error("resource-limit", "M3G file size exceeds host limits"))?;
        if file_size > limits.file_bytes {
            return Err(loader_error(
                "resource-limit",
                "M3G file exceeds configured byte budget",
            ));
        }
        if file_size > bytes.len() {
            return Err(loader_error(
                "truncated-file",
                "header total file size extends past the byte buffer",
            ));
        }

        let mut cursor = FILE_IDENTIFIER
            .len()
            .checked_add(header_section.total_length as usize)
            .ok_or_else(|| loader_error("length-overflow", "section offset overflow"))?;
        if file_size < cursor {
            return Err(loader_error(
                "file-size-mismatch",
                "header total file size ends inside the header section",
            ));
        }
        let bytes = &bytes[..file_size];
        let mut objects = header_section.objects.clone();
        let mut sections = vec![header_section];
        while cursor < bytes.len() {
            if sections.len() >= limits.sections {
                return Err(loader_error(
                    "resource-limit",
                    "M3G section count exceeds configured budget",
                ));
            }
            let section = parse_section(
                bytes,
                cursor,
                limits,
                &mut decompressed_total,
                objects.len(),
            )?;
            cursor = cursor
                .checked_add(section.total_length as usize)
                .ok_or_else(|| loader_error("length-overflow", "section offset overflow"))?;
            objects.extend(section.objects.iter().cloned());
            sections.push(section);
        }
        let file = validate_container(header, sections, objects)?;
        Ok((file, file_size))
    }
}

/// Exercises the bounded object-stream parser independently of container checksums.
/// This entry point is intended for the repository's dependency-free fuzz harness.
pub fn parse_section_object_stream(
    bytes: &[u8],
    limits: LoaderLimits,
) -> Result<Vec<ParsedObject>, EmuError> {
    if bytes.len() > limits.decompressed_bytes {
        return Err(loader_error(
            "resource-limit",
            "M3G section object stream exceeds configured byte budget",
        ));
    }
    parse_objects(bytes, 0, 0, limits)
}

fn validate_container(
    header: FileHeader,
    sections: Vec<Section>,
    objects: Vec<ParsedObject>,
) -> Result<M3gFile, EmuError> {
    if sections.len() < 2 {
        return Err(loader_error(
            "missing-content-section",
            "M3G file must contain a header and at least one content section",
        ));
    }
    if objects
        .iter()
        .skip(1)
        .any(|object| object.object_type == ObjectType::Header)
    {
        return Err(loader_error(
            "duplicate-header",
            "M3G file contains more than one header object",
        ));
    }
    let external_count = sections[1]
        .objects
        .iter()
        .filter(|object| object.object_type == ObjectType::ExternalReference)
        .count();
    let external_in_second = external_count != 0;
    if header.has_external_references != external_in_second
        || external_in_second && external_count != sections[1].objects.len()
    {
        return Err(loader_error(
            "external-section-mismatch",
            "header external-reference flag does not match section 1",
        ));
    }
    if sections.iter().skip(2).any(|section| {
        section
            .objects
            .iter()
            .any(|object| object.object_type == ObjectType::ExternalReference)
    }) {
        return Err(loader_error(
            "external-section-order",
            "external references are only allowed immediately after the header",
        ));
    }
    if !header.has_external_references && objects.len() == 1 {
        return Err(loader_error(
            "empty-file",
            "M3G file contains no scene objects",
        ));
    }
    validate_object_payloads(&objects)?;
    Ok(M3gFile {
        header,
        sections,
        objects,
    })
}

fn object_error(object: &ParsedObject, code: &'static str, message: &'static str) -> EmuError {
    loader_error(
        code,
        format!(
            "section {} object {} type {:?} offset {}: {message}",
            object.section_offset,
            object.index,
            object.object_type,
            object.data.len()
        ),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, EmuError> {
    let value = bytes
        .get(offset..)
        .and_then(|tail| tail.first_chunk::<4>())
        .ok_or_else(|| at_error("truncated-field", offset, "cannot read UInt32"))?;
    Ok(u32::from_le_bytes(*value))
}

/// JSR-184 file-format Float32 permits normal values and positive zero only.
/// Keep this binary constraint separate from floats passed through the Java API.
pub(super) fn decode_f32(bits: u32) -> Result<f32, EmuError> {
    let value = f32::from_bits(bits);
    if bits == 0 || value.is_normal() {
        Ok(value)
    } else if !value.is_finite() {
        Err(loader_error(
            "non-finite",
            "serialized float is NaN or infinity",
        ))
    } else {
        Err(loader_error(
            "invalid-float",
            "serialized float is negative zero or subnormal",
        ))
    }
}

fn at_error(code: &'static str, offset: usize, message: &'static str) -> EmuError {
    loader_error(code, format!("at byte offset {offset}: {message}"))
}

fn loader_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::M3g, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/m3g/loader/mod.rs"]
mod tests;
