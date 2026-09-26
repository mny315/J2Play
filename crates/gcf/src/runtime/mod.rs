//! Suite permissions, resource limits and the common HTTP/`FileConnection` runtime.

use crate::{FILE_READ_PERMISSION, FILE_WRITE_PERMISSION, api_error};
use diagnostics::EmuError;
use natives::GcfFileMetadata;
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

mod file;
mod http;
mod http_date;
mod http_transport;
mod native_api;

pub use file::FileSandbox;
pub use http::http_request_destination;
pub use http_transport::{
    HttpTransport, Resolver, SystemResolver, TransportRequest, TransportResponse, UreqTransport,
};
pub use native_api::register_natives;

#[cfg(test)]
pub(crate) use http::{ParsedHttpUrl, is_public_address};

#[derive(Clone, Debug)]
pub struct Limits {
    pub max_url_bytes: usize,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_headers: usize,
    pub max_header_bytes: usize,
    pub max_redirects: usize,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub max_file_bytes: u64,
    pub max_suite_bytes: u64,
    pub max_directory_entries: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_url_bytes: 2_048,
            max_request_bytes: 1_048_576,
            max_response_bytes: 4_194_304,
            max_headers: 64,
            max_header_bytes: 32_768,
            max_redirects: 4,
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(20),
            max_file_bytes: 8_388_608,
            max_suite_bytes: 67_108_864,
            max_directory_entries: 4_096,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PermissionPolicy {
    allowed: HashSet<String>,
}

impl PermissionPolicy {
    #[must_use]
    pub fn new(allowed: impl IntoIterator<Item = String>) -> Self {
        Self {
            allowed: allowed.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn allows(&self, permission: &str) -> bool {
        self.allowed.contains(permission)
    }

    fn require(&self, permission: &str) -> Result<(), EmuError> {
        if self.allows(permission) {
            Ok(())
        } else {
            Err(api_error(
                "security-exception",
                format!("permission denied: {permission}"),
            ))
        }
    }
}

pub struct Runtime {
    permissions: PermissionPolicy,
    transport: Box<dyn HttpTransport>,
    resolver: Box<dyn Resolver>,
    files: FileSandbox,
    limits: Limits,
    offline: bool,
}

impl Runtime {
    /// Creates one isolated suite runtime.
    ///
    /// # Errors
    /// Returns an error if the suite file root cannot be created safely.
    pub fn new(
        permissions: PermissionPolicy,
        transport: Box<dyn HttpTransport>,
        resolver: Box<dyn Resolver>,
        file_root: &Path,
        limits: Limits,
        offline: bool,
    ) -> Result<Self, EmuError> {
        Ok(Self {
            permissions,
            transport,
            resolver,
            files: FileSandbox::new(file_root, limits.clone())?,
            limits,
            offline,
        })
    }

    /// Whether the suite policy permits an operation before interactive host approval.
    #[must_use]
    pub fn permission_allowed(&self, permission: &str) -> bool {
        self.permissions.allows(permission)
    }

    /// Applies read permission and returns one sandbox metadata snapshot.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_metadata(&self, url: &str) -> Result<GcfFileMetadata, EmuError> {
        self.permissions.require(FILE_READ_PERMISSION)?;
        self.files.metadata(url)
    }
    /// Reads one bounded suite file after permission validation.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_read(&self, url: &str) -> Result<Vec<u8>, EmuError> {
        self.permissions.require(FILE_READ_PERMISSION)?;
        self.files.read(url)
    }
    /// Returns a constant-space change token for cached suite file reads.
    #[must_use]
    pub fn file_revision(&self) -> u64 {
        self.files.revision()
    }
    /// Writes one bounded suite file after permission validation.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_write(&self, url: &str, data: &[u8], append: bool) -> Result<(), EmuError> {
        self.permissions.require(FILE_WRITE_PERMISSION)?;
        self.files.write(url, data, append)
    }
    /// Writes bytes at an existing file offset without requiring read permission.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_write_at(&self, url: &str, data: &[u8], offset: u64) -> Result<(), EmuError> {
        self.permissions.require(FILE_WRITE_PERMISSION)?;
        self.files.write_at(url, data, offset)
    }
    /// Validates output access and clamps the initial position to EOF.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_output_offset(&self, url: &str, offset: u64) -> Result<u64, EmuError> {
        self.permissions.require(FILE_WRITE_PERMISSION)?;
        self.files.output_offset(url, offset)
    }
    /// Creates a new empty suite file.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_create(&self, url: &str) -> Result<(), EmuError> {
        self.permissions.require(FILE_WRITE_PERMISSION)?;
        self.files.create(url)
    }
    /// Creates one suite directory without creating missing parents.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_mkdir(&self, url: &str) -> Result<(), EmuError> {
        self.permissions.require(FILE_WRITE_PERMISSION)?;
        self.files.mkdir(url)
    }
    /// Deletes one file or empty directory below the suite root.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_delete(&self, url: &str) -> Result<(), EmuError> {
        self.permissions.require(FILE_WRITE_PERMISSION)?;
        self.files.delete(url)
    }
    /// Renames a path within its current directory and returns its new URL.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_rename(&self, url: &str, name: &str) -> Result<String, EmuError> {
        self.permissions.require(FILE_WRITE_PERMISSION)?;
        self.files.rename(url, name)
    }
    /// Truncates one file; offsets at or beyond its end leave it unchanged.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_truncate(&self, url: &str, size: u64) -> Result<(), EmuError> {
        self.permissions.require(FILE_WRITE_PERMISSION)?;
        self.files.truncate(url, size)
    }
    /// Returns a sorted, bounded directory listing without symbolic links.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_list(&self, url: &str) -> Result<Vec<String>, EmuError> {
        self.permissions.require(FILE_READ_PERMISSION)?;
        self.files.list(url)
    }
    /// Computes a bounded directory size without following symbolic links.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_directory_size(&self, url: &str, recursive: bool) -> Result<u64, EmuError> {
        self.permissions.require(FILE_READ_PERMISSION)?;
        self.files.directory_size(url, recursive)
    }
    /// Returns the configured suite quota, remaining quota, or used bytes.
    #[allow(clippy::missing_errors_doc)]
    pub fn file_space(&self, selector: i32) -> Result<u64, EmuError> {
        self.permissions.require(FILE_READ_PERMISSION)?;
        self.files.space(selector)
    }
}
