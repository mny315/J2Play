//! Worker-owned checkpoint decisions and host runtime persistence.

use super::{
    Cell, EmuError, FrontendNativeContext, HostDecision, HostRequestKind, Instant, LibraryEntry,
    LibraryRepository, Rc, RequestBroker, SessionEvent, SessionEventKind,
};
use crate::library::checkpoint::{
    Checkpoint, CheckpointIdentity, RuntimeStorage, SavedFrame, capture_storage,
};

pub(super) enum LaunchCheckpoint {
    Fresh,
    Resume(Box<Checkpoint>),
    Cancel,
}

pub(super) fn choose_checkpoint(
    repository: &LibraryRepository,
    entry: &LibraryEntry,
    identity: &CheckpointIdentity,
    broker: &mut RequestBroker,
) -> Result<LaunchCheckpoint, EmuError> {
    if entry.settings().resume_behavior == crate::ResumeBehavior::Never {
        return Ok(LaunchCheckpoint::Fresh);
    }
    let (mut saved, problem) = match repository.load_checkpoint(entry, identity) {
        Ok(None) => return Ok(LaunchCheckpoint::Fresh),
        Ok(Some(saved)) => (Some(saved), None),
        Err(error) => (None, Some(error.message().to_owned())),
    };
    if entry.settings().resume_behavior == crate::ResumeBehavior::Always
        && let Some(saved) = saved.take()
    {
        return Ok(LaunchCheckpoint::Resume(Box::new(saved)));
    }
    let decision = broker.decide(HostRequestKind::ResumeGame {
        title: entry.title().to_owned(),
        can_resume: saved.is_some(),
        detail: problem.unwrap_or_else(|| {
            "An automatic save is available. Continue from where you left off?".to_owned()
        }),
    })?;
    Ok(match decision {
        HostDecision::ResumeGame {
            continue_game: true,
            ..
        } => saved.map_or(LaunchCheckpoint::Cancel, |saved| {
            LaunchCheckpoint::Resume(Box::new(saved))
        }),
        HostDecision::ResumeGame {
            continue_game: false,
            ..
        } => LaunchCheckpoint::Fresh,
        _ => LaunchCheckpoint::Cancel,
    })
}

pub(super) struct ContextCheckpoints {
    pub(super) repository: LibraryRepository,
    pub(super) entry: LibraryEntry,
    pub(super) identity: CheckpointIdentity,
    pub(super) epoch: u64,
    pub(super) storage: RuntimeStorage,
    pub(super) host_close: Rc<Cell<bool>>,
    pub(super) restored: bool,
    pub(super) monotonic_base: i64,
    pub(super) wall_offset: i64,
    pub(super) last_frame: Option<SavedFrame>,
}

impl FrontendNativeContext {
    pub(super) fn write_automatic_checkpoint(
        &mut self,
        vm: &[u8],
        driver: &[u8],
        clock: (i64, i64),
    ) -> Result<(), EmuError> {
        let started = Instant::now();
        let cancelled = || {
            started.elapsed() >= std::time::Duration::from_millis(750)
                || self.urgent.force_cancelled()
                || self.urgent.stop_cancellation_due(self.attempt_id)
        };
        let checkpoint = Checkpoint {
            identity: self.checkpoints.identity.clone(),
            saved_at_millis: system_millis(),
            monotonic_millis: clock.0,
            wall_clock_millis: clock.1,
            active_canvas: self.active_canvas_dimensions.get(),
            frame: self.checkpoints.last_frame.clone(),
            vm,
            driver,
            ams: save_state::encode(&*self.ams.borrow())?,
            rms: self.rms_runtime.encode_checkpoint_cancellable(&cancelled)?,
            mmapi: self
                .mmapi_runtime
                .encode_checkpoint_cancellable(&cancelled)?,
            files: capture_storage(&self.checkpoints.storage.files, cancelled)?,
        };
        self.checkpoints.repository.write_checkpoint(
            &self.checkpoints.entry,
            self.checkpoints.epoch,
            &checkpoint,
            &cancelled,
        )
    }

    pub(super) fn finish_checkpoint_restore(&mut self) -> Result<(), EmuError> {
        if super::attempt::attempt_stop_requested(&self.urgent, self.attempt_id) {
            return Err(super::runtime_error(
                "execution-cancelled",
                "Restoring the game was cancelled",
            ));
        }
        if self.checkpoints.restored {
            self.checkpoints
                .repository
                .publish_restored_storage(&self.checkpoints.entry, &self.checkpoints.storage)?;
            self.checkpoints.restored = false;
            self.checkpoints.wall_offset = self.checkpoints.wall_offset.saturating_sub(
                i64::try_from(self.started.elapsed().as_millis()).unwrap_or(i64::MAX),
            );
            self.started = Instant::now();
            if let Some(frame) = self.checkpoints.last_frame.clone() {
                let [x, y, width, height] = frame.canvas_region;
                self.present_physical_frame(
                    &frame.pixels,
                    platform::LogicalRect {
                        x,
                        y,
                        width,
                        height,
                    },
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn report_checkpoint_failure(&self, error: &EmuError) {
        let _ = self.events.publish(SessionEvent {
            session_id: self.session_id,
            attempt_id: self.attempt_id,
            kind: SessionEventKind::CheckpointSaveFailed {
                detail: diagnostics::bounded_text(error.message(), 1024),
            },
        });
    }
}

pub(super) fn system_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}
