use super::{
    Category, EmuError, Event, FrontendApp, HostAction, ImeEvent, InputEvent, Key, KeyState,
    MAX_TEXT_INPUT_BYTES, MAX_UI_TECHNICAL_ERROR_BYTES, PointerEvent, PointerPhase, Pos2, Screen,
    SessionState, TouchOwner, TouchPhase, bounded_ui_text, egui,
};

mod keyboard;
mod launch;

impl FrontendApp {
    pub(super) fn stop_active_session(&mut self) {
        self.platform.cancel_guest_operations();
        self.gameplay_transition = super::gameplay_transition::GameplayTransition::default();
        self.release_all_input();
        if self.session.state() == SessionState::Failed {
            let _ = self.session.discard_failed();
            self.finish_stopped_session();
            return;
        }
        if let Ok(command) = self.session.stop()
            && let Err(error) = self.runtime.submit(command)
        {
            self.show_error("Could not stop game", &error);
        }
        self.touch_owners.clear();
        self.control_regions.clear();
        self.stick_region = None;
        self.canvas_rect = None;
        self.host_request = None;
    }

    pub(super) fn toggle_runtime_debug(&mut self) {
        let enabled = !self.runtime_debug.enabled;
        if let Err(error) = self.runtime.telemetry().set_enabled(enabled) {
            self.show_error("Could not change debug telemetry", &error);
            return;
        }
        self.runtime_debug.set_enabled(enabled);
    }

    pub(super) fn rotate_gameplay_screen(&mut self, landscape: bool) {
        use super::PlatformOrientation::{Automatic, Landscape, Portrait};

        if self.platform.orientation_control() != super::OrientationControl::HostWindow
            || !matches!(self.screen, Screen::Gameplay(_))
        {
            return;
        }
        // Keep rapid taps ordered even before Android reports the new viewport.
        let orientation = match self.host_orientation {
            Landscape => Portrait,
            Automatic if landscape => Portrait,
            Portrait | Automatic => Landscape,
        };
        self.clear_gameplay_input_geometry();
        self.fullscreen_exit_gesture.reset();
        match self.platform.set_orientation(orientation) {
            Ok(()) => {
                self.host_orientation = orientation;
                self.gameplay_transition.request();
            }
            Err(error) => self.show_error("Could not rotate screen", &error),
        }
    }

    pub(super) fn restore_system_orientation(&mut self) -> Result<(), EmuError> {
        if self.host_orientation != super::PlatformOrientation::Automatic {
            if self.platform.orientation_control() == super::OrientationControl::HostWindow {
                self.platform
                    .set_orientation(super::PlatformOrientation::Automatic)?;
            }
            self.host_orientation = super::PlatformOrientation::Automatic;
        }
        Ok(())
    }

    pub(super) fn toggle_fast_forward(&mut self) {
        let Some(enabled) = (match &self.screen {
            Screen::Gameplay(gameplay) => Some(!gameplay.fast_forward),
            Screen::Library | Screen::AppSettings(_) | Screen::Settings(_) => None,
        }) else {
            return;
        };
        let command = match self.session.set_fast_forward(enabled) {
            Ok(command) => command,
            Err(error) => {
                self.show_error("Could not change fast-forward", &error);
                return;
            }
        };
        if let Err(error) = self.runtime.submit(command) {
            self.show_error("Could not change fast-forward", &error);
            return;
        }
        if let Screen::Gameplay(gameplay) = &mut self.screen {
            gameplay.fast_forward = enabled;
        }
    }

    pub(super) fn release_all_input(&mut self) {
        self.slider_repeat = None;
        self.navigation_repeat = None;
        if let Some(editor) = self.screen.control_editor_mut() {
            editor.navigation.moving = None;
            editor.drag_origin = None;
        }
        // Consume the native backlog as held-state updates, so pre-navigation
        // presses cannot enter a new session or a newly opened capture dialog.
        for _ in 0..256 {
            let Some(event) = self.platform.poll_physical_input() else {
                break;
            };
            self.physical_input
                .update(event, self.physical_bindings.dead_zone_percent);
        }
        self.physical_input.release_actions();
        if let Some(editor) = &mut self.physical_editor {
            editor.cancel_capture();
        }
        self.ime_composition = None;
        if let Ok(command) = self.session.release_all_input()
            && let Err(error) = self.runtime.submit(command)
        {
            if self.runtime_diagnostics.len() == 8 {
                self.runtime_diagnostics.pop_front();
            }
            self.runtime_diagnostics.push_back(bounded_ui_text(
                &format!("input-release: {}", error.message()),
                MAX_UI_TECHNICAL_ERROR_BYTES,
            ));
        }
        self.touch_owners.clear();
    }

    pub(super) fn clear_gameplay_input_geometry(&mut self) {
        self.release_all_input();
        self.canvas_rect = None;
        self.control_regions.clear();
        self.stick_region = None;
    }

    pub(super) fn process_game_input(&mut self, ctx: &egui::Context) {
        if !matches!(self.screen, Screen::Gameplay(_))
            || self.session.state() != SessionState::Running
            || self.overlay_active()
        {
            return;
        }
        if ctx.input(|input| input.raw.events.contains(&Event::WindowFocused(false))) {
            // Physical input processing consumes held-state edges and releases
            // all sources. No game event may precede that batch-wide barrier.
            return;
        }
        let events =
            match ctx.input(|input| game_input_events(&input.raw.events, self.text_input_active)) {
                Ok(events) => events,
                Err(error) => {
                    self.show_error("Text input failed", &error);
                    return;
                }
            };
        let mut delete_backward = 0_usize;
        // winit emits pointer events alongside touch. Only the real mouse gets
        // a synthetic touch owner; it uses the same inverse Canvas transform.
        let mouse = !events
            .iter()
            .any(|event| matches!(event, Event::Touch { .. }))
            && self.touch_owners.keys().all(|id| *id == u64::MAX);
        for event in events {
            if self.overlay_active()
                || self.session.state() != SessionState::Running
                || !matches!(self.screen, Screen::Gameplay(_))
            {
                return;
            }
            match event {
                event @ Event::Key { .. } => {
                    self.process_game_key(ctx, &event, &mut delete_backward);
                }
                Event::Touch { id, phase, pos, .. } => {
                    self.process_touch(id.0, phase, pos);
                }
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } if mouse => {
                    self.process_touch(
                        u64::MAX,
                        if pressed {
                            TouchPhase::Start
                        } else {
                            TouchPhase::End
                        },
                        pos,
                    );
                }
                Event::PointerMoved(pos) if mouse => {
                    self.process_touch(u64::MAX, TouchPhase::Move, pos);
                }
                Event::PointerCancelled | Event::PointerGone if mouse => {
                    self.process_touch(u64::MAX, TouchPhase::Cancel, Pos2::new(-1.0, -1.0));
                }
                Event::Ime(event) if self.text_input_active => {
                    self.process_ime_event(event, &mut delete_backward);
                }
                Event::Text(text) if self.text_input_active => {
                    if text.is_empty() {
                        self.set_ime_composition(None);
                    } else {
                        self.ime_composition = None;
                        self.submit_input(InputEvent::TextCommit { text });
                    }
                }
                _ => {}
            }
        }
    }

    fn process_ime_event(&mut self, event: ImeEvent, deleted: &mut usize) {
        match event {
            ImeEvent::DeleteSurrounding {
                before_chars,
                after_chars,
            } => {
                self.submit_text_deletions(before_chars, after_chars, deleted);
            }
            ImeEvent::Preedit {
                text,
                active_range_chars,
            } => {
                if text.is_empty() {
                    self.set_ime_composition(None);
                } else {
                    let range = active_range_chars.unwrap_or_else(|| {
                        let end = text.chars().count();
                        end..end
                    });
                    let selection_start = text.chars().take(range.start).map(char::len_utf16).sum();
                    let selection_end = text.chars().take(range.end).map(char::len_utf16).sum();
                    self.set_ime_composition(Some(super::ImeComposition {
                        text,
                        selection_start,
                        selection_end,
                    }));
                }
            }
            _ => {}
        }
    }

    pub(super) fn overlay_active(&self) -> bool {
        self.display_error.is_some()
            || self.exit_confirmation
            || self.host_request.is_some()
            || self.heap_recovery.is_some()
            || self.data_action.is_some()
            || self.library_folders.dialog.is_some()
            || self.import_flow.is_some()
            || self.platform.document_browser().is_some()
            || self.fullscreen_help_visible()
            || self.control_editor_preparing()
    }

    pub(super) fn submit_text_deletions(&mut self, before: usize, after: usize, used: &mut usize) {
        for (requested, event) in [
            (before, InputEvent::DeleteBackward),
            (after, InputEvent::DeleteForward),
        ] {
            let count = requested.min(64_usize.saturating_sub(*used));
            *used += count;
            for _ in 0..count {
                self.submit_input(event.clone());
            }
        }
    }

    pub(super) fn process_touch(&mut self, touch_id: u64, phase: TouchPhase, position: Pos2) {
        match phase {
            TouchPhase::Start => {
                if self.touch_owners.contains_key(&touch_id) {
                    return;
                }
                if let Some(owner) = self.action_at(position) {
                    self.move_virtual_control(touch_id, owner);
                } else if let Some((bounds, _)) = self.stick_region
                    && super::controls::stick::contains(bounds, position)
                {
                    if !self
                        .touch_owners
                        .values()
                        .any(|owner| matches!(owner, TouchOwner::VirtualStick { .. }))
                    {
                        self.move_virtual_stick(touch_id, position, None);
                    }
                } else if self.game_accepts_pointer()
                    && let Some((canvas_x, canvas_y)) = self.canvas_position(position)
                {
                    self.touch_owners.insert(
                        touch_id,
                        TouchOwner::Canvas {
                            last_x: canvas_x,
                            last_y: canvas_y,
                        },
                    );
                    self.submit_input(InputEvent::Pointer(PointerEvent {
                        touch_id,
                        phase: PointerPhase::Pressed,
                        canvas_x,
                        canvas_y,
                    }));
                }
            }
            TouchPhase::Move => {
                let Some(owner) = self.touch_owners.get(&touch_id).copied() else {
                    return;
                };
                if matches!(
                    owner,
                    TouchOwner::VirtualKey(_) | TouchOwner::VirtualKeyPair(..)
                ) {
                    self.move_virtual_control(
                        touch_id,
                        self.action_at(position)
                            .unwrap_or(TouchOwner::VirtualKey(None)),
                    );
                    return;
                }
                if let TouchOwner::VirtualStick { action, .. } = owner {
                    self.move_virtual_stick(touch_id, position, action);
                    return;
                }
                if let TouchOwner::Canvas { .. } = owner
                    && let Some((canvas_x, canvas_y)) = self.canvas_position(position)
                {
                    self.touch_owners.insert(
                        touch_id,
                        TouchOwner::Canvas {
                            last_x: canvas_x,
                            last_y: canvas_y,
                        },
                    );
                    self.submit_input(InputEvent::Pointer(PointerEvent {
                        touch_id,
                        phase: PointerPhase::Dragged,
                        canvas_x,
                        canvas_y,
                    }));
                }
            }
            TouchPhase::End | TouchPhase::Cancel => {
                let Some(owner) = self.touch_owners.get(&touch_id).copied() else {
                    return;
                };
                match owner {
                    TouchOwner::VirtualKey(_)
                    | TouchOwner::VirtualKeyPair(..)
                    | TouchOwner::VirtualStick { .. } => {
                        self.move_virtual_key(touch_id, None);
                    }
                    TouchOwner::Canvas { last_x, last_y } => {
                        let (canvas_x, canvas_y) =
                            self.canvas_position(position).unwrap_or((last_x, last_y));
                        self.submit_input(InputEvent::Pointer(PointerEvent {
                            touch_id,
                            phase: if phase == TouchPhase::Cancel {
                                PointerPhase::Cancelled
                            } else {
                                PointerPhase::Released
                            },
                            canvas_x,
                            canvas_y,
                        }));
                    }
                }
                self.touch_owners.remove(&touch_id);
            }
        }
    }

    fn move_virtual_key(&mut self, touch_id: u64, action: Option<HostAction>) {
        self.move_virtual_control(touch_id, TouchOwner::VirtualKey(action));
    }

    fn move_virtual_stick(&mut self, touch_id: u64, position: Pos2, previous: Option<HostAction>) {
        let owner = self.stick_region.map_or(
            TouchOwner::VirtualStick {
                action: None,
                offset: egui::Vec2::ZERO,
                two_key_diagonals: false,
            },
            |(bounds, two_key_diagonals)| {
                super::controls::stick::touch_owner(bounds, position, previous, two_key_diagonals)
            },
        );
        self.move_virtual_control(touch_id, owner);
    }

    fn move_virtual_control(&mut self, touch_id: u64, owner: TouchOwner) {
        let events = update_virtual_control(&mut self.touch_owners, touch_id, owner);
        // A diagonal is one touch gesture, even when it presses two keys.
        if events.iter().flatten().any(|event| {
            matches!(
                event,
                InputEvent::Key {
                    state: KeyState::Pressed,
                    ..
                }
            )
        }) && let Screen::Gameplay(gameplay) = &self.screen
            && gameplay.vibration.enabled
        {
            self.request_game_haptic(gameplay.vibration.strength_percent);
        }
        for event in events.into_iter().flatten() {
            if let InputEvent::Key { action, .. } = &event
                && self.physical_input.action_active(
                    frontend_core::physical_input::PhysicalAction::Phone(*action),
                )
            {
                continue;
            }
            self.submit_input(event);
        }
    }

    pub(super) fn submit_input(&mut self, event: InputEvent) {
        if self.overlay_active() {
            return;
        }
        let command = match self.session.input(event) {
            Ok(command) => command,
            Err(error) if error.code() == "session-text-input-limit" => {
                self.show_error("Text input failed", &error);
                return;
            }
            Err(_) => return,
        };
        #[cfg(test)]
        if let Some(observed) = &mut self.observed_input
            && let frontend_core::SessionCommandKind::Input(event) = &command.kind
        {
            observed.push(event.clone());
        }
        if let Err(error) = self.runtime.submit(command) {
            self.show_error("Input failed", &error);
        }
    }

    pub(super) fn game_accepts_pointer(&self) -> bool {
        matches!(&self.screen, Screen::Gameplay(gameplay) if gameplay.pointer_events)
    }

    pub(super) fn action_at(&self, position: Pos2) -> Option<TouchOwner> {
        self.control_regions
            .iter()
            .rev()
            .find_map(|(rect, action)| rect.contains(position).then_some(*action))
    }

    // Runtime/profile limits keep framebuffer dimensions and transformed touch
    // coordinates far below the precision and range boundaries of these casts.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub(super) fn canvas_position(&self, position: Pos2) -> Option<(i32, i32)> {
        let rect = self.canvas_rect?;
        let frame = self.latest_frame.as_ref()?;
        if !rect.contains(position) || rect.width() <= 0.0 || rect.height() <= 0.0 {
            return None;
        }
        let logical_x = ((position.x - rect.left()) * frame.width as f32 / rect.width()).floor();
        let logical_y = ((position.y - rect.top()) * frame.height as f32 / rect.height()).floor();
        frame.canvas_point(logical_x as i32, logical_y as i32)
    }
}

fn update_virtual_control(
    owners: &mut std::collections::HashMap<u64, TouchOwner>,
    touch_id: u64,
    owner: TouchOwner,
) -> [Option<InputEvent>; 4] {
    let actions = owner.virtual_actions();
    let previous = owners
        .insert(touch_id, owner)
        .map_or([None, None], TouchOwner::virtual_actions);
    if previous == actions {
        return [None, None, None, None];
    }
    let held_elsewhere = |key| {
        owners
            .iter()
            .any(|(id, owner)| *id != touch_id && owner.virtual_actions().contains(&Some(key)))
    };
    // Keep shared components held across direction changes. Release old keys
    // before pressing new ones, respecting every other finger's actual keys.
    std::array::from_fn(|index| {
        let (key, other, state) = if index < 2 {
            (previous[index], actions, KeyState::Released)
        } else {
            (actions[index - 2], previous, KeyState::Pressed)
        };
        key.filter(|key| !other.contains(&Some(*key)) && !held_elsewhere(*key))
            .map(|action| InputEvent::Key { action, state })
    })
}

pub(super) fn physical_key_event(
    owners: &std::collections::HashMap<u64, TouchOwner>,
    action: HostAction,
    pressed: bool,
) -> Option<InputEvent> {
    // Physical sources and fingers share one guest key. Either kind can keep
    // it held after the other releases, without sending a second press.
    (!owners
        .values()
        .any(|owner| owner.virtual_actions().contains(&Some(action))))
    .then_some(InputEvent::Key {
        action,
        state: if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        },
    })
}

fn game_input_events(events: &[Event], text_input_active: bool) -> Result<Vec<Event>, EmuError> {
    let mut output = Vec::new();
    let mut previous_commit = None;
    let mut text_bytes = 0_usize;
    for event in events {
        let committed = matches!(event, Event::Ime(ImeEvent::Commit(_)));
        let text = match event {
            Event::Text(text) | Event::Ime(ImeEvent::Commit(text)) if text_input_active => text,
            other => {
                previous_commit = None;
                // Snapshot only events gameplay consumes. In particular, a
                // clipboard/IME payload must not be copied when text input is off.
                if matches!(
                    other,
                    Event::Key { .. }
                        | Event::Touch { .. }
                        | Event::PointerButton {
                            button: egui::PointerButton::Primary,
                            ..
                        }
                        | Event::PointerMoved(_)
                        | Event::PointerGone
                        | Event::PointerCancelled
                ) || text_input_active
                    && matches!(
                        other,
                        Event::Ime(ImeEvent::Preedit { .. } | ImeEvent::DeleteSurrounding { .. })
                    )
                {
                    if let Event::Ime(ImeEvent::Preedit { text, .. }) = other
                        && text.len() > MAX_TEXT_INPUT_BYTES
                    {
                        return Err(EmuError::new(
                            Category::Platform,
                            "ime-composition-limit",
                            "IME composition exceeds 4096 bytes",
                        ));
                    }
                    output.push(other.clone());
                }
                continue;
            }
        };
        // Some hosts report the same edit as both IME Commit and Text. Only
        // an adjacent matching pair is redundant; other text must keep its
        // position relative to deletes, keys, and independent commits.
        if previous_commit == Some(!committed)
            && matches!(output.last(), Some(Event::Text(previous)) if previous == text)
        {
            previous_commit = None;
            continue;
        }
        text_bytes = text_bytes.saturating_add(text.len());
        if text_bytes > MAX_TEXT_INPUT_BYTES {
            return Err(EmuError::new(
                Category::Platform,
                "session-text-input-limit",
                format!("committed text exceeds {MAX_TEXT_INPUT_BYTES} bytes"),
            ));
        }
        previous_commit = Some(committed);
        output.push(Event::Text(text.clone()));
    }
    Ok(output)
}

#[cfg(test)]
#[path = "../../../tests/unit/frontend-ui/session_input/mod.rs"]
mod tests;
