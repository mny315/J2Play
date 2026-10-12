use super::*;
use crate::{GameplayScreen, Instant, SessionEvent};

mod diagonals;
mod focus;
mod launch;
mod physical;
mod stick;
mod transition;

fn update_virtual_key(
    owners: &mut std::collections::HashMap<u64, TouchOwner>,
    touch_id: u64,
    action: Option<HostAction>,
) -> [Option<InputEvent>; 2] {
    let [release, second_release, press, second_press] =
        update_virtual_control(owners, touch_id, TouchOwner::VirtualKey(action));
    assert!(second_release.is_none() && second_press.is_none());
    [release, press]
}

#[test]
fn keypad_drag_releases_before_pressing_and_preserves_other_fingers() {
    let mut owners = std::collections::HashMap::new();
    let key = |action, state| Some(InputEvent::Key { action, state });
    assert_eq!(
        update_virtual_key(&mut owners, 1, Some(HostAction::Up)),
        [None, key(HostAction::Up, KeyState::Pressed)]
    );
    assert_eq!(
        update_virtual_key(&mut owners, 1, Some(HostAction::Up)),
        [None, None]
    );
    assert_eq!(
        update_virtual_key(&mut owners, 2, Some(HostAction::Fire)),
        [None, key(HostAction::Fire, KeyState::Pressed)]
    );
    assert_eq!(
        update_virtual_key(&mut owners, 1, Some(HostAction::Num3)),
        [
            key(HostAction::Up, KeyState::Released),
            key(HostAction::Num3, KeyState::Pressed),
        ]
    );
    assert_eq!(
        update_virtual_key(&mut owners, 1, None),
        [key(HostAction::Num3, KeyState::Released), None]
    );
    assert_eq!(owners[&1], TouchOwner::VirtualKey(None));
    assert_eq!(owners[&2], TouchOwner::VirtualKey(Some(HostAction::Fire)));
    assert_eq!(
        update_virtual_key(&mut owners, 1, Some(HostAction::Fire)),
        [None, None]
    );
    assert_eq!(update_virtual_key(&mut owners, 2, None), [None, None]);
    assert_eq!(
        update_virtual_key(&mut owners, 1, None),
        [key(HostAction::Fire, KeyState::Released), None]
    );
}

#[test]
fn keypad_drag_haptics_follow_new_keys_and_respect_off_and_cancellation() {
    use crate::{DocumentKind, DocumentOutcome, PlatformBridge, VibrationSettings};
    use std::sync::{Arc, Mutex};

    struct HapticPlatform(Arc<Mutex<Vec<u8>>>);
    impl PlatformBridge for HapticPlatform {
        fn request_document(&mut self, _: DocumentKind) -> Result<(), EmuError> {
            unreachable!("touch fixture does not open documents")
        }

        fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
            None
        }

        fn request_game_haptic(&mut self, strength: u8) -> Result<bool, EmuError> {
            self.0.lock().unwrap().push(strength);
            Ok(true)
        }
    }

    let scratch = crate::tests::test_storage::Scratch::new();
    let ticks = Arc::new(Mutex::new(Vec::new()));
    let mut app = FrontendApp::new(&scratch.0, Box::new(HapticPlatform(ticks.clone()))).unwrap();
    app.screen = Screen::Gameplay(Box::new(GameplayScreen {
        entry_id: "fixture".into(),
        title: "Touch fixture".into(),
        canvas_dimensions: (240, 320),
        pointer_events: false,
        game_scale: crate::GameScale::AutomaticFit,
        portrait_frame_percent: None,
        control_layout: crate::VirtualControlLayout::default(),
        landscape_control_layout: crate::VirtualControlLayout::default(),
        vibration: VibrationSettings {
            enabled: true,
            strength_percent: 37,
        },
        orientation: None,
        fast_forward: false,
        fullscreen: false,
    }));
    for (index, action) in [
        HostAction::Up,
        HostAction::Num3,
        HostAction::Right,
        HostAction::Fire,
    ]
    .into_iter()
    .enumerate()
    {
        let x = f32::from(u16::try_from(index).unwrap()) * 60.0;
        app.control_regions.push((
            egui::Rect::from_min_size(Pos2::new(x, 0.0), egui::Vec2::splat(50.0)),
            TouchOwner::VirtualKey(Some(action)),
        ));
    }
    let up = Pos2::new(25.0, 25.0);
    let diagonal = Pos2::new(85.0, 25.0);
    let right = Pos2::new(145.0, 25.0);
    let fire = Pos2::new(205.0, 25.0);
    let gap = Pos2::new(55.0, 25.0);
    app.process_touch(1, TouchPhase::Start, up);
    app.process_touch(1, TouchPhase::Move, Pos2::new(30.0, 25.0));
    assert_eq!(*ticks.lock().unwrap(), [37]);
    app.process_touch(2, TouchPhase::Start, fire);
    app.process_touch(1, TouchPhase::Move, diagonal);
    app.process_touch(1, TouchPhase::Move, right);
    assert_eq!(*ticks.lock().unwrap(), [37; 4]);
    app.process_touch(1, TouchPhase::Move, gap);
    assert_eq!(*ticks.lock().unwrap(), [37; 4]);
    assert_eq!(app.touch_owners[&1], TouchOwner::VirtualKey(None));
    app.process_touch(1, TouchPhase::Move, up);
    assert_eq!(*ticks.lock().unwrap(), [37; 5]);
    let Screen::Gameplay(gameplay) = &mut app.screen else {
        unreachable!()
    };
    gameplay.vibration.enabled = false;
    app.process_touch(1, TouchPhase::Move, right);
    assert_eq!(
        app.touch_owners[&1],
        TouchOwner::VirtualKey(Some(HostAction::Right))
    );
    assert_eq!(*ticks.lock().unwrap(), [37; 5]);
    let Screen::Gameplay(gameplay) = &mut app.screen else {
        unreachable!()
    };
    gameplay.vibration.enabled = true;
    app.process_touch(1, TouchPhase::Cancel, right);
    app.process_touch(1, TouchPhase::Move, up);
    assert!(!app.touch_owners.contains_key(&1));
    assert_eq!(
        app.touch_owners[&2],
        TouchOwner::VirtualKey(Some(HostAction::Fire))
    );
    app.release_all_input();
    app.process_touch(2, TouchPhase::Move, up);
    assert!(app.touch_owners.is_empty());
    assert_eq!(*ticks.lock().unwrap(), [37; 5]);
    app.move_virtual_control(
        3,
        TouchOwner::VirtualKeyPair(HostAction::Num2, HostAction::Num6),
    );
    assert_eq!(*ticks.lock().unwrap(), [37; 6]);
    app.move_virtual_key(3, None);
    assert_eq!(*ticks.lock().unwrap(), [37; 6]);
}

#[test]
fn host_text_batch_has_one_shared_byte_limit() {
    let text = "Я".repeat(2_048);
    assert!(game_input_events(&[Event::Text(text.clone())], true).is_ok());
    assert!(game_input_events(&[Event::Text(text), Event::Text("x".into())], true).is_err());
}

#[test]
fn ime_duplicates_do_not_discard_independent_text_or_reorder_deletion() {
    let delete = Event::Ime(ImeEvent::DeleteSurrounding {
        before_chars: 1,
        after_chars: 0,
    });
    let result = game_input_events(
        &[
            Event::Ime(ImeEvent::Commit("Я".into())),
            Event::Text("Я".into()),
            Event::Text("!".into()),
            delete.clone(),
            Event::Text("?".into()),
        ],
        true,
    )
    .unwrap();
    assert_eq!(
        result,
        vec![
            Event::Text("Я".into()),
            Event::Text("!".into()),
            delete,
            Event::Text("?".into())
        ]
    );
}

#[test]
fn repeated_identical_commits_remain_distinct() {
    let result = game_input_events(
        &[
            Event::Text("a".into()),
            Event::Ime(ImeEvent::Commit("a".into())),
            Event::Ime(ImeEvent::Commit("a".into())),
        ],
        true,
    )
    .unwrap();
    assert_eq!(
        result,
        vec![Event::Text("a".into()), Event::Text("a".into())]
    );
}

#[test]
fn ignored_events_separate_commits_and_text_payloads_require_text_input() {
    let events = [
        Event::Text("a".into()),
        Event::Copy,
        Event::Ime(ImeEvent::Commit("a".into())),
    ];
    assert_eq!(
        game_input_events(&events, true).unwrap(),
        [Event::Text("a".into()), Event::Text("a".into())]
    );
    assert!(game_input_events(&events, false).unwrap().is_empty());

    let oversized = "x".repeat(MAX_TEXT_INPUT_BYTES + 1);
    let preedit = Event::Ime(ImeEvent::Preedit {
        text: oversized.clone(),
        active_range_chars: None,
    });
    assert_eq!(
        game_input_events(std::slice::from_ref(&preedit), true)
            .unwrap_err()
            .code(),
        "ime-composition-limit"
    );
    assert!(
        game_input_events(&[Event::Text(oversized), preedit], false)
            .unwrap()
            .is_empty()
    );
}
