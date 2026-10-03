use super::*;

#[test]
fn audio_overflow_drops_oldest_and_underrun_is_silence() {
    let mailbox = AudioMailbox::new(3);
    mailbox.activate(SessionId(1), AttemptId(1)).unwrap();
    mailbox
        .publish(SessionId(1), AttemptId(1), &[1, 2, 3, 4])
        .unwrap();
    let mut output = [9; 5];
    assert_eq!(mailbox.read_or_silence(&mut output).unwrap(), 3);
    assert_eq!(output, [2, 3, 4, 0, 0]);
    assert_eq!(mailbox.stats().unwrap().dropped_frames, 1);
    assert_eq!(mailbox.stats().unwrap().underrun_frames, 2);
}

#[test]
fn audio_suspend_revision_changes_without_changing_attempt() {
    let mailbox = AudioMailbox::new(3);
    mailbox.activate(SessionId(1), AttemptId(2)).unwrap();
    let before = mailbox.status().unwrap();
    mailbox
        .publish(SessionId(1), AttemptId(2), &[1, 2])
        .unwrap();
    mailbox
        .set_suspended(SessionId(1), AttemptId(2), true)
        .unwrap();
    let after = mailbox.status().unwrap();
    assert_eq!(before.active_generation, Some((SessionId(1), AttemptId(2))));
    assert_eq!(after.active_generation, None);
    assert_eq!(after.queued_frames, 0);
    assert!(after.flush_revision > before.flush_revision);
}

#[test]
fn available_audio_keeps_wrapped_samples_and_their_flush_marker_together() {
    let mailbox = AudioMailbox::new(5);
    let first = (SessionId(1), AttemptId(1));
    mailbox.activate(first.0, first.1).unwrap();
    mailbox.publish(first.0, first.1, &[1, 2, 3, 4, 5]).unwrap();
    let mut output = [99; 3];
    let (marker, copied) = mailbox.read_available(&mut output).unwrap();
    assert_eq!(marker.active_generation, Some(first));
    assert_eq!(marker.queued_frames, 5);
    assert_eq!(copied, 3);
    assert_eq!(output, [1, 2, 3]);

    mailbox.publish(first.0, first.1, &[6, 7, 8]).unwrap();
    let mut wrapped = [99; 6];
    let (next, copied) = mailbox.read_available(&mut wrapped).unwrap();
    assert_eq!(next, marker);
    assert_eq!(copied, 5);
    assert_eq!(wrapped, [4, 5, 6, 7, 8, 99]);

    mailbox.publish(first.0, first.1, &[9]).unwrap();
    mailbox.set_suspended(first.0, first.1, true).unwrap();
    let (paused, copied) = mailbox.read_available(&mut output).unwrap();
    assert_eq!(paused.active_generation, None);
    assert!(paused.flush_revision > marker.flush_revision);
    assert_eq!(copied, 0);
    assert_eq!(output, [1, 2, 3]);

    let second = (SessionId(2), AttemptId(2));
    mailbox.activate(second.0, second.1).unwrap();
    assert!(!mailbox.publish(first.0, first.1, &[10]).unwrap());
    mailbox.publish(second.0, second.1, &[20, 21]).unwrap();
    let (restarted, copied) = mailbox.read_available(&mut output).unwrap();
    assert_eq!(restarted.active_generation, Some(second));
    assert!(restarted.flush_revision > paused.flush_revision);
    assert_eq!(copied, 2);
    assert_eq!(output, [20, 21, 3]);
    assert_eq!(mailbox.stats().unwrap().underrun_frames, 0);
}
