//! One bounded library asset job, independent of the emulator worker.

use super::{decode_icon, egui};
use diagnostics::{Category, EmuError};
use frontend_core::{LibraryEntry, LibraryRepository, RecoveredGameInfo};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Control {
    stopped: AtomicBool,
    revision: AtomicU64,
}

struct Request {
    entry: LibraryEntry,
    edge: u32,
    revision: u64,
    repaint: egui::Context,
}

#[derive(Default)]
struct Assets {
    image: Option<egui::ColorImage>,
    info: Option<RecoveredGameInfo>,
}

pub(super) struct ReadyAssets {
    pub(super) entry_id: String,
    pub(super) image: Option<egui::ColorImage>,
    pub(super) info: Option<RecoveredGameInfo>,
    revision: u64,
}

pub(crate) struct LibraryAssetLoader {
    requests: Option<mpsc::SyncSender<Request>>,
    ready: mpsc::Receiver<ReadyAssets>,
    thread: Option<JoinHandle<()>>,
    control: Arc<Control>,
    busy: bool,
}

impl LibraryAssetLoader {
    pub(crate) fn spawn(repository: LibraryRepository) -> Result<Self, EmuError> {
        Self::spawn_with(move |entry, edge, cancelled| {
            let info = repository.read_game_info(entry, cancelled).ok().flatten();
            let image = if cancelled() {
                None
            } else {
                repository
                    .read_icon_bytes(entry, cancelled)
                    .ok()
                    .flatten()
                    .filter(|_| !cancelled())
                    .and_then(|bytes| decode_icon(&bytes, edge).ok())
            };
            Assets { image, info }
        })
    }

    fn spawn_with(
        mut load: impl FnMut(&LibraryEntry, u32, &dyn Fn() -> bool) -> Assets + Send + 'static,
    ) -> Result<Self, EmuError> {
        let (requests, receiver) = mpsc::sync_channel::<Request>(1);
        let (publish, ready) = mpsc::sync_channel(1);
        let control = Arc::new(Control::default());
        let worker_control = Arc::clone(&control);
        let thread = thread::Builder::new()
            .name("j2play-library-assets".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let cancelled = || {
                        worker_control.stopped.load(Ordering::Acquire)
                            || worker_control.revision.load(Ordering::Acquire) != request.revision
                    };
                    let assets = if cancelled() {
                        Assets::default()
                    } else {
                        load(&request.entry, request.edge, &cancelled)
                    };
                    let assets = if cancelled() {
                        Assets::default()
                    } else {
                        assets
                    };
                    if publish
                        .send(ReadyAssets {
                            entry_id: request.entry.id().to_owned(),
                            image: assets.image,
                            info: assets.info,
                            revision: request.revision,
                        })
                        .is_err()
                    {
                        break;
                    }
                    super::super::eframe_app::repaint_for_runtime_event(&request.repaint);
                }
            })
            .map_err(|_| worker_error("Cannot start the library asset worker"))?;
        Ok(Self {
            requests: Some(requests),
            ready,
            thread: Some(thread),
            control,
            busy: false,
        })
    }

    pub(crate) fn invalidate(&self) {
        self.control.revision.fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn request(
        &mut self,
        entry: &LibraryEntry,
        edge: u32,
        repaint: &egui::Context,
    ) -> Result<(), EmuError> {
        if self.busy {
            return Ok(());
        }
        let Some(requests) = &self.requests else {
            return Ok(());
        };
        let request = Request {
            entry: entry.clone(),
            edge,
            revision: self.control.revision.load(Ordering::Acquire),
            repaint: repaint.clone(),
        };
        match requests.try_send(request) {
            Ok(()) => self.busy = true,
            Err(mpsc::TrySendError::Full(_)) => {}
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.requests = None;
                return Err(worker_error(
                    "The library asset worker stopped unexpectedly",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn take_ready(&mut self) -> Result<Option<ReadyAssets>, EmuError> {
        if !self.busy {
            return Ok(None);
        }
        match self.ready.try_recv() {
            Ok(ready) => {
                self.busy = false;
                Ok(
                    (ready.revision == self.control.revision.load(Ordering::Acquire))
                        .then_some(ready),
                )
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.requests = None;
                self.busy = false;
                Err(worker_error(
                    "The library asset worker stopped unexpectedly",
                ))
            }
        }
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), EmuError> {
        self.shutdown_before(Instant::now() + Duration::from_secs(3))
    }

    fn shutdown_before(&mut self, deadline: Instant) -> Result<(), EmuError> {
        self.control.stopped.store(true, Ordering::Release);
        self.requests = None;
        let Some(thread) = self.thread.as_ref() else {
            return Ok(());
        };
        while !thread.is_finished() {
            if Instant::now() >= deadline {
                return Err(worker_error(
                    "The library asset worker did not stop within its shutdown deadline",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
        self.thread
            .take()
            .expect("worker thread is present")
            .join()
            .map_err(|_| worker_error("The library asset worker panicked"))
    }
}

impl Drop for LibraryAssetLoader {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("j2play: platform[library-assets-shutdown]: {error}");
        }
    }
}

fn worker_error(message: &str) -> EmuError {
    EmuError::new(Category::Platform, "library-assets-worker", message)
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/library_icons/loader.rs"]
mod tests;
