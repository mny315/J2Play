//! Bounded snapshots of host files and decompressed ZIP entries.

use diagnostics::{Category, EmuError};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub(super) fn read_regular_file_bounded(
    path: &Path,
    limit: u64,
    open_code: &'static str,
    read_code: &'static str,
    too_large_code: &'static str,
    label: &'static str,
) -> Result<Vec<u8>, EmuError> {
    let mut file = open_snapshot_source(path).map_err(|error| {
        EmuError::with_source(
            Category::Jar,
            open_code,
            format!("cannot open {}", path.display()),
            error,
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        EmuError::with_source(
            Category::Jar,
            open_code,
            format!("cannot inspect open file {}", path.display()),
            error,
        )
    })?;
    if !metadata.is_file() {
        return Err(EmuError::new(
            Category::Jar,
            "not-regular-file",
            format!("{label} path is not a regular file: {}", path.display()),
        ));
    }
    let declared = metadata.len();
    read_open_file_bounded(&mut file, declared, limit, read_code, too_large_code, label)
}

#[cfg(unix)]
fn open_snapshot_source(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_snapshot_source(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

fn read_open_file_bounded(
    file: &mut File,
    declared: u64,
    limit: u64,
    read_code: &'static str,
    too_large_code: &'static str,
    label: &'static str,
) -> Result<Vec<u8>, EmuError> {
    if declared > limit {
        return Err(EmuError::new(
            Category::Jar,
            too_large_code,
            format!("{label} is {declared} bytes; limit is {limit}"),
        ));
    }
    let capacity = usize::try_from(declared).map_err(|_| {
        EmuError::new(
            Category::Jar,
            "file-allocation",
            format!("cannot represent {declared}-byte {label} on this platform"),
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|_| {
        EmuError::new(
            Category::Jar,
            "file-allocation",
            format!("cannot reserve {declared} bytes for {label}"),
        )
    })?;
    let actual = read_declared_payload(file, declared, &mut bytes).map_err(|error| {
        EmuError::with_source(
            Category::Jar,
            read_code,
            format!("cannot read {label}"),
            error,
        )
    })?;
    if actual > limit {
        return Err(EmuError::new(
            Category::Jar,
            too_large_code,
            format!("{label} exceeds {limit} bytes while being read"),
        ));
    }
    if actual != declared {
        return Err(EmuError::new(
            Category::Jar,
            "file-size-changed",
            format!("{label} changed size while being read; expected {declared} bytes"),
        ));
    }
    Ok(bytes)
}

pub(super) fn read_bounded_entry(
    entry: &mut impl Read,
    declared: u64,
    limit: u64,
    operation: &'static str,
) -> Result<Vec<u8>, EmuError> {
    if declared > limit {
        return Err(EmuError::new(
            Category::Jar,
            "entry-too-large",
            format!("entry declares {declared} bytes; limit is {limit}"),
        ));
    }
    let capacity = usize::try_from(declared).unwrap_or(usize::MAX);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|_| {
        EmuError::new(
            Category::Jar,
            "entry-allocation",
            format!("cannot reserve {declared} bytes for an archive entry"),
        )
    })?;
    let actual = read_declared_payload(entry, declared, &mut bytes).map_err(io_error(operation))?;
    if actual > limit {
        return Err(EmuError::new(
            Category::Jar,
            "entry-too-large",
            format!("expanded entry exceeds {limit} bytes"),
        ));
    }
    if actual != declared {
        return Err(EmuError::new(
            Category::Jar,
            "entry-size-mismatch",
            if actual > declared {
                format!("entry expands beyond its declared {declared} bytes")
            } else {
                format!("entry declares {declared} bytes but expands to {actual}")
            },
        ));
    }
    Ok(bytes)
}

fn read_declared_payload(
    reader: &mut impl Read,
    declared: u64,
    bytes: &mut Vec<u8>,
) -> std::io::Result<u64> {
    // Keep the extra-byte probe outside the payload buffer: a growing file or
    // lying ZIP entry must not grow that allocation just before rejection.
    reader.by_ref().take(declared).read_to_end(bytes)?;
    let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual < declared {
        return Ok(actual);
    }
    let mut extra = [0];
    let read = loop {
        match reader.read(&mut extra) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            result => break result?,
        }
    };
    Ok(actual.saturating_add(read as u64))
}

fn io_error(operation: &'static str) -> impl FnOnce(std::io::Error) -> EmuError {
    move |error| EmuError::with_source(Category::Jar, "io", operation, error)
}

#[cfg(test)]
#[path = "../../../tests/unit/jar/bounded_io.rs"]
mod tests;
