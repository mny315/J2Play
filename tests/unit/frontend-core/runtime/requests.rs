use super::*;

#[test]
fn empty_platform_request_cancels_without_prompt_or_request_identifier() {
    let events = EventMailbox::default();
    let mut broker = RequestBroker::new(
        SessionId(1),
        AttemptId(1),
        CommandQueue::default(),
        events.clone(),
        PlatformLifecycleSignal::default(),
    );
    for _ in 0..1000 {
        assert_eq!(
            broker
                .decide(HostRequestKind::PlatformRequest { url: String::new() })
                .unwrap(),
            HostDecision::AllowOnce,
        );
    }
    assert_eq!(broker.next_request_id, 0);
    assert!(broker.allowed_for_session.is_empty());
    let output = events.take_all().unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].kind, SessionEventKind::PlatformRequestCancelled);
    broker.lifecycle.set_suspended(true);
    assert_eq!(
        broker
            .decide(HostRequestKind::PlatformRequest { url: String::new() })
            .unwrap(),
        HostDecision::AllowOnce
    );
    assert_eq!(broker.next_request_id, 0);
    broker
        .commands
        .push(crate::SessionCommand {
            session_id: SessionId(1),
            attempt_id: AttemptId(1),
            kind: crate::SessionCommandKind::Stop,
        })
        .unwrap();
    assert_eq!(
        broker
            .decide(HostRequestKind::PlatformRequest { url: String::new() })
            .unwrap(),
        HostDecision::AllowOnce
    );
    assert_eq!(broker.next_request_id, 0);
}

#[test]
fn permission_queries_share_operation_grants_without_emitting_requests() {
    use crate::{SessionCommand, SessionCommandKind};
    let kinds = [
        (
            HostRequestKind::Network {
                url: "https://example.invalid/".into(),
            },
            [gcf::HTTP_PERMISSION, gcf::HTTPS_PERMISSION],
            [gcf::FILE_READ_PERMISSION, gcf::FILE_WRITE_PERMISSION],
        ),
        (
            HostRequestKind::FileConnection {
                url: "file:///probe.dat".into(),
            },
            [gcf::FILE_READ_PERMISSION, gcf::FILE_WRITE_PERMISSION],
            [gcf::HTTP_PERMISSION, gcf::HTTPS_PERMISSION],
        ),
    ];
    for (kind, granted, other) in kinds {
        for decision in [
            HostDecision::AllowOnce,
            HostDecision::AllowForSession,
            HostDecision::Deny,
        ] {
            let commands = CommandQueue::default();
            let events = EventMailbox::default();
            let mut broker = RequestBroker::new(
                SessionId(1),
                AttemptId(1),
                commands.clone(),
                events.clone(),
                PlatformLifecycleSignal::default(),
            );
            for permission in granted.into_iter().chain(other) {
                assert_eq!(broker.permission_status(permission), -1);
            }
            for permission in [
                "",
                "unknown.permission",
                "javax.microedition.io.Connector.HTTP",
                "javax.microedition.io.Connector.socket",
            ] {
                assert_eq!(broker.permission_status(permission), 0);
            }
            assert!(events.take_all().unwrap().is_empty());
            assert_eq!(broker.next_request_id, 0);
            commands
                .push(SessionCommand {
                    session_id: SessionId(1),
                    attempt_id: AttemptId(1),
                    kind: SessionCommandKind::ResolveHostRequest {
                        request_id: 1,
                        decision,
                    },
                })
                .unwrap();
            assert_eq!(broker.decide(kind.clone()).unwrap(), decision);
            assert_eq!(events.take_all().unwrap().len(), 2);
            for permission in granted {
                assert_eq!(
                    broker.permission_status(permission),
                    if decision == HostDecision::AllowForSession {
                        1
                    } else {
                        -1
                    }
                );
            }
            for permission in other {
                assert_eq!(broker.permission_status(permission), -1);
            }
            assert!(events.take_all().unwrap().is_empty());
            assert_eq!(broker.next_request_id, 1);
            broker.lifecycle.set_suspended(true);
            for permission in granted {
                assert_eq!(broker.permission_status(permission), 0);
            }
            broker.lifecycle.set_suspended(false);
            commands
                .push(SessionCommand {
                    session_id: SessionId(1),
                    attempt_id: AttemptId(1),
                    kind: SessionCommandKind::Stop,
                })
                .unwrap();
            for permission in granted {
                assert_eq!(broker.permission_status(permission), 0);
            }
            assert!(events.take_all().unwrap().is_empty());
        }
    }
}

#[test]
fn suspension_denies_a_waiting_host_request_and_invalidates_cached_approval() {
    let signal = PlatformLifecycleSignal::default();
    let events = EventMailbox::default();
    let suspend = signal.clone();
    events
        .set_waker(Arc::new(move || suspend.set_suspended(true)))
        .unwrap();
    let mut broker = RequestBroker::new(
        SessionId(1),
        AttemptId(1),
        CommandQueue::default(),
        events.clone(),
        signal,
    );
    let kind = HostRequestKind::Network {
        url: "https://example.invalid/".into(),
    };
    let started = Instant::now();
    assert_eq!(broker.decide(kind.clone()).unwrap(), HostDecision::Deny);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(events.take_all().unwrap().iter().any(|event| matches!(
        event.kind,
        SessionEventKind::HostRequestClosed { request_id: 1 }
    )));
    broker
        .allowed_for_session
        .insert(request_scope(&kind).into_owned());
    assert_eq!(broker.decide(kind).unwrap(), HostDecision::Deny);
}

#[test]
fn access_denial_and_stop_return_security_errors_without_waiting_for_network() {
    use crate::{SessionCommand, SessionCommandKind};
    for kind in [
        HostRequestKind::Network {
            url: "https://example.invalid/".into(),
        },
        HostRequestKind::FileConnection {
            url: "file:///probe.dat".into(),
        },
    ] {
        for command in [
            SessionCommandKind::ResolveHostRequest {
                request_id: 1,
                decision: HostDecision::Deny,
            },
            SessionCommandKind::Stop,
        ] {
            let commands = CommandQueue::default();
            commands
                .push(SessionCommand {
                    session_id: SessionId(1),
                    attempt_id: AttemptId(1),
                    kind: command,
                })
                .unwrap();
            let mut broker = RequestBroker::new(
                SessionId(1),
                AttemptId(1),
                commands,
                EventMailbox::default(),
                PlatformLifecycleSignal::default(),
            );
            let started = Instant::now();
            let error = broker.require_access(kind.clone()).unwrap_err();
            assert_eq!(error.code(), "security-exception");
            assert!(started.elapsed() < Duration::from_secs(1));
        }
    }
}

#[test]
fn pending_pause_overrides_cached_and_queued_host_approvals() {
    use crate::{PauseReason, SessionCommand, SessionCommandKind};

    let kind = HostRequestKind::Network {
        url: "https://example.invalid/".into(),
    };
    for reason in [PauseReason::User, PauseReason::Lifecycle] {
        for cached in [false, true] {
            let commands = CommandQueue::default();
            let events = EventMailbox::default();
            let mut broker = RequestBroker::new(
                SessionId(1),
                AttemptId(1),
                commands.clone(),
                events.clone(),
                PlatformLifecycleSignal::default(),
            );
            if cached {
                broker
                    .allowed_for_session
                    .insert(request_scope(&kind).into_owned());
            }
            for command in [
                SessionCommandKind::SetPause {
                    reason,
                    paused: true,
                },
                SessionCommandKind::ResolveHostRequest {
                    request_id: 1,
                    decision: HostDecision::AllowOnce,
                },
            ] {
                commands
                    .push(SessionCommand {
                        session_id: SessionId(1),
                        attempt_id: AttemptId(1),
                        kind: command,
                    })
                    .unwrap();
            }
            assert_eq!(broker.decide(kind.clone()).unwrap(), HostDecision::Deny);
            assert!(events.take_all().unwrap().is_empty());
            // The driver must still apply the pause and its input/output barrier.
            assert!(matches!(
                commands.pop_priority().unwrap().unwrap().kind,
                SessionCommandKind::SetPause { paused: true, .. }
            ));
        }
    }
}

#[test]
fn stop_overrides_cached_and_queued_host_approvals() {
    use crate::{SessionCommand, SessionCommandKind};
    use std::sync::atomic::{AtomicBool, Ordering};

    let kind = HostRequestKind::Network {
        url: "https://example.invalid/".into(),
    };
    let command = |kind| SessionCommand {
        session_id: SessionId(1),
        attempt_id: AttemptId(1),
        kind,
    };
    let commands = CommandQueue::default();
    let mut cached = RequestBroker::new(
        SessionId(1),
        AttemptId(1),
        commands.clone(),
        EventMailbox::default(),
        PlatformLifecycleSignal::default(),
    );
    cached
        .allowed_for_session
        .insert(request_scope(&kind).into_owned());
    commands.push(command(SessionCommandKind::Stop)).unwrap();
    assert_eq!(cached.decide(kind.clone()).unwrap(), HostDecision::Deny);

    for approval in [
        HostDecision::AllowOnce,
        HostDecision::AllowForSession,
        HostDecision::AllowAfterExit,
    ] {
        let commands = CommandQueue::default();
        let events = EventMailbox::default();
        let responded = AtomicBool::new(false);
        let response_queue = commands.clone();
        events
            .set_waker(Arc::new(move || {
                if !responded.swap(true, Ordering::Relaxed) {
                    response_queue
                        .push(command(SessionCommandKind::ResolveHostRequest {
                            request_id: 1,
                            decision: approval,
                        }))
                        .unwrap();
                    response_queue
                        .push(command(SessionCommandKind::Stop))
                        .unwrap();
                }
            }))
            .unwrap();
        let mut broker = RequestBroker::new(
            SessionId(1),
            AttemptId(1),
            commands,
            events.clone(),
            PlatformLifecycleSignal::default(),
        );
        let request_kind = if approval == HostDecision::AllowAfterExit {
            HostRequestKind::PlatformRequest {
                url: "https://example.invalid/".into(),
            }
        } else {
            kind.clone()
        };
        let expected = if approval == HostDecision::AllowAfterExit {
            approval
        } else {
            HostDecision::Deny
        };
        assert_eq!(broker.decide(request_kind).unwrap(), expected);
        assert!(broker.allowed_for_session.is_empty());
        assert!(events.take_all().unwrap().iter().any(|event| matches!(
            event.kind,
            SessionEventKind::HostRequestClosed { request_id: 1 }
        )));
    }
}

#[test]
fn pause_during_a_request_closes_it_without_granting_access() {
    use crate::{PauseReason, SessionCommand, SessionCommandKind};
    use std::sync::atomic::{AtomicBool, Ordering};

    for approval in [HostDecision::AllowOnce, HostDecision::AllowForSession] {
        let commands = CommandQueue::default();
        let events = EventMailbox::default();
        let responded = AtomicBool::new(false);
        let response_queue = commands.clone();
        events
            .set_waker(Arc::new(move || {
                if !responded.swap(true, Ordering::Relaxed) {
                    for kind in [
                        SessionCommandKind::SetPause {
                            reason: PauseReason::User,
                            paused: true,
                        },
                        SessionCommandKind::SetPause {
                            reason: PauseReason::User,
                            paused: false,
                        },
                        SessionCommandKind::ResolveHostRequest {
                            request_id: 1,
                            decision: approval,
                        },
                    ] {
                        response_queue
                            .push(SessionCommand {
                                session_id: SessionId(1),
                                attempt_id: AttemptId(1),
                                kind,
                            })
                            .unwrap();
                    }
                }
            }))
            .unwrap();
        let mut broker = RequestBroker::new(
            SessionId(1),
            AttemptId(1),
            commands.clone(),
            events.clone(),
            PlatformLifecycleSignal::default(),
        );
        assert_eq!(
            broker
                .decide(HostRequestKind::Network {
                    url: "https://example.invalid/".into(),
                })
                .unwrap(),
            HostDecision::Deny
        );
        assert!(broker.allowed_for_session.is_empty());
        assert!(events.take_all().unwrap().iter().any(|event| matches!(
            event.kind,
            SessionEventKind::HostRequestClosed { request_id: 1 }
        )));
        for paused in [true, false] {
            assert_eq!(
                commands.pop_priority().unwrap().unwrap().kind,
                SessionCommandKind::SetPause {
                    reason: PauseReason::User,
                    paused
                }
            );
        }
    }
}

#[test]
fn unrelated_pauses_and_resume_dialogs_do_not_cancel_host_decisions() {
    use crate::{PauseReason, SessionCommand, SessionCommandKind};

    for (session_id, attempt_id, resume) in [(2, 1, false), (1, 2, false), (1, 1, true)] {
        let commands = CommandQueue::default();
        commands
            .push(SessionCommand {
                session_id: SessionId(session_id),
                attempt_id: AttemptId(attempt_id),
                kind: SessionCommandKind::SetPause {
                    reason: PauseReason::User,
                    paused: true,
                },
            })
            .unwrap();
        let (kind, decision) = if resume {
            (
                HostRequestKind::ResumeGame {
                    title: "Fixture".into(),
                    can_resume: true,
                    detail: String::new(),
                },
                HostDecision::ResumeGame {
                    continue_game: true,
                    remember: false,
                },
            )
        } else {
            (
                HostRequestKind::Network {
                    url: "https://example.invalid/".into(),
                },
                HostDecision::AllowOnce,
            )
        };
        commands
            .push(SessionCommand {
                session_id: SessionId(1),
                attempt_id: AttemptId(1),
                kind: SessionCommandKind::ResolveHostRequest {
                    request_id: 1,
                    decision,
                },
            })
            .unwrap();
        let mut broker = RequestBroker::new(
            SessionId(1),
            AttemptId(1),
            commands,
            EventMailbox::default(),
            PlatformLifecycleSignal::default(),
        );
        assert_eq!(broker.decide(kind).unwrap(), decision);
    }
}

#[test]
fn shutdown_and_destroy_close_pending_requests_without_consuming_a_late_approval() {
    use crate::{SessionCommand, SessionCommandKind};
    for destroyed in [false, true] {
        let commands = CommandQueue::default();
        let events = EventMailbox::default();
        let signal = PlatformLifecycleSignal::default();
        let cancel_commands = commands.clone();
        let cancel_signal = signal.clone();
        events
            .set_waker(Arc::new(move || {
                if destroyed {
                    cancel_signal.mark_destroyed();
                } else {
                    cancel_commands.request_shutdown();
                }
            }))
            .unwrap();
        let mut broker = RequestBroker::new(
            SessionId(1),
            AttemptId(1),
            commands.clone(),
            events.clone(),
            signal,
        );
        let kind = HostRequestKind::Network {
            url: "https://example.invalid/".into(),
        };
        assert_eq!(broker.decide(kind.clone()).unwrap(), HostDecision::Deny);
        assert!(events.take_all().unwrap().iter().any(|event| matches!(
            event.kind,
            SessionEventKind::HostRequestClosed { request_id: 1 }
        )));
        let _ = commands.push(SessionCommand {
            session_id: SessionId(1),
            attempt_id: AttemptId(1),
            kind: SessionCommandKind::ResolveHostRequest {
                request_id: 1,
                decision: HostDecision::AllowForSession,
            },
        });
        assert_eq!(broker.decide(kind).unwrap(), HostDecision::Deny);
        assert!(broker.allowed_for_session.is_empty());
        assert_eq!(broker.next_request_id, 1);
    }
}
