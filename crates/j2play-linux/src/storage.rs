use diagnostics::{Category, EmuError};
use frontend_core::LibraryRepository;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub(crate) const APP_ID: &str = "io.github.mny315.j2play";

#[derive(Debug)]
pub(crate) struct AppPaths {
    pub(crate) data: PathBuf,
    pub(crate) cache: PathBuf,
}

impl AppPaths {
    pub(crate) fn from_environment() -> Result<Self, EmuError> {
        Self::resolve(|name| std::env::var_os(name))
    }

    fn resolve(get: impl Fn(&str) -> Option<OsString>) -> Result<Self, EmuError> {
        // Flatpak supplies XDG roots within ~/.var/app/<app-id>. Respect them
        // directly instead of deriving an unsandboxed host home directory.
        let base = |variable: &str, fallback: &str| {
            get(variable)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .or_else(|| {
                    get("HOME")
                        .map(PathBuf::from)
                        .filter(|path| path.is_absolute())
                        .map(|home| home.join(fallback))
                })
                .map(|path| path.join(APP_ID))
                .ok_or_else(|| {
                    storage_error(
                        "linux-data-environment",
                        "J2Play needs an absolute HOME or XDG data/cache directory. Check your desktop session environment.",
                    )
                })
        };
        Ok(Self {
            data: base("XDG_DATA_HOME", ".local/share")?,
            cache: base("XDG_CACHE_HOME", ".cache")?,
        })
    }

    pub(crate) fn acquire(&self) -> Result<InstanceGuard, EmuError> {
        private_directory(&self.data)?;
        // Lock the persistent data inode, not a PID file or a runtime-dir name:
        // different desktop sessions using the same library must also exclude
        // each other. Never unlink this file; the kernel releases it on exit.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(self.data.join("instance.lock"))
            .map_err(|_| storage_error("linux-instance-lock", "J2Play could not lock its data directory. Check its permissions and available disk space."))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(storage_error(
                    "linux-already-running",
                    "J2Play is already using this library. Return to the existing window or close it before starting again.",
                ));
            }
            Err(TryLockError::Error(_)) => {
                return Err(storage_error(
                    "linux-instance-lock",
                    "J2Play cannot safely lock its library. Use a writable local filesystem with file locking support.",
                ));
            }
        }
        private_directory(&self.cache)?;
        Ok(InstanceGuard { file })
    }

    pub(crate) fn repository(&self, guard: InstanceGuard) -> Result<LibraryRepository, EmuError> {
        LibraryRepository::open_with_cache(
            self.data.join("library"),
            self.cache.join("native-code"),
        )
        .map(|repository| {
            repository
                .with_instance_lock(guard.file)
                .with_default_virtual_controls_visible(false)
        })
    }
}

pub(crate) struct InstanceGuard {
    file: File,
}

fn private_directory(path: &Path) -> Result<(), EmuError> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|_| {
            storage_error(
                "linux-data-directory",
                "J2Play could not create its data or cache directory. Check its permissions and available disk space.",
            )
        })
}

fn storage_error(code: &'static str, message: &'static str) -> EmuError {
    EmuError::new(Category::Platform, code, message)
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/storage.rs"]
mod tests;
