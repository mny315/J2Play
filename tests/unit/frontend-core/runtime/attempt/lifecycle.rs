use super::*;

fn pause_source(driver: &mut AttemptDriver, source: u8, paused: bool) {
    match source {
        0 => driver.set_pause(PauseReason::User, paused).unwrap(),
        1 => driver.set_pause(PauseReason::Lifecycle, paused).unwrap(),
        2 => {
            driver.lifecycle.set_suspended(paused);
            driver.sync_platform_lifecycle().unwrap();
        }
        _ => unreachable!(),
    }
}

#[test]
fn every_pause_source_order_keeps_effects_muted_until_the_last_resume() {
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for pause_order in orders {
        for resume_order in orders {
            let mut driver = driver();
            for source in pause_order {
                pause_source(&mut driver, source, true);
                assert!(driver.is_paused());
                assert_eq!(driver.audio.active_generation().unwrap(), None);
                let stopped = driver.vibration.take_latest().unwrap();
                assert!(
                    stopped.is_none_or(|effect| effect.request == natives::VibrationRequest::Stop)
                );
                assert!(
                    driver
                        .vibration
                        .publish(crate::VibrationEffect {
                            session_id: driver.session_id,
                            attempt_id: driver.attempt_id,
                            request: natives::VibrationRequest::Continuous { level: None },
                        })
                        .unwrap()
                );
                assert!(driver.vibration.take_latest().unwrap().is_none());
            }
            for (index, source) in resume_order.into_iter().enumerate() {
                pause_source(&mut driver, source, false);
                assert_eq!(driver.is_paused(), index != 2);
                assert_eq!(
                    driver.audio.active_generation().unwrap(),
                    (index == 2).then_some((driver.session_id, driver.attempt_id))
                );
            }
        }
    }
}
