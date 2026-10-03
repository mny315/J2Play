mod directory;

use super::{Pending, document, request};
use crate::{lifecycle::Lifecycle, operation::error};
use diagnostics::EmuError;
use eframe::egui;
use frontend_ui::{
    DocumentBrowser, DocumentBrowserEntry, DocumentKind, DocumentOutcome, PickedDocument,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_ENTRY: AtomicU64 = AtomicU64::new(1);

enum ResultData {
    Directory(directory::Directory),
    File(PickedDocument),
}

/// One bounded operation at a time. The UI only receives labels and ephemeral
/// IDs; the shell owns paths, reads, cancellation and the worker's join handle.
pub(super) struct Browser {
    view: DocumentBrowser,
    directory: PathBuf,
    shortcuts: Vec<(String, PathBuf)>,
    targets: Vec<(u64, PathBuf, bool)>,
    pending: Option<Pending<ResultData>>,
    cancelled: bool,
    ctx: egui::Context,
    lifecycle: Lifecycle,
}

impl Browser {
    pub(super) fn start(
        kind: DocumentKind,
        directory: PathBuf,
        ctx: egui::Context,
        lifecycle: Lifecycle,
    ) -> Result<Self, EmuError> {
        let home = home_directory();
        let shortcuts = vec![
            ("Home".into(), home.clone()),
            ("Downloads".into(), home.join("Downloads")),
            ("Removable drives".into(), PathBuf::from("/run/media")),
            ("Media".into(), PathBuf::from("/media")),
            ("File system".into(), PathBuf::from("/")),
        ];
        let mut browser = Self {
            view: DocumentBrowser {
                kind,
                folder: directory.to_string_lossy().chars().take(512).collect(),
                revision: 0,
                parent: None,
                shortcuts: Vec::new(),
                entries: Vec::new(),
                busy: false,
                truncated: false,
                error: None,
            },
            directory,
            shortcuts,
            targets: Vec::new(),
            pending: None,
            cancelled: false,
            ctx,
            lifecycle,
        };
        browser.publish(directory::Directory::empty(browser.directory.clone()))?;
        browser.open(browser.directory.clone(), true)?;
        Ok(browser)
    }

    pub(super) fn view(&self) -> Option<&DocumentBrowser> {
        (!self.cancelled).then_some(&self.view)
    }

    pub(super) fn current_directory(&self) -> &Path {
        &self.directory
    }

    pub(super) fn activate(&mut self, id: u64) -> Result<(), EmuError> {
        if self.cancelled || self.pending.is_some() {
            return Err(error(
                "document-picker-busy",
                "File selection is still busy.",
            ));
        }
        let (_, path, directory) = self
            .targets
            .iter()
            .find(|(entry, _, _)| *entry == id)
            .ok_or_else(|| {
                error(
                    "linux-document-selection",
                    "This selection has expired. Select the file again.",
                )
            })?;
        self.open(path.clone(), *directory)
    }

    fn open(&mut self, path: PathBuf, directory: bool) -> Result<(), EmuError> {
        let kind = self.view.kind;
        let lifecycle = self.lifecycle.clone();
        self.pending = Some(Pending::start(&self.ctx, move |cancel| {
            let deadline = Instant::now() + Duration::from_secs(15);
            let check = || request::read_cancelled(&cancel, &lifecycle, deadline);
            if directory {
                directory::read(path, kind, check).map(ResultData::Directory)
            } else {
                document::read_path(&path, kind, check).map(ResultData::File)
            }
        })?);
        self.view.busy = true;
        self.view.error = None;
        Ok(())
    }

    pub(super) fn poll(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        if self.lifecycle.system_suspended() {
            self.cancel();
        }
        let result = Pending::poll_slot(&mut self.pending);
        self.view.busy = self.pending.is_some();
        if self.cancelled {
            return self
                .pending
                .is_none()
                .then_some(Ok(DocumentOutcome::Cancelled {
                    kind: self.view.kind,
                }));
        }
        let result = result?;
        match result {
            Ok(ResultData::File(document)) => return Some(Ok(DocumentOutcome::Selected(document))),
            Ok(ResultData::Directory(directory)) => {
                if let Err(error) = self.publish(directory) {
                    return Some(Err(error));
                }
            }
            Err(error) if error.code() == "linux-document-cancelled" => {
                return Some(Ok(DocumentOutcome::Cancelled {
                    kind: self.view.kind,
                }));
            }
            Err(error) => self.view.error = Some(error.message().to_owned()),
        }
        None
    }

    fn publish(&mut self, directory: directory::Directory) -> Result<(), EmuError> {
        let mut targets = Vec::new();
        let mut entry = |name: String, path: PathBuf, directory: bool| {
            let id = next_id()?;
            targets.push((id, path, directory));
            Ok(DocumentBrowserEntry {
                id,
                name,
                directory,
            })
        };
        let parent = directory
            .path
            .parent()
            .map(|path| entry("Up".into(), path.to_owned(), true).map(|entry| entry.id))
            .transpose()?;
        let shortcuts = self
            .shortcuts
            .iter()
            .map(|(name, path)| entry(name.clone(), path.clone(), true))
            .collect::<Result<_, EmuError>>()?;
        let entries = directory
            .entries
            .into_iter()
            .map(|(name, path, is_directory)| entry(name, path, is_directory))
            .collect::<Result<_, EmuError>>()?;
        self.view = DocumentBrowser {
            kind: self.view.kind,
            folder: directory
                .path
                .to_string_lossy()
                .chars()
                .map(|c| if c.is_control() { '�' } else { c })
                .take(512)
                .collect(),
            revision: next_id()?,
            parent,
            shortcuts,
            entries,
            busy: false,
            truncated: directory.truncated,
            error: None,
        };
        self.targets = targets;
        self.directory = directory.path;
        Ok(())
    }

    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
        if let Some(pending) = &mut self.pending {
            pending.cancel_result();
        }
    }

    pub(super) fn shutdown(&mut self) -> Result<(), EmuError> {
        self.cancel();
        self.pending.as_mut().map_or(Ok(()), Pending::shutdown)
    }
}

fn next_id() -> Result<u64, EmuError> {
    NEXT_ENTRY
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| {
            error(
                "linux-document-id",
                "File selection identifiers were exhausted.",
            )
        })
}

pub(super) fn home_directory() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.as_os_str().len() <= 4096)
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(test)]
#[path = "../../../../tests/unit/j2play-linux/portal/browser.rs"]
mod tests;
