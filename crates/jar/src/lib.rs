//! Bounded, read-only inspection of `MIDlet` JAR archives.

use std::collections::BTreeMap;

mod archive;
mod bounded_io;
mod inspection;
mod properties;
mod raw_zip;
mod resources;

pub use inspection::{inspect_bytes, inspect_path};
pub use properties::{parse_jad, parse_jad_path};
pub use resources::{
    ResourceArchive, read_class_entries_bytes, read_class_entries_path, read_resource_bytes,
    read_resource_path, read_resource_path_with_limit,
};

const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 16_384;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_JAD_BYTES: u64 = 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JarInfo {
    pub sha256: String,
    pub archive_bytes: u64,
    /// Preferred DOS archive timestamp, in Unix milliseconds.
    ///
    /// The manifest entry wins when present; otherwise this is the newest
    /// usable entry timestamp. Archives without a usable timestamp remain
    /// valid and expose `None`.
    pub archive_timestamp_millis: Option<i64>,
    pub manifest: BTreeMap<String, String>,
    pub manifest_sections: Vec<ManifestSection>,
    pub midlets: Vec<MidletInfo>,
    pub resources: Vec<ResourceInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestSection {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MidletInfo {
    pub index: u32,
    pub name: String,
    pub icon: Option<String>,
    pub class_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceInfo {
    pub name: String,
    pub compressed_bytes: u64,
    pub uncompressed_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JadInfo {
    pub properties: BTreeMap<String, String>,
    pub midlets: Vec<MidletInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassResource {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[cfg(test)]
#[path = "../../../tests/unit/jar/mod.rs"]
mod tests;
