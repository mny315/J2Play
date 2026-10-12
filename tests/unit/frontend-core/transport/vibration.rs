use super::*;
use crate::{VibrationRequest, VibrationSettings};

#[test]
fn new_vibration_generation_preserves_an_undelivered_stop() {
    let mailbox = VibrationMailbox::default();
    mailbox.activate(SessionId(1), AttemptId(1)).unwrap();
    let first = VibrationEffect {
        session_id: SessionId(1),
        attempt_id: AttemptId(1),
        request: VibrationRequest::Continuous { level: None },
    };
    assert!(mailbox.publish(first).unwrap());
    assert_eq!(mailbox.take_latest().unwrap(), Some(first));
    mailbox.stop_and_deactivate().unwrap();
    mailbox.activate(SessionId(2), AttemptId(2)).unwrap();
    assert!(!mailbox.publish(first).unwrap());
    assert_eq!(
        mailbox.take_latest().unwrap(),
        Some(VibrationEffect {
            session_id: SessionId(2),
            attempt_id: AttemptId(2),
            request: VibrationRequest::Stop,
        })
    );
}

#[test]
fn missing_actuator_rejects_guest_vibration_across_launches_and_hotplug() {
    let mailbox = VibrationMailbox::default();
    mailbox.set_available(false).unwrap();
    for id in [1, 2] {
        mailbox.activate(SessionId(id), AttemptId(id)).unwrap();
        let effect = VibrationEffect {
            session_id: SessionId(id),
            attempt_id: AttemptId(id),
            request: VibrationRequest::Continuous { level: None },
        };
        assert!(!mailbox.publish(effect).unwrap());
        mailbox.set_available(true).unwrap();
        assert!(mailbox.publish(effect).unwrap());
        mailbox.set_available(false).unwrap();
        assert_eq!(
            mailbox.take_latest().unwrap().unwrap().request,
            VibrationRequest::Stop
        );
        assert!(!mailbox.publish(effect).unwrap());
    }
}

#[test]
fn paused_effect_mailboxes_discard_new_audio_and_vibration() {
    let session_id = SessionId(1);
    let attempt_id = AttemptId(2);
    let audio = AudioMailbox::new(3);
    audio.activate(session_id, attempt_id).unwrap();
    audio.set_suspended(session_id, attempt_id, true).unwrap();
    assert!(audio.publish(session_id, attempt_id, &[1, 2]).unwrap());
    assert_eq!(audio.status().unwrap().active_generation, None);
    assert_eq!(audio.status().unwrap().queued_frames, 0);

    let vibration = VibrationMailbox::default();
    vibration.activate(session_id, attempt_id).unwrap();
    vibration
        .set_suspended(session_id, attempt_id, true)
        .unwrap();
    assert!(
        vibration
            .publish(VibrationEffect {
                session_id,
                attempt_id,
                request: VibrationRequest::Continuous { level: Some(50) },
            })
            .unwrap()
    );
    assert_eq!(
        vibration.take_latest().unwrap().unwrap().request,
        VibrationRequest::Stop
    );
}

#[test]
fn vibration_mailbox_applies_the_active_attempt_policy() {
    let session_id = SessionId(3);
    let attempt_id = AttemptId(4);
    let vibration = VibrationMailbox::default();
    vibration.activate(session_id, attempt_id).unwrap();
    vibration
        .configure(
            session_id,
            attempt_id,
            VibrationSettings {
                enabled: true,
                strength_percent: 25,
            },
        )
        .unwrap();
    assert!(
        vibration
            .publish(VibrationEffect {
                session_id,
                attempt_id,
                request: VibrationRequest::Timed {
                    duration_millis: 40,
                    level: Some(80),
                },
            })
            .unwrap()
    );
    assert_eq!(
        vibration.take_latest().unwrap().unwrap().request,
        VibrationRequest::Timed {
            duration_millis: 40,
            level: Some(20),
        }
    );

    vibration
        .configure(
            session_id,
            attempt_id,
            VibrationSettings {
                enabled: false,
                strength_percent: 25,
            },
        )
        .unwrap();
    vibration
        .publish(VibrationEffect {
            session_id,
            attempt_id,
            request: VibrationRequest::Continuous { level: None },
        })
        .unwrap();
    assert_eq!(
        vibration.take_latest().unwrap().unwrap().request,
        VibrationRequest::Stop
    );
}
