use super::physical::gameplay_fixture;
use super::*;
use eframe::App;
use frontend_core::physical_input::{GamepadButton, PhysicalControl, PhysicalInputEvent};

fn render(app: &mut FrontendApp, ctx: &egui::Context, events: Vec<Event>) {
    let mut frame = eframe::Frame::_new_kittest();
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(453.0, 960.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.logic(ui.ctx(), &mut frame);
            app.ui(ui, &mut frame);
        },
    )
    .drop_without_applying_deltas();
    assert!(app.display_error.is_none(), "{:?}", app.display_error);
}

fn begin(app: &mut FrontendApp, ctx: &egui::Context, pending: bool) {
    app.startup_splash = None;
    app.fullscreen_help = crate::FullscreenHelpState::Acknowledged;
    render(app, ctx, vec![]);
    render(app, ctx, vec![]);
    if pending {
        app.gameplay_transition.request();
    } else {
        app.set_gameplay_fullscreen(true).unwrap();
    }
    render(app, ctx, vec![]);
    assert!(app.gameplay_transition.busy());
    app.observed_input = Some(Vec::new());
}

#[test]
fn gameplay_animation_keeps_keyboard_and_gamepad_live() {
    for pending in [false, true] {
        let (_scratch, mut app, ctx) = gameplay_fixture();
        begin(&mut app, &ctx, pending);
        for pressed in [true, false] {
            render(
                &mut app,
                &ctx,
                vec![Event::Key {
                    key: Key::ArrowUp,
                    physical_key: Some(Key::ArrowUp),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        for pressed in [true, false] {
            app.process_physical_event(
                &ctx,
                PhysicalInputEvent::Button {
                    device: 1,
                    control: PhysicalControl::Gamepad {
                        button: GamepadButton::South,
                    },
                    pressed,
                },
                true,
            );
        }
        assert_eq!(
            app.observed_input.take().unwrap(),
            [
                (HostAction::Up, KeyState::Pressed),
                (HostAction::Up, KeyState::Released),
                (HostAction::Num5, KeyState::Pressed),
                (HostAction::Num5, KeyState::Released),
            ]
            .map(|(action, state)| InputEvent::Key { action, state }),
            "pending={pending}"
        );
    }
}

#[test]
fn gameplay_animation_uses_painted_canvas_and_virtual_key_positions() {
    for pointer in [false, true] {
        let (_scratch, mut app, ctx) = gameplay_fixture();
        let Screen::Gameplay(gameplay) = &mut app.screen else {
            unreachable!()
        };
        gameplay.pointer_events = pointer;
        let (session_id, attempt_id) = app.session.active_ids().unwrap();
        app.accept_frame(
            &ctx,
            std::sync::Arc::new(
                frontend_core::Frame::new(
                    session_id,
                    attempt_id,
                    240,
                    320,
                    vec![0xff00_0000; 240 * 320].into(),
                )
                .unwrap(),
            ),
        );
        begin(&mut app, &ctx, false);
        for phase in [TouchPhase::Start, TouchPhase::End] {
            let position = if pointer {
                app.canvas_rect
                    .expect("painted Canvas must remain interactive")
                    .lerp_inside(egui::vec2(0.501, 0.501))
            } else {
                app.control_regions
                    .iter()
                    .find(|(_, owner)| *owner == TouchOwner::VirtualKey(Some(HostAction::Fire)))
                    .expect("painted virtual keys must remain interactive")
                    .0
                    .center()
            };
            render(
                &mut app,
                &ctx,
                vec![Event::Touch {
                    device_id: egui::TouchDeviceId(1),
                    id: egui::TouchId(7),
                    phase,
                    pos: position,
                    force: None,
                }],
            );
        }
        let expected = if pointer {
            [PointerPhase::Pressed, PointerPhase::Released].map(|phase| {
                InputEvent::Pointer(PointerEvent {
                    touch_id: 7,
                    phase,
                    canvas_x: 120,
                    canvas_y: 160,
                })
            })
        } else {
            [KeyState::Pressed, KeyState::Released].map(|state| InputEvent::Key {
                action: HostAction::Fire,
                state,
            })
        };
        assert_eq!(app.observed_input.take().unwrap(), expected);
        assert!(app.touch_owners.is_empty());
    }
}

fn toolbar_frame(
    app: &FrontendApp,
    ctx: &egui::Context,
    viewport: egui::Rect,
    enabled: bool,
    events: Vec<Event>,
) -> (
    Option<crate::gameplay_toolbar::GameplayToolbarRequest>,
    egui::Rect,
    egui::Rect,
) {
    let mut request = None;
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(viewport),
            events,
            ..Default::default()
        },
        |ui| {
            // Keep the toolbar's clipped portion within the window,
            // so the test distinguishes clipping from window bounds.
            ui.add_space(100.0);
            if !enabled {
                ui.disable();
            }
            request = app.draw_animated_gameplay_toolbar(ui, SessionState::Running);
        },
    );
    // Stop is the final painted toolbar icon, a filled square.
    let (icon, clip) = output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| {
            if let egui::Shape::Rect(rect) = &shape.shape {
                Some((rect.rect, shape.clip_rect))
            } else {
                None
            }
        })
        .expect("Stop must be painted during the transition");
    output.drop_without_applying_deltas();
    (request, icon, clip)
}

#[test]
fn animated_toolbar_accepts_stop_only_in_its_visible_enabled_part() {
    use crate::gameplay_toolbar::GameplayToolbarRequest;
    use egui::Rect;
    use std::time::Duration;

    for initially_fullscreen in [false, true] {
        for enabled in [true, false] {
            let (_scratch, mut app, ctx) = gameplay_fixture();
            let viewport = Rect::from_min_size(Pos2::ZERO, egui::vec2(453.0, 960.0));
            let now = Instant::now();
            app.gameplay_transition
                .begin(viewport, initially_fullscreen, false, now);
            app.gameplay_transition.request();
            app.gameplay_transition
                .begin(viewport, !initially_fullscreen, false, now);
            app.gameplay_transition.begin(
                viewport,
                !initially_fullscreen,
                false,
                now + Duration::from_millis(120),
            );
            assert!((app.gameplay_transition.toolbar() - 0.5).abs() < 0.001);
            let paint = |events| toolbar_frame(&app, &ctx, viewport, enabled, events);
            paint(vec![]);
            let (_, icon, clip) = paint(vec![]);
            let hidden = Pos2::new(icon.center().x, icon.top() + 1.0);
            let visible = icon.intersect(clip).center();
            assert!(viewport.contains(hidden) && !clip.contains(hidden));
            assert!(icon.contains(visible) && clip.contains(visible));
            for (position, expected) in [
                (hidden, None),
                (visible, enabled.then_some(GameplayToolbarRequest::Stop)),
            ] {
                for pressed in [true, false] {
                    let (request, _, _) = paint(vec![
                        Event::PointerMoved(position),
                        Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]);
                    assert_eq!(
                        request,
                        if pressed { None } else { expected },
                        "initially_fullscreen={initially_fullscreen}, enabled={enabled}, position={position:?}"
                    );
                }
            }
            // A pointer outside the clip must not block keyboard activation
            // of the focused Stop button.
            if enabled {
                for pressed in [true, false] {
                    paint(vec![Event::Key {
                        key: Key::Tab,
                        physical_key: Some(Key::Tab),
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::SHIFT,
                    }]);
                }
                let focused = ctx.memory(egui::Memory::focused).unwrap();
                assert!(ctx.read_response(focused).unwrap().rect.contains(visible));
            }
            let (request, _, _) = paint(vec![
                Event::PointerMoved(hidden),
                Event::Key {
                    key: Key::Space,
                    physical_key: Some(Key::Space),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            assert_eq!(request, enabled.then_some(GameplayToolbarRequest::Stop));
        }
    }
}
