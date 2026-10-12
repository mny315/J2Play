use super::{io_error, library_error};
use diagnostics::EmuError;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) fn open_bounded_file(
    path: &Path,
    maximum: u64,
    code: &'static str,
) -> Result<(fs::File, u64), EmuError> {
    let mut options = OpenOptions::new();
    options.read(true);
    // Inspect the opened object without blocking on a corrupt FIFO record.
    // This also closes the metadata-before-open race; regular files ignore it.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|error| io_error(code, "cannot open app-private library data", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error(code, "cannot inspect app-private library data", error))?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(library_error(
            code,
            format!("app-private library data exceeds {maximum} bytes or is not a regular file"),
        ));
    }
    Ok((file, metadata.len()))
}

pub(super) fn read_bounded_file(
    path: &Path,
    maximum: u64,
    code: &'static str,
) -> Result<Vec<u8>, EmuError> {
    read_bounded_file_cancellable(path, maximum, code, || false)
}

pub(super) fn read_bounded_file_cancellable(
    path: &Path,
    maximum: u64,
    code: &'static str,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Vec<u8>, EmuError> {
    let mut check = || {
        if cancelled() {
            Err(library_error(
                code,
                "Reading app-private library data was interrupted",
            ))
        } else {
            Ok(())
        }
    };
    check()?;
    let (mut file, length) = open_bounded_file(path, maximum, code)?;
    let allocation_error = || library_error(code, "Cannot allocate app-private library data");
    let capacity = usize::try_from(length).map_err(|_| allocation_error())?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_error())?;
    let changed = || {
        library_error(
            code,
            "App-private library data changed size while being read",
        )
    };
    while bytes.len() < capacity {
        check()?;
        let remaining = (capacity - bytes.len()).min(64 * 1024);
        let read = (&mut file)
            .take(remaining as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error(code, "cannot read app-private library data", error))?;
        if read == 0 {
            return Err(changed());
        }
    }
    check()?;
    // Probe outside the payload buffer, so a growing file cannot double its
    // allocation immediately before rejection.
    match file.read_exact(&mut [0]) {
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(bytes),
        Ok(()) => Err(changed()),
        Err(error) => Err(io_error(
            code,
            "cannot read app-private library data",
            error,
        )),
    }
}

pub(super) fn remove_optional_file(
    path: &Path,
    code: &'static str,
    message: &'static str,
) -> Result<(), EmuError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() || metadata.file_type().is_symlink() => {
            fs::remove_file(path).map_err(|error| io_error(code, message, error))?;
            sync_parent_after_delete(path, code, message)
        }
        Ok(_) => Err(library_error(
            code,
            "app-private deletion target is not a regular file",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(code, message, error)),
    }
}

pub(super) fn remove_optional_tree(
    path: &Path,
    code: &'static str,
    message: &'static str,
) -> Result<(), EmuError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.file_type().is_file() => {
            fs::remove_file(path).map_err(|error| io_error(code, message, error))?;
            sync_parent_after_delete(path, code, message)
        }
        Ok(metadata) if metadata.file_type().is_dir() => {
            fs::remove_dir_all(path).map_err(|error| io_error(code, message, error))?;
            sync_parent_after_delete(path, code, message)
        }
        Ok(_) => Err(library_error(
            code,
            "app-private deletion target has an unsupported file type",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(code, message, error)),
    }
}

fn sync_parent_after_delete(
    path: &Path,
    code: &'static str,
    message: &'static str,
) -> Result<(), EmuError> {
    let parent = path
        .parent()
        .ok_or_else(|| library_error(code, "app-private deletion target has no parent"))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error(code, message, error))
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), EmuError> {
    atomic_write_parts(path, &[bytes])
}

pub(super) fn atomic_write_parts(path: &Path, parts: &[&[u8]]) -> Result<(), EmuError> {
    let parent = path.parent().ok_or_else(|| {
        library_error(
            "library-atomic-path",
            "app-private destination has no parent directory",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        io_error(
            "library-atomic-directory",
            "cannot create app-private destination directory",
            error,
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            library_error(
                "library-atomic-path",
                "app-private destination filename is invalid Unicode",
            )
        })?;
    let (temporary, mut file) = (0..128)
        .find_map(|_| {
            let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temporary = parent.join(format!(
                ".{file_name}.{}.{}.tmp",
                std::process::id(),
                sequence
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => Some(Ok((temporary, file))),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .transpose()
        .map_err(|error| {
            io_error(
                "library-atomic-create",
                "cannot create app-private temporary file",
                error,
            )
        })?
        .ok_or_else(|| {
            library_error(
                "library-atomic-create",
                "cannot allocate a unique app-private temporary filename",
            )
        })?;
    let write_result = (|| {
        for bytes in parts {
            file.write_all(bytes).map_err(|error| {
                io_error(
                    "library-atomic-write",
                    "cannot write app-private temporary file",
                    error,
                )
            })?;
        }
        file.sync_all().map_err(|error| {
            io_error(
                "library-atomic-sync",
                "cannot sync app-private temporary file",
                error,
            )
        })?;
        drop(file);
        fs::rename(&temporary, path).map_err(|error| {
            io_error(
                "library-atomic-rename",
                "cannot publish app-private file",
                error,
            )
        })?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                io_error(
                    "library-atomic-directory-sync",
                    "cannot sync app-private destination directory",
                    error,
                )
            })
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    write_result
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-core/library/storage.rs"]
mod tests;
