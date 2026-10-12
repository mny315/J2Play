use super::*;
use crate::PlatformBridge;
use crate::tests::test_storage::Scratch;
use eframe::App;
use frontend_core::physical_input::{GamepadButton, PhysicalControl, PhysicalInputEvent};
use std::collections::{HashSet, VecDeque};

mod feedback;

struct Desktop;

impl PlatformBridge for Desktop {
    fn orientation_control(&self) -> crate::OrientationControl {
        crate::OrientationControl::Layout
    }

    fn request_document(&mut self, kind: crate::DocumentKind) -> Result<(), crate::EmuError> {
        crate::UnavailablePlatformBridge.request_document(kind)
    }

    fn poll_document(&mut self) -> Option<Result<crate::DocumentOutcome, crate::EmuError>> {
        None
    }
}

struct Editor {
    app: FrontendApp,
    ctx: egui::Context,
    size: egui::Vec2,
    _scratch: Scratch,
}

impl Editor {
    fn new(landscape: bool) -> Self {
        let scratch = Scratch::new();
        let mut app = FrontendApp::new(&scratch.0, Box::new(Desktop)).unwrap();
        app.startup_splash = None;
        app.focus_ring.navigate();
        app.screen = Screen::AppSettings(app.app_settings.clone().into());
        app.open_control_editor();
        app.screen
            .control_editor_mut()
            .unwrap()
            .select_orientation(landscape);
        let ctx = egui::Context::default();
        crate::apply_material_theme(&ctx, &app.material_theme);
        let mut style = (*ctx.global_style()).clone();
        style.scroll_animation = egui::style::ScrollAnimation::none();
        ctx.set_global_style(style);
        let mut editor = Self {
            app,
            ctx,
            size: if landscape {
                egui::vec2(1280.0, 1400.0)
            } else {
                egui::vec2(453.0, 700.0)
            },
            _scratch: scratch,
        };
        editor.settle();
        editor
    }

    fn state(&mut self) -> &mut ControlEditorState {
        self.app.screen.control_editor_mut().unwrap()
    }

    fn focused(&self) -> egui::Response {
        self.ctx
            .read_response(self.ctx.memory(egui::Memory::focused).unwrap())
            .unwrap()
    }

    fn paint(&mut self, events: &[PhysicalInputEvent], keys: Vec<egui::Event>) -> egui::FullOutput {
        let mut frame = eframe::Frame::_new_kittest();
        let mut pending = Some(events);
        self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events: keys,
                ..Default::default()
            },
            |ui| {
                if let Some(events) = pending.take() {
                    self.app.process_physical_inputs(&self.ctx, true, false);
                    for &event in events {
                        self.app.process_physical_event(&self.ctx, event, false);
                    }
                }
                self.app.ui(ui, &mut frame);
            },
        )
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.paint(&[], Vec::new()).drop_without_applying_deltas();
        }
    }

    fn tap(&mut self, button: GamepadButton) {
        let events = [true, false].map(|pressed| PhysicalInputEvent::Button {
            device: 1,
            control: PhysicalControl::Gamepad { button },
            pressed,
        });
        self.paint(&events, Vec::new())
            .drop_without_applying_deltas();
        self.settle();
    }

    fn key(&mut self, key: egui::Key) {
        let events = [true, false].map(|pressed| egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        self.paint(&[], events.to_vec())
            .drop_without_applying_deltas();
        self.settle();
    }

    fn focus(&mut self, id: egui::Id) {
        self.ctx.memory_mut(|memory| memory.request_focus(id));
        self.settle();
    }

    // Traverse the actual D-pad graph, including the scroll area and footer.
    fn inventory(&mut self) -> Vec<(egui::Id, Option<crate::ControlSelection>, Vec<String>)> {
        let initial = self.focused().id;
        let mut pending = VecDeque::from([initial]);
        let mut seen = HashSet::from([initial]);
        let mut found = Vec::new();
        while let Some(id) = pending.pop_front() {
            assert!(seen.len() <= 64, "navigation must have a bounded inventory");
            self.focus(id);
            let output = self.paint(&[], Vec::new());
            let response = self.focused();
            let selection =
                (self.state().navigation.focused == Some(id)).then(|| self.state().selected);
            let labels = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if response.rect.contains(text.pos) => {
                        Some(text.galley.text().to_owned())
                    }
                    _ => None,
                })
                .collect();
            if selection.is_some() {
                assert!(
                    response.interact_rect.height() > 5.0,
                    "focused control must be visible"
                );
                assert!(
                    output.shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Rect(rect) if (rect.stroke.width - 2.5).abs() < f32::EPSILON
                    )),
                    "virtual controls need the shared focus ring"
                );
            }
            found.push((id, selection, labels));
            output.drop_without_applying_deltas();
            for direction in [
                GamepadButton::DpadUp,
                GamepadButton::DpadDown,
                GamepadButton::DpadLeft,
                GamepadButton::DpadRight,
            ] {
                self.focus(id);
                self.tap(direction);
                let next = self.focused().id;
                if seen.insert(next) {
                    pending.push_back(next);
                }
            }
        }
        found
    }

    fn focus_number_one(&mut self) -> egui::Id {
        let id = self
            .inventory()
            .into_iter()
            .find(|(_, selected, _)| *selected == Some(crate::ControlSelection::NumberKey(0)))
            .unwrap()
            .0;
        self.focus(id);
        id
    }
}

#[test]
fn controller_reaches_every_virtual_control_properties_and_footer_in_both_layouts() {
    for (landscape, phone) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut editor = Editor::new(landscape);
        if phone {
            editor.app.platform = Box::new(crate::UnavailablePlatformBridge);
            editor.size = if landscape {
                egui::vec2(960.0, 453.0)
            } else {
                egui::vec2(453.0, 960.0)
            };
            editor.settle();
        }
        editor.state().layout.number_keys[3].visible = false;
        let inventory = editor.inventory();
        for selection in [
            crate::ControlSelection::DirectionPad,
            crate::ControlSelection::Fire,
            crate::ControlSelection::LeftSoftKey,
            crate::ControlSelection::RightSoftKey,
        ]
        .into_iter()
        .chain((0..12).map(crate::ControlSelection::NumberKey))
        {
            assert!(
                inventory
                    .iter()
                    .any(|(_, selected, _)| *selected == Some(selection)),
                "{landscape}: unreachable {selection:?}"
            );
        }
        for label in [
            "Grid & snap: On",
            "Hide selected",
            "Reset",
            "Cancel",
            "Save",
        ] {
            assert!(
                inventory
                    .iter()
                    .any(|(_, _, labels)| labels.iter().any(|text| {
                        text == label || (label == "Hide selected" && text == "Show selected")
                    })),
                "{landscape}: unreachable {label}"
            );
        }
        // Down from the grid row must reach the existing size adjustment.
        let grid = inventory
            .iter()
            .find(|(_, _, labels)| labels.iter().any(|text| text == "Grid & snap: On"))
            .unwrap()
            .0;
        editor.focus(grid);
        let selected = editor.state().selected;
        let before = selected_transform(&editor.state().layout, selected).size_percent;
        editor.tap(GamepadButton::DpadDown);
        let below = editor.focused();
        editor.tap(GamepadButton::DpadRight);
        assert_eq!(
            selected_transform(&editor.state().layout, selected).size_percent,
            before + 1,
            "landscape={landscape}, phone={phone}, down={below:?}, right={:?}",
            editor.focused()
        );
    }
}

#[test]
fn pointer_drag_still_moves_the_selected_control_without_entering_controller_mode() {
    for landscape in [false, true] {
        let mut editor = Editor::new(landscape);
        editor.focus_number_one();
        editor.state().grid_enabled = false;
        let origin = editor.focused().rect.center();
        let before = editor.state().layout.number_keys[0];
        editor
            .paint(
                &[],
                vec![
                    egui::Event::PointerMoved(origin),
                    egui::Event::PointerButton {
                        pos: origin,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            )
            .drop_without_applying_deltas();
        for distance in [8.0, 16.0, 24.0] {
            editor
                .paint(
                    &[],
                    vec![egui::Event::PointerMoved(
                        origin - egui::vec2(0.0, distance),
                    )],
                )
                .drop_without_applying_deltas();
        }
        editor
            .paint(
                &[],
                vec![egui::Event::PointerButton {
                    pos: origin - egui::vec2(0.0, 24.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            )
            .drop_without_applying_deltas();
        editor.settle();
        assert!(editor.state().layout.number_keys[0].offset_y < before.offset_y);
        assert!(editor.state().navigation.moving.is_none());
        assert!(editor.state().drag_origin.is_none());
    }
}

#[test]
fn controller_movement_keeps_focus_and_back_finishes_before_cancelling_the_editor() {
    for landscape in [false, true] {
        let mut editor = Editor::new(landscape);
        let id = editor.focus_number_one();
        let before = editor.state().layout.number_keys[0];
        editor.tap(GamepadButton::South);
        assert_eq!(editor.state().navigation.moving, Some(id));
        editor.tap(GamepadButton::DpadUp);
        let moved = editor.state().layout.number_keys[0];
        assert!(moved.offset_y < before.offset_y);
        assert_eq!(moved.offset_x, before.offset_x);
        assert_eq!(editor.focused().id, id);
        editor.tap(GamepadButton::East);
        assert!(editor.state().navigation.moving.is_none());
        assert_eq!(editor.state().layout.number_keys[0], moved);
        editor.tap(GamepadButton::DpadUp);
        assert_ne!(
            editor.focused().id,
            id,
            "arrows must navigate again after finishing"
        );
        editor.tap(GamepadButton::East);
        assert!(editor.app.screen.control_editor_mut().is_none());
        let Screen::AppSettings(settings) = &editor.app.screen else {
            panic!("parent settings must remain open")
        };
        assert!(settings.draft.control_layout.is_default());
        assert!(settings.draft.landscape_control_layout.is_default());
    }
}

#[test]
fn keyboard_movement_stops_on_escape_disconnect_focus_loss_and_touch() {
    let mut editor = Editor::new(true);
    let id = editor.focus_number_one();
    editor.key(egui::Key::Enter);
    assert_eq!(editor.state().navigation.moving, Some(id));
    editor.key(egui::Key::ArrowUp);
    assert!(editor.state().layout.number_keys[0].offset_y < 0);
    assert_eq!(editor.focused().id, id);
    editor.key(egui::Key::Escape);
    assert!(editor.state().navigation.moving.is_none());
    editor.key(egui::Key::ArrowRight);
    assert_ne!(
        editor.focused().id,
        id,
        "keyboard arrows must also select drag targets"
    );

    for interruption in 0..3 {
        editor.focus(id);
        editor.tap(GamepadButton::South);
        assert!(editor.state().navigation.moving.is_some());
        let before = editor.state().layout.clone();
        let center = editor.focused().rect.center();
        let (physical, keys) = match interruption {
            0 => (
                vec![PhysicalInputEvent::Disconnected { device: 1 }],
                Vec::new(),
            ),
            1 => (Vec::new(), vec![egui::Event::WindowFocused(false)]),
            _ => (
                Vec::new(),
                vec![
                    egui::Event::PointerMoved(center),
                    egui::Event::PointerButton {
                        pos: center,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: center,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            ),
        };
        editor.paint(&physical, keys).drop_without_applying_deltas();
        editor.settle();
        assert!(editor.state().navigation.moving.is_none());
        assert_eq!(editor.state().layout, before);
    }
}

#[test]
fn controller_movement_respects_canvas_protection_and_save_stays_transactional() {
    let mut editor = Editor::new(true);
    let id = editor.focus_number_one();
    editor.tap(GamepadButton::South);
    for _ in 0..24 {
        editor.tap(GamepadButton::DpadRight);
    }
    let blocked = editor.state().layout.number_keys[0];
    editor.tap(GamepadButton::DpadRight);
    assert_eq!(editor.state().layout.number_keys[0], blocked);
    assert_eq!(editor.focused().id, id);
    editor.tap(GamepadButton::DpadUp);
    editor.tap(GamepadButton::East);
    let layout = editor.state().layout.clone();
    assert!(!layout.is_default());
    let saved = editor.app.repository.load_app_settings().unwrap();
    editor.app.screen.commit_control_editor();
    assert_eq!(editor.app.repository.load_app_settings().unwrap(), saved);
    editor.app.save_settings_screen();
    assert_eq!(
        editor
            .app
            .repository
            .load_app_settings()
            .unwrap()
            .landscape_control_layout,
        layout
    );
}
