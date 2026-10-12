//! Suite-owned file paths, quotas and filesystem operations.

use super::Limits;
use crate::{api_error, api_source, platform_source};
use diagnostics::EmuError;
use natives::GcfFileMetadata;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct FileSandbox {
    root: PathBuf,
    limits: Limits,
    revision: AtomicU64,
}

impl FileSandbox {
    /// Creates and canonicalizes the suite-owned root.
    ///
    /// # Errors
    /// Returns a platform error if the root cannot be created or canonicalized.
    pub fn new(root: &Path, limits: Limits) -> Result<Self, EmuError> {
        fs::create_dir_all(root).map_err(|error| {
            platform_source("file-root", "cannot create suite file root", error)
        })?;
        let root = root.canonicalize().map_err(|error| {
            platform_source("file-root", "cannot canonicalize suite file root", error)
        })?;
        Ok(Self {
            root,
            limits,
            revision: AtomicU64::new(0),
        })
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }

    fn invalidate_readers(&self) {
        // One suite-wide counter avoids an unbounded per-path cache. Invalidate
        // attempts too: a failed host write can have partially changed a file.
        self.revision.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn resolve(&self, url: &str) -> Result<PathBuf, EmuError> {
        if url.len() > self.limits.max_url_bytes {
            return Err(api_error("file-url", "file URL exceeds configured limit"));
        }
        let encoded = url
            .strip_prefix("file://")
            .ok_or_else(|| api_error("file-url", "absolute file URL required"))?;
        if !encoded.starts_with('/') || encoded.starts_with("//") {
            return Err(api_error("file-url", "file URL authority is not allowed"));
        }
        let decoded = percent_decode(encoded)?;
        let mut result = self.root.clone();
        for component in Path::new(decoded.trim_start_matches('/')).components() {
            match component {
                Component::Normal(value) if value != "." && value != ".." => result.push(value),
                _ => {
                    return Err(api_error(
                        "file-traversal",
                        "file URL contains a forbidden component",
                    ));
                }
            }
            if let Ok(metadata) = fs::symlink_metadata(&result)
                && metadata.file_type().is_symlink()
            {
                return Err(api_error(
                    "file-symlink",
                    "symbolic links are forbidden in suite storage",
                ));
            }
        }
        if result == self.root {
            return Ok(result);
        }
        if !result.starts_with(&self.root) {
            return Err(api_error(
                "file-traversal",
                "file URL escapes suite storage",
            ));
        }
        Ok(result)
    }

    pub(crate) fn metadata(&self, url: &str) -> Result<GcfFileMetadata, EmuError> {
        let path = self.resolve(url)?;
        match fs::metadata(path) {
            Ok(value) => Ok(GcfFileMetadata {
                exists: true,
                directory: value.is_dir(),
                size: value.len(),
                modified_millis: value
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |value| {
                        i64::try_from(value.as_millis()).unwrap_or(i64::MAX)
                    }),
                readable: true,
                writable: !value.permissions().readonly(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(GcfFileMetadata::default())
            }
            Err(error) => Err(platform_source(
                "file-metadata",
                "cannot read file metadata",
                error,
            )),
        }
    }

    pub(crate) fn read(&self, url: &str) -> Result<Vec<u8>, EmuError> {
        let path = self.resolve(url)?;
        let (file, length) = open_regular_file(&path, OpenOptions::new().read(true))?;
        if length > self.limits.max_file_bytes {
            return Err(api_error("file-limit", "file exceeds configured limit"));
        }
        let mut data = Vec::new();
        file.take(self.limits.max_file_bytes.saturating_add(1))
            .read_to_end(&mut data)
            .map_err(|error| api_source("file-io", "cannot read file", error))?;
        if u64::try_from(data.len()).unwrap_or(u64::MAX) > self.limits.max_file_bytes {
            return Err(api_error("file-limit", "file exceeds configured limit"));
        }
        Ok(data)
    }

    pub(crate) fn write(&self, url: &str, data: &[u8], append: bool) -> Result<(), EmuError> {
        self.invalidate_readers();
        let path = self.resolve(url)?;
        self.validate_parent(&path)?;
        let (mut file, old_size) =
            open_regular_file(&path, OpenOptions::new().write(true).append(append))?;
        let existing = if append { old_size } else { 0 };
        let new_size = existing.saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
        self.validate_growth(old_size, new_size)?;
        if !append {
            file.set_len(0)
                .map_err(|error| api_source("file-io", "cannot replace file contents", error))?;
        }
        file.write_all(data)
            .and_then(|()| file.sync_all())
            .map_err(|error| api_source("file-io", "cannot commit file", error))
    }

    pub(crate) fn write_at(&self, url: &str, data: &[u8], offset: u64) -> Result<(), EmuError> {
        self.invalidate_readers();
        let path = self.resolve(url)?;
        self.validate_parent(&path)?;
        let (mut file, old_size) = open_regular_file(&path, OpenOptions::new().write(true))?;
        if offset > old_size {
            return Err(api_error("file-io", "output offset exceeds file size"));
        }
        let end = offset
            .checked_add(u64::try_from(data.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| api_error("file-limit", "output offset overflow"))?;
        self.validate_growth(old_size, old_size.max(end))?;
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.write_all(data))
            .and_then(|()| file.sync_all())
            .map_err(|error| api_source("file-io", "cannot commit positional output", error))
    }

    fn validate_growth(&self, old_size: u64, new_size: u64) -> Result<(), EmuError> {
        if new_size > self.limits.max_file_bytes {
            return Err(api_error("file-limit", "file exceeds configured limit"));
        }
        let (suite_size, _) = directory_usage(&self.root, self.limits.max_directory_entries, true)?;
        if suite_size.saturating_sub(old_size).saturating_add(new_size)
            > self.limits.max_suite_bytes
        {
            return Err(api_error("file-limit", "suite file quota exceeded"));
        }
        Ok(())
    }

    pub(crate) fn output_offset(&self, url: &str, offset: u64) -> Result<u64, EmuError> {
        let path = self.resolve(url)?;
        let (_, length) = open_regular_file(&path, OpenOptions::new().write(true))?;
        Ok(offset.min(length))
    }

    pub(crate) fn create(&self, url: &str) -> Result<(), EmuError> {
        self.invalidate_readers();
        let path = self.resolve(url)?;
        self.validate_parent(&path)?;
        self.require_entry_capacity()?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .and_then(|file| file.sync_all())
            .map_err(|error| api_source("file-io", "cannot create file", error))
    }

    pub(crate) fn mkdir(&self, url: &str) -> Result<(), EmuError> {
        self.invalidate_readers();
        let path = self.resolve(url)?;
        self.validate_parent(&path)?;
        self.require_entry_capacity()?;
        fs::create_dir(path)
            .map_err(|error| api_source("file-io", "cannot create directory", error))
    }

    pub(crate) fn delete(&self, url: &str) -> Result<(), EmuError> {
        self.invalidate_readers();
        let path = self.resolve(url)?;
        if path == self.root {
            return Err(api_error("file-traversal", "suite root cannot be deleted"));
        }
        let metadata = fs::metadata(&path)
            .map_err(|error| api_source("file-io", "cannot inspect delete target", error))?;
        if metadata.is_dir() {
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        }
        .map_err(|error| api_source("file-io", "cannot delete path", error))
    }

    pub(crate) fn rename(&self, url: &str, name: &str) -> Result<String, EmuError> {
        self.invalidate_readers();
        if name.is_empty() || name.contains(['/', '\\']) || matches!(name, "." | "..") {
            return Err(api_error(
                "file-traversal",
                "rename target must be one file name",
            ));
        }
        if name.len() > self.limits.max_url_bytes {
            return Err(api_error(
                "file-url",
                "rename target exceeds configured limit",
            ));
        }
        let path = self.resolve(url)?;
        let directory = path.is_dir();
        let source = url.trim_end_matches('/');
        let prefix = source
            .rfind('/')
            .map_or("file:///", |index| &source[..=index]);
        let target_url = format!(
            "{prefix}{}{}",
            encode_path_component(name),
            if directory { "/" } else { "" }
        );
        if target_url.len() > self.limits.max_url_bytes {
            return Err(api_error(
                "file-url",
                "renamed file URL exceeds configured limit",
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| api_error("file-traversal", "path has no parent"))?;
        let target = parent.join(name);
        self.validate_parent(&target)?;
        match fs::symlink_metadata(&target) {
            Ok(_) => return Err(api_error("file-io", "rename target already exists")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(api_source("file-io", "cannot inspect rename target", error));
            }
        }
        fs::rename(&path, &target)
            .map_err(|error| api_source("file-io", "cannot rename path", error))?;
        Ok(target_url)
    }

    pub(crate) fn truncate(&self, url: &str, size: u64) -> Result<(), EmuError> {
        self.invalidate_readers();
        let path = self.resolve(url)?;
        let metadata = fs::metadata(&path)
            .map_err(|error| api_source("file-io", "cannot inspect truncate target", error))?;
        if !metadata.is_file() {
            return Err(api_error("file-io", "truncate target is not a file"));
        }
        // JSR-75 truncation only releases storage. Offsets beyond EOF leave
        // the file unchanged, including offsets above the current quota.
        if size >= metadata.len() {
            return Ok(());
        }
        open_regular_file(&path, OpenOptions::new().write(true))?
            .0
            .set_len(size)
            .map_err(|error| api_source("file-io", "cannot truncate file", error))
    }

    pub(crate) fn list(&self, url: &str) -> Result<Vec<String>, EmuError> {
        let path = self.resolve(url)?;
        let mut names = Vec::new();
        let mut entries = 0usize;
        for entry in fs::read_dir(path)
            .map_err(|error| api_source("file-io", "cannot list directory", error))?
        {
            entries = entries.saturating_add(1);
            if entries > self.limits.max_directory_entries {
                return Err(api_error("file-limit", "directory entry limit exceeded"));
            }
            let entry = entry
                .map_err(|error| api_source("file-io", "cannot read directory entry", error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| api_source("file-io", "cannot inspect directory entry", error))?;
            if file_type.is_symlink() {
                continue;
            }
            let mut name = entry.file_name().to_string_lossy().into_owned();
            if file_type.is_dir() {
                name.push('/');
            }
            names.push(name);
        }
        names.sort();
        Ok(names)
    }

    pub(crate) fn directory_size(&self, url: &str, recursive: bool) -> Result<u64, EmuError> {
        let root = self.resolve(url)?;
        if !root.is_dir() {
            return Err(api_error(
                "file-io",
                "directorySize target is not a directory",
            ));
        }
        directory_usage(&root, self.limits.max_directory_entries, recursive).map(|(size, _)| size)
    }

    pub(crate) fn space(&self, selector: i32) -> Result<u64, EmuError> {
        match selector {
            0 => Ok(self.limits.max_suite_bytes),
            1 | 2 => {
                let (used, _) =
                    directory_usage(&self.root, self.limits.max_directory_entries, true)?;
                Ok(if selector == 1 {
                    self.limits.max_suite_bytes.saturating_sub(used)
                } else {
                    used
                })
            }
            _ => Err(api_error("illegal-argument", "unknown file space selector")),
        }
    }

    pub(crate) fn validate_parent(&self, path: &Path) -> Result<(), EmuError> {
        let parent = path
            .parent()
            .ok_or_else(|| api_error("file-traversal", "path has no parent"))?;
        if !parent.starts_with(&self.root) || !parent.is_dir() {
            return Err(api_error("file-io", "parent directory does not exist"));
        }
        let canonical = parent
            .canonicalize()
            .map_err(|error| api_source("file-io", "cannot validate parent directory", error))?;
        if !canonical.starts_with(&self.root) {
            return Err(api_error("file-symlink", "parent escapes suite storage"));
        }
        Ok(())
    }

    pub(crate) fn require_entry_capacity(&self) -> Result<(), EmuError> {
        let (_, entries) = directory_usage(&self.root, self.limits.max_directory_entries, true)?;
        if entries >= self.limits.max_directory_entries {
            return Err(api_error("file-limit", "suite entry limit exceeded"));
        }
        Ok(())
    }
}

fn open_regular_file(path: &Path, options: &mut OpenOptions) -> Result<(File, u64), EmuError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Corrupt suite storage must not block the VM on a FIFO or redirect a
        // final path component through a link between validation and opening.
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|error| api_source("file-io", "cannot open file", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| api_source("file-io", "cannot inspect open file", error))?;
    if !metadata.is_file() {
        return Err(api_error("file-io", "target is not a regular file"));
    }
    Ok((file, metadata.len()))
}

fn percent_decode(value: &str) -> Result<String, EmuError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(api_error("file-url", "truncated percent escape"));
            }
            let high = hex(bytes[index + 1])?;
            let low = hex(bytes[index + 2])?;
            let byte = high * 16 + low;
            if byte == b'/' || byte == b'\\' || byte == 0 {
                return Err(api_error(
                    "file-traversal",
                    "encoded separator is forbidden",
                ));
            }
            decoded.push(byte);
            index += 3;
        } else {
            if bytes[index] == 0 || bytes[index] == b'\\' {
                return Err(api_error("file-traversal", "invalid file path byte"));
            }
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| api_error("file-url", "file URL is not UTF-8"))
}

fn hex(value: u8) -> Result<u8, EmuError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(api_error("file-url", "invalid percent escape")),
    }
}

fn encode_path_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

fn directory_usage(
    path: &Path,
    max_entries: usize,
    recursive: bool,
) -> Result<(u64, usize), EmuError> {
    let mut pending = vec![path.to_owned()];
    let mut total = 0u64;
    let mut entries = 0usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)
            .map_err(|error| api_source("file-io", "cannot measure directory", error))?
        {
            let entry = entry
                .map_err(|error| api_source("file-io", "cannot measure directory entry", error))?;
            entries = entries.saturating_add(1);
            if entries > max_entries {
                return Err(api_error("file-limit", "directory entry limit exceeded"));
            }
            let kind = entry
                .file_type()
                .map_err(|error| api_source("file-io", "cannot inspect directory entry", error))?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if recursive {
                    pending.push(entry.path());
                }
            } else {
                total = total.saturating_add(
                    entry
                        .metadata()
                        .map_err(|error| api_source("file-io", "cannot inspect file", error))?
                        .len(),
                );
            }
        }
    }
    Ok((total, entries))
}
