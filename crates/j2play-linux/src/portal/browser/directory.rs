use crate::operation::error;
use diagnostics::EmuError;
use frontend_ui::DocumentKind;
use std::path::PathBuf;

const MAX_ENTRIES: usize = 2048;
const MAX_SCANNED: usize = 16_384;
const MAX_PATH_BYTES: usize = 4096;

pub(super) struct Directory {
    pub(super) path: PathBuf,
    pub(super) entries: Vec<(String, PathBuf, bool)>,
    pub(super) truncated: bool,
}

impl Directory {
    pub(super) fn empty(path: PathBuf) -> Self {
        Self {
            path,
            entries: Vec::new(),
            truncated: false,
        }
    }
}

pub(super) fn read(
    path: PathBuf,
    kind: DocumentKind,
    mut check: impl FnMut() -> Result<(), EmuError>,
) -> Result<Directory, EmuError> {
    check()?;
    if !path.is_absolute() || path.as_os_str().len() > MAX_PATH_BYTES {
        return Err(unreadable());
    }
    let entries = std::fs::read_dir(&path).map_err(|_| unreadable())?;
    let mut result = Directory::empty(path);
    let extension = match kind {
        DocumentKind::Jar => "jar",
        DocumentKind::Jad => "jad",
    };
    for (scanned, entry) in entries.enumerate() {
        check()?;
        if scanned == MAX_SCANNED || result.entries.len() == MAX_ENTRIES {
            result.truncated = true;
            break;
        }
        let entry = entry.map_err(|_| unreadable())?;
        let path = entry.path();
        if path.as_os_str().len() > MAX_PATH_BYTES {
            result.truncated = true;
            continue;
        }
        // Symlinks are navigable, but special files are never offered as imports.
        let Ok(mut file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            let Ok(metadata) = path.metadata() else {
                continue;
            };
            file_type = metadata.file_type();
        }
        let directory = file_type.is_dir();
        if directory
            || (file_type.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(extension)))
        {
            let name = entry
                .file_name()
                .to_string_lossy()
                .chars()
                .map(|c| if c.is_control() { '�' } else { c })
                .take(512)
                .collect();
            result.entries.push((name, path, directory));
        }
    }
    check()?;
    result
        .entries
        .sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    Ok(result)
}

fn unreadable() -> EmuError {
    error(
        "linux-document-directory",
        "This folder cannot be opened. Choose another location.",
    )
}

#[cfg(test)]
#[path = "../../../../../tests/unit/j2play-linux/portal/browser/directory.rs"]
mod tests;
