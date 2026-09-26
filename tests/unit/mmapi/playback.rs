use super::*;

#[test]
fn audio_tick_partitioning_after_silence_preserves_every_sample() {
    let samples: Vec<i16> = (100..164).collect();
    let mut whole = Runtime::new(CaptureSink::default(), Limits::default());
    let mut sliced = Runtime::new(CaptureSink::default(), Limits::default());
    for runtime in [&mut whole, &mut sliced] {
        runtime.tick(40).unwrap();
        let handle = runtime
            .create_from_bytes("audio/wav", &wav(&samples))
            .unwrap();
        runtime.start(handle, 40).unwrap();
    }
    whole.tick(1_000).unwrap();
    for time in (40..1_000).step_by(17) {
        sliced.tick(time).unwrap();
    }
    sliced.tick(1_000).unwrap();
    assert_eq!(whole.sink().0, samples[..21]);
    assert_eq!(sliced.sink().0, whole.sink().0);
}

#[test]
fn a_failed_audio_write_does_not_advance_fractional_time() {
    struct RetrySink {
        failed: bool,
        samples: Vec<i16>,
    }
    impl AudioSink for RetrySink {
        fn write(&mut self, samples: &[i16]) -> Result<(), EmuError> {
            if !self.failed {
                self.failed = true;
                return Err(media_error("fixture-sink", "injected output failure"));
            }
            self.samples.extend_from_slice(samples);
            Ok(())
        }
    }
    let mut runtime = Runtime::new(
        RetrySink {
            failed: false,
            samples: Vec::new(),
        },
        Limits::default(),
    );
    let handle = runtime
        .create_from_bytes("audio/wav", &wav(&[1, 2, 3]))
        .unwrap();
    runtime.start(handle, 0).unwrap();
    assert!(runtime.tick(60).is_err());
    runtime.tick(60).unwrap();
    runtime.tick(90).unwrap();
    assert_eq!(runtime.sink().samples, [1]);
}

#[test]
fn short_ticks_play_the_last_sample_of_every_loop() {
    let samples: Vec<i16> = (1..=64).collect();
    let mut runtime = Runtime::new(CaptureSink::default(), Limits::default());
    let handle = runtime
        .create_from_bytes("audio/wav", &wav(&samples))
        .unwrap();
    runtime.set_loop_count(handle, 2).unwrap();
    runtime.start(handle, 0).unwrap();
    for time in (0..6_000).step_by(17) {
        runtime.tick(time).unwrap();
    }
    assert_eq!(
        runtime.state(handle, 6_000).unwrap(),
        PlayerState::Prefetched
    );
    let expected = samples.repeat(2);
    assert_eq!(runtime.sink().0[..expected.len()], expected);
    assert!(
        runtime.sink().0[expected.len()..]
            .iter()
            .all(|sample| *sample == 0)
    );
}

#[test]
fn changing_loop_count_after_stop_preserves_the_next_pcm_sample() {
    let mut runtime = Runtime::new(CaptureSink::default(), Limits::default());
    let handle = runtime
        .create_from_bytes("audio/wav", &wav(&[100, 200]))
        .unwrap();
    runtime.set_loop_count(handle, -1).unwrap();
    runtime.start(handle, 0).unwrap();
    runtime.stop(handle, 20_000).unwrap();
    assert_eq!(runtime.sink().0.len(), 441);
    assert_eq!(runtime.sink().0.last(), Some(&100));

    runtime.set_loop_count(handle, 1).unwrap();
    runtime.start(handle, 20_000).unwrap();
    runtime.tick(20_100).unwrap();
    assert_eq!(&runtime.sink().0[441..], &[200, 0]);
    assert_eq!(
        runtime.state(handle, 20_100).unwrap(),
        PlayerState::Prefetched
    );
}

#[test]
fn seeking_to_media_end_does_not_replay_the_last_pcm_sample() {
    let mut runtime = Runtime::new(CaptureSink::default(), Limits::default());
    let handle = runtime
        .create_from_bytes("audio/wav", &wav(&[100, 200, 300]))
        .unwrap();
    runtime.start(handle, 0).unwrap();
    let duration = runtime.duration(handle).unwrap();
    assert_eq!(
        runtime.set_media_time(handle, i64::MAX, 0).unwrap(),
        duration
    );
    runtime.tick(100).unwrap();
    assert_eq!(runtime.sink().0, [0, 0]);
    assert_eq!(runtime.state(handle, 100).unwrap(), PlayerState::Prefetched);
    assert_eq!(runtime.media_time(handle, 100).unwrap(), duration);
}

#[test]
fn state_machine_loop_stop_resume_and_events_are_deterministic() {
    let mut runtime = Runtime::new(CaptureSink::default(), Limits::default());
    let handle = runtime
        .create_from_bytes(
            "audio/x-wav",
            &wav(&vec![1_000; OUTPUT_SAMPLE_RATE as usize / 10]),
        )
        .unwrap();
    assert_eq!(runtime.state(handle, 0).unwrap(), PlayerState::Unrealized);
    runtime.realize(handle).unwrap();
    assert!(matches!(
        runtime.players[&handle].encoded,
        EncodedMedia::Decoded
    ));
    assert_eq!(runtime.duration(handle).unwrap(), 100_000);
    runtime.set_loop_count(handle, 2).unwrap();
    runtime.start(handle, 0).unwrap();
    assert_eq!(runtime.state(handle, 50_000).unwrap(), PlayerState::Started);
    runtime.stop(handle, 50_000).unwrap();
    assert_eq!(runtime.media_time(handle, 50_000).unwrap(), 50_000);
    runtime.start(handle, 100_000).unwrap();
    assert_eq!(
        runtime.state(handle, 250_000).unwrap(),
        PlayerState::Prefetched
    );
    let events = std::iter::from_fn(|| runtime.next_event(handle, 250_000).unwrap())
        .map(|event| event.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        events,
        vec![
            PlayerEventKind::Started,
            PlayerEventKind::Stopped,
            PlayerEventKind::Started,
            PlayerEventKind::EndOfMedia,
            PlayerEventKind::Started,
            PlayerEventKind::EndOfMedia,
        ]
    );
    assert_eq!(runtime.sink().0.len(), OUTPUT_SAMPLE_RATE as usize / 5);
}

#[test]
fn volume_mute_and_two_player_mixer_saturate_without_a_device() {
    let mut runtime = Runtime::new(CaptureSink::default(), Limits::default());
    let clip = wav(&vec![30_000; OUTPUT_SAMPLE_RATE as usize / 10]);
    let first = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();
    let second = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();
    runtime.prefetch(first).unwrap();
    runtime.prefetch(second).unwrap();
    assert_eq!(runtime.set_volume(second, 50).unwrap(), 50);
    runtime.start(first, 0).unwrap();
    runtime.start(second, 0).unwrap();
    runtime.tick(10_000).unwrap();
    assert_eq!(runtime.sink().0, [i16::MAX; 220]);
    runtime.set_muted(first, true).unwrap();
    assert!(runtime.muted(first).unwrap());
    runtime.tick(20_000).unwrap();
    assert_eq!(runtime.sink().0[220..], [15_000; 221]);
    runtime.set_volume(second, 0).unwrap();
    runtime.tick(30_000).unwrap();
    assert_eq!(runtime.sink().0[441..], [0; 220]);
}

#[test]
fn exclusive_players_replace_only_the_previous_group_member() {
    let mut runtime = Runtime::new(CaptureSink::default(), Limits::default());
    let clip = wav(&vec![1_000; OUTPUT_SAMPLE_RATE as usize / 10]);
    let ordinary = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();
    let first = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();
    let second = runtime.create_from_bytes("audio/x-wav", &clip).unwrap();

    runtime.start(ordinary, 0).unwrap();
    runtime.start_exclusive(first, 0).unwrap();
    runtime.start_exclusive(second, 10_000).unwrap();

    assert_eq!(runtime.players[&ordinary].state, PlayerState::Started);
    assert_eq!(runtime.players[&first].state, PlayerState::Prefetched);
    assert_eq!(runtime.players[&second].state, PlayerState::Started);
    assert_eq!(runtime.exclusive_player, Some(second));
}

#[test]
fn mixer_reuses_tick_scratch_buffers() {
    let mut runtime = Runtime::new(CaptureSink::default(), Limits::default());
    let handle = runtime
        .create_from_bytes(
            "audio/x-wav",
            &wav(&vec![1_000; OUTPUT_SAMPLE_RATE as usize / 10]),
        )
        .unwrap();
    runtime.start(handle, 0).unwrap();
    runtime.tick(10_000).unwrap();
    // The 22,050 Hz remainder makes the next 10 ms block one frame
    // larger; warm both buffers to that observed high-water mark.
    runtime.tick(20_000).unwrap();
    let mix_pointer = runtime.mix_scratch.as_ptr();
    let output_pointer = runtime.output_scratch.as_ptr();
    let mix_capacity = runtime.mix_scratch.capacity();
    let output_capacity = runtime.output_scratch.capacity();
    runtime.tick(30_000).unwrap();
    assert_eq!(runtime.mix_scratch.capacity(), mix_capacity);
    assert_eq!(runtime.output_scratch.capacity(), output_capacity);
    assert_eq!(runtime.mix_scratch.as_ptr(), mix_pointer);
    assert_eq!(runtime.output_scratch.as_ptr(), output_pointer);
}

#[test]
fn volume_control_state_is_observable_before_realize() {
    let mut runtime = Runtime::new(CaptureSink::default(), Limits::default());
    let handle = runtime.create_tone_player().unwrap();
    assert_eq!(runtime.state(handle, 0).unwrap(), PlayerState::Unrealized);
    assert_eq!(runtime.volume(handle).unwrap(), 100);
    assert_eq!(runtime.set_volume(handle, 37).unwrap(), 37);
    runtime.set_muted(handle, true).unwrap();
    assert!(runtime.muted(handle).unwrap());
    runtime.close(handle, 0).unwrap();
    assert!(runtime.volume(handle).is_err());
}

#[test]
fn idle_time_is_skipped_and_play_tone_replaces_the_previous_tone() {
    let mut runtime = Runtime::new(NullAudioSink::default(), Limits::default());
    runtime.tick(600_000_000).unwrap();
    runtime.play_tone(69, 100, 50, 600_000_000).unwrap();
    runtime.play_tone(72, 100, 50, 600_000_000).unwrap();
    assert!(runtime.one_shot.is_some());
    runtime.tick(600_100_000).unwrap();
    assert_eq!(
        runtime.sink().frames_written(),
        u64::from(OUTPUT_SAMPLE_RATE / 10)
    );
}

#[test]
fn generated_events_cannot_replay_audio_when_the_listener_queue_is_full() {
    let limits = Limits {
        max_pending_events: 1,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let encoded = wav(&[1_000]);
    let handle = runtime.create_from_bytes("audio/x-wav", &encoded).unwrap();
    runtime.start(handle, 0).unwrap();

    runtime.tick(1_000).unwrap();
    let written = runtime.sink().frames_written();
    assert_eq!(
        runtime.state(handle, 1_000).unwrap(),
        PlayerState::Prefetched
    );
    assert_eq!(
        runtime.next_event(handle, 1_000).unwrap().unwrap().kind,
        PlayerEventKind::EndOfMedia
    );
    runtime.tick(1_000).unwrap();
    assert_eq!(runtime.sink().frames_written(), written);

    let second = runtime.create_from_bytes("audio/x-wav", &encoded).unwrap();
    runtime.start(second, 1_000).unwrap();
    runtime.close(second, 1_000).unwrap();
    assert_eq!(runtime.state(second, 1_000).unwrap(), PlayerState::Closed);
    assert_eq!(
        runtime.next_event(second, 1_000).unwrap().unwrap().kind,
        PlayerEventKind::Closed
    );
}

#[test]
fn bounded_loop_events_retain_the_actual_terminal_lifecycle_state() {
    let limits = Limits {
        max_pending_events: 1,
        ..Limits::default()
    };
    let mut runtime = Runtime::new(NullAudioSink::default(), limits);
    let encoded = wav(&[1_000]);

    let finite = runtime.create_from_bytes("audio/x-wav", &encoded).unwrap();
    runtime.set_loop_count(finite, 32).unwrap();
    runtime.start(finite, 0).unwrap();
    runtime.tick(1_000_000).unwrap();
    assert_eq!(
        runtime.state(finite, 1_000_000).unwrap(),
        PlayerState::Prefetched
    );
    assert_eq!(
        runtime.next_event(finite, 1_000_000).unwrap().unwrap().kind,
        PlayerEventKind::EndOfMedia
    );

    let infinite = runtime.create_from_bytes("audio/x-wav", &encoded).unwrap();
    runtime.set_loop_count(infinite, -1).unwrap();
    runtime.start(infinite, 1_000_000).unwrap();
    runtime.tick(2_000_000).unwrap();
    assert_eq!(
        runtime.state(infinite, 2_000_000).unwrap(),
        PlayerState::Started
    );
    assert_eq!(
        runtime
            .next_event(infinite, 2_000_000)
            .unwrap()
            .unwrap()
            .kind,
        PlayerEventKind::Started
    );
}
