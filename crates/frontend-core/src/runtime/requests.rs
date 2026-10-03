//! Bounded host decisions, cancellation and session permission caching.

use super::{PlatformLifecycleSignal, runtime_error};
use crate::transport::{CommandQueue, EventMailbox, UrgentState};
use crate::{
    AttemptId, HostDecision, HostRequest, HostRequestKind, SessionEvent, SessionEventKind,
    SessionId,
};
use diagnostics::{Category, EmuError};
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

const REQUEST_POLL: Duration = Duration::from_millis(25);
const HOST_REQUEST_TIMEOUT: Duration = Duration::from_mins(1);
const MAX_SESSION_REQUEST_SCOPES: usize = 64;

pub(super) struct RequestBroker {
    session_id: SessionId,
    attempt_id: AttemptId,
    next_request_id: u64,
    commands: CommandQueue,
    urgent: Arc<UrgentState>,
    events: EventMailbox,
    allowed_for_session: HashSet<String>,
    lifecycle: PlatformLifecycleSignal,
}

impl RequestBroker {
    pub(super) fn new(
        session_id: SessionId,
        attempt_id: AttemptId,
        commands: CommandQueue,
        events: EventMailbox,
        lifecycle: PlatformLifecycleSignal,
    ) -> Self {
        let urgent = Arc::clone(commands.urgent());
        Self {
            session_id,
            attempt_id,
            next_request_id: 0,
            commands,
            urgent,
            events,
            allowed_for_session: HashSet::new(),
            lifecycle,
        }
    }

    pub(super) fn decide(&mut self, kind: HostRequestKind) -> Result<HostDecision, EmuError> {
        // MIDP's empty URL cancels pending external work. It grants no access,
        // consumes no request ID and must also work during pause or teardown.
        if matches!(&kind, HostRequestKind::PlatformRequest { url } if url.is_empty()) {
            self.events.publish(SessionEvent {
                session_id: self.session_id,
                attempt_id: self.attempt_id,
                kind: SessionEventKind::PlatformRequestCancelled,
            })?;
            return Ok(HostDecision::AllowOnce);
        }
        let resume = matches!(kind, HostRequestKind::ResumeGame { .. });
        if self.request_cancelled(resume) || self.pause_pending(resume)? {
            return Ok(HostDecision::Deny);
        }
        let scope = request_scope(&kind);
        if !resume && self.allowed_for_session.contains(scope.as_ref()) {
            return Ok(HostDecision::AllowForSession);
        }
        self.next_request_id = self.next_request_id.checked_add(1).ok_or_else(|| {
            runtime_error(
                "host-request-id-exhausted",
                "host request identifier space is exhausted",
            )
        })?;
        let request = HostRequest::new(self.next_request_id, kind)?;
        self.events.publish(SessionEvent {
            session_id: self.session_id,
            attempt_id: self.attempt_id,
            kind: SessionEventKind::HostRequest(request),
        })?;
        let deadline = Instant::now() + HOST_REQUEST_TIMEOUT;
        loop {
            let decision = self.commands.take_decision(
                self.session_id,
                self.attempt_id,
                self.next_request_id,
            )?;
            // A queued approval or cached permission cannot begin another host
            // operation after Stop. AllowAfterExit is the explicit UI action
            // which queues this stop itself; it performs no effect in the VM.
            if self.pause_pending(resume)?
                || (!resume && self.lifecycle.suspended())
                || (self.request_cancelled(resume)
                    && decision != Some(HostDecision::AllowAfterExit))
            {
                self.close_request()?;
                return Ok(HostDecision::Deny);
            }
            if let Some(decision) = decision {
                if !resume && decision == HostDecision::AllowForSession {
                    if self.allowed_for_session.len() < MAX_SESSION_REQUEST_SCOPES {
                        self.allowed_for_session.insert(scope.into_owned());
                    } else {
                        self.events.publish(SessionEvent {
                            session_id: self.session_id,
                            attempt_id: self.attempt_id,
                            kind: SessionEventKind::Diagnostic {
                                code: "host-request-session-capacity".to_owned(),
                                detail:
                                    "session permission cache is full; this approval applies once"
                                        .to_owned(),
                                repeated: 1,
                            },
                        })?;
                    }
                }
                self.close_request()?;
                return Ok(decision);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.events.publish(SessionEvent {
                    session_id: self.session_id,
                    attempt_id: self.attempt_id,
                    kind: SessionEventKind::Diagnostic {
                        code: "host-request-timeout".to_owned(),
                        detail: "host request timed out and was denied".to_owned(),
                        repeated: 1,
                    },
                })?;
                self.close_request()?;
                return Ok(HostDecision::Deny);
            }
            self.commands.wait_for_change(remaining.min(REQUEST_POLL))?;
        }
    }

    /// A MIDP permission query must never start an interactive request.
    pub(super) fn permission_status(&self, permission: &str) -> i32 {
        if self.request_cancelled(false) {
            return 0;
        }
        let scope = match permission {
            gcf::HTTP_PERMISSION | gcf::HTTPS_PERMISSION => "network",
            gcf::FILE_READ_PERMISSION | gcf::FILE_WRITE_PERMISSION => "file-connection",
            _ => return 0,
        };
        if self.allowed_for_session.contains(scope) {
            1
        } else {
            -1
        }
    }

    fn request_cancelled(&self, resume: bool) -> bool {
        (!resume && self.lifecycle.suspended())
            || self.lifecycle.destroyed()
            || self.urgent.shutdown_requested()
            || self.urgent.stop_requested(self.attempt_id)
            || self.urgent.force_cancelled()
    }

    fn pause_pending(&self, resume: bool) -> Result<bool, EmuError> {
        // A native call waiting for permission blocks the driver from draining
        // pause commands. Leave them queued for its input/output barrier, but
        // deny host access now so that the driver can process them promptly.
        Ok(!resume
            && self
                .commands
                .pause_pending(self.session_id, self.attempt_id)?)
    }

    fn close_request(&self) -> Result<(), EmuError> {
        self.events.publish(SessionEvent {
            session_id: self.session_id,
            attempt_id: self.attempt_id,
            kind: SessionEventKind::HostRequestClosed {
                request_id: self.next_request_id,
            },
        })
    }

    pub(super) fn require_access(&mut self, kind: HostRequestKind) -> Result<(), EmuError> {
        match self.decide(kind)? {
            HostDecision::AllowOnce | HostDecision::AllowForSession => Ok(()),
            HostDecision::Deny | HostDecision::AllowAfterExit | HostDecision::ResumeGame { .. } => {
                Err(EmuError::new(
                    Category::Api,
                    "security-exception",
                    "access denied by user or host policy",
                ))
            }
        }
    }
}

fn request_scope(kind: &HostRequestKind) -> Cow<'static, str> {
    match kind {
        HostRequestKind::ResumeGame { .. } => Cow::Borrowed("resume-game"),
        HostRequestKind::Network { .. } => Cow::Borrowed("network"),
        HostRequestKind::FileConnection { .. } => Cow::Borrowed("file-connection"),
        HostRequestKind::PlatformRequest { url } => format!("platform:{url}").into(),
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/frontend-core/runtime/requests.rs"]
mod tests;
