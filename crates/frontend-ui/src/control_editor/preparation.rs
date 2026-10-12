//! One background preview preparation; cancelled jobs retain their join handle.

use crate::egui;
use diagnostics::{Category, EmuError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

type Dimensions = Result<(u32, u32), EmuError>;
type Work = Box<dyn FnOnce(&AtomicBool) -> Dimensions + Send>;

struct Request {
    id: u64,
    work: Work,
}

struct Running {
    id: u64,
    cancelled: Arc<AtomicBool>,
    thread: JoinHandle<Dimensions>,
}

#[derive(Default)]
pub(crate) struct EditorPreparation {
    next_id: u64,
    queued: Option<Request>,
    running: Option<Running>,
}

impl EditorPreparation {
    pub(super) fn request(
        &mut self,
        repository: frontend_core::LibraryRepository,
        entry: frontend_core::LibraryEntry,
        settings: frontend_core::GameSettings,
    ) -> Result<u64, EmuError> {
        self.request_with(Box::new(move |cancelled| {
            repository
                .prepare_launch_cancellable(&entry, || cancelled.load(Ordering::Acquire))?
                .launch_plan(&settings)
                .map(|plan| plan.profile_summary().canvas_dimensions)
        }))
    }

    fn request_with(&mut self, work: Work) -> Result<u64, EmuError> {
        let id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| worker_error("Control editor request IDs are exhausted"))?;
        self.retain(None);
        self.next_id = id;
        self.queued = Some(Request { id, work });
        Ok(id)
    }

    pub(super) fn retain(&mut self, current: Option<u64>) {
        if self
            .queued
            .as_ref()
            .is_some_and(|request| Some(request.id) != current)
        {
            self.queued = None;
        }
        if let Some(running) = &self.running
            && Some(running.id) != current
        {
            running.cancelled.store(true, Ordering::Release);
        }
    }

    pub(super) fn poll(&mut self, ctx: &egui::Context) -> Option<(u64, Dimensions)> {
        let mut ready = None;
        if self
            .running
            .as_ref()
            .is_some_and(|running| running.thread.is_finished())
        {
            let running = self.running.take().expect("finished preparation exists");
            let result = running
                .thread
                .join()
                .unwrap_or_else(|_| Err(worker_error("Control editor preparation panicked")));
            if !running.cancelled.load(Ordering::Acquire) {
                ready = Some((running.id, result));
            }
        }
        if self.running.is_none()
            && let Some(request) = self.queued.take()
        {
            let cancelled = Arc::new(AtomicBool::new(false));
            let worker_cancelled = Arc::clone(&cancelled);
            let repaint = ctx.clone();
            match thread::Builder::new()
                .name("j2play-control-preview".into())
                .spawn(move || {
                    let result = (request.work)(&worker_cancelled);
                    crate::eframe_app::repaint_for_runtime_event(&repaint);
                    result
                }) {
                Ok(thread) => {
                    self.running = Some(Running {
                        id: request.id,
                        cancelled,
                        thread,
                    });
                }
                Err(_) => {
                    ready = Some((
                        request.id,
                        Err(worker_error("Cannot start control editor preparation")),
                    ));
                }
            }
        }
        // The worker requests a repaint before returning. Cover the race where
        // that paint arrives before JoinHandle reports completion.
        if self.running.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        ready
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), EmuError> {
        self.shutdown_before(Instant::now() + Duration::from_secs(3))
    }

    fn shutdown_before(&mut self, deadline: Instant) -> Result<(), EmuError> {
        self.retain(None);
        let Some(running) = &self.running else {
            return Ok(());
        };
        while !running.thread.is_finished() {
            if Instant::now() >= deadline {
                return Err(worker_error(
                    "Control editor preparation did not stop within its shutdown deadline",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
        self.running
            .take()
            .expect("preparation exists")
            .thread
            .join()
            .map(|_| ())
            .map_err(|_| worker_error("Control editor preparation panicked"))
    }
}

impl Drop for EditorPreparation {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("j2play: platform[control-editor-shutdown]: {error}");
        }
    }
}

fn worker_error(message: &str) -> EmuError {
    EmuError::new(Category::Platform, "control-editor-preparation", message)
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-ui/control_editor/preparation.rs"]
mod tests;
