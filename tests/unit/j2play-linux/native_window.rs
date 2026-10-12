use super::*;
use crate::test_storage::Scratch;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Opt-in real-window fixture check. All JAR bytes belong to the project; it
/// never reads the user's library or runs a commercial game.
#[test]
#[ignore = "requires a graphical Linux session; run separately for Wayland and X11"]
fn native_window_renders_settings_fullscreen_and_closes() {
    let scratch = Scratch::new();
    let paths = AppPaths {
        data: scratch.0.join("data"),
        cache: scratch.0.join("cache"),
    };
    // This fixture locates visible English labels, regardless of host locale.
    let repository = paths.repository(paths.acquire().unwrap()).unwrap();
    repository
        .save_app_settings(&frontend_core::AppSettings {
            language: Some(frontend_core::Language::English),
            ..Default::default()
        })
        .unwrap();
    drop(repository);
    if std::env::var_os("J2PLAY_LINUX_FIXTURE_LIBRARY").is_some() {
        let guard = paths.acquire().unwrap();
        let repository = paths.repository(guard).unwrap();
        let prepared = frontend_core::inspect_import(frontend_core::ImportSource::new(
            None,
            include_bytes!("../../fixtures/java-me/conformance.jar").to_vec(),
            None,
        ))
        .unwrap()
        .select_midlet(1)
        .unwrap();
        repository
            .commit_import(&prepared, frontend_core::GameSettings::default())
            .unwrap();
    }
    let observed = Arc::new(Mutex::new(Observed::default()));
    let result = observed.clone();
    let mut options = native_options();
    options.event_loop_builder = Some(Box::new(|builder| {
        winit::platform::wayland::EventLoopBuilderExtWayland::with_any_thread(builder, true);
        winit::platform::x11::EventLoopBuilderExtX11::with_any_thread(builder, true);
    }));
    let artifacts = std::env::var_os("J2PLAY_LINUX_CHECK_DIR").map(PathBuf::from);
    let started = Instant::now();
    eframe::run_native(
        "J2Play fixture check",
        options,
        Box::new(move |creation| {
            eprintln!("Linux fixture window/GPU ready: {:?}", started.elapsed());
            eprintln!(
                "Linux fixture graphics: {:?}",
                creation
                    .wgpu_render_state
                    .as_ref()
                    .unwrap()
                    .adapter
                    .get_info()
            );
            Ok(Box::new(NativeCheck {
                app: LinuxApp::new(&creation.egui_ctx, &paths)?,
                observed,
                started,
                interaction_started: None,
                phase_started: Instant::now(),
                phase: 0,
                artifacts,
                clicks: VecDeque::new(),
                click_injected: false,
            }))
        }),
    )
    .unwrap();
    let result = result.lock().unwrap();
    assert!(
        result.paints > 4,
        "the native surface never rendered the shared UI"
    );
    assert!(result.exited);
    assert!(
        result.fullscreen_seen,
        "the compositor never entered fullscreen"
    );
    assert!(result.closed_at.unwrap().elapsed() < Duration::from_secs(4));
}

#[derive(Default)]
struct Observed {
    paints: usize,
    exited: bool,
    closed_at: Option<Instant>,
    fullscreen_seen: bool,
}

struct NativeCheck {
    app: LinuxApp,
    observed: Arc<Mutex<Observed>>,
    started: Instant,
    interaction_started: Option<Instant>,
    phase_started: Instant,
    phase: u8,
    artifacts: Option<PathBuf>,
    clicks: VecDeque<(egui::Pos2, Option<bool>)>,
    click_injected: bool,
}

impl App for NativeCheck {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.observed.lock().unwrap().fullscreen_seen |= input.viewport().fullscreen == Some(true);
        for event in &input.events {
            if let egui::Event::Screenshot {
                image, user_data, ..
            } = event
                && let Some(directory) = &self.artifacts
            {
                std::fs::create_dir_all(directory).unwrap();
                let name = user_data
                    .data
                    .as_ref()
                    .unwrap()
                    .downcast_ref::<String>()
                    .unwrap();
                image::save_buffer(
                    directory.join(format!("{name}.png")),
                    &image
                        .pixels
                        .iter()
                        .flat_map(egui::Color32::to_array)
                        .collect::<Vec<_>>(),
                    u32::try_from(image.size[0]).unwrap(),
                    u32::try_from(image.size[1]).unwrap(),
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
        if !self.click_injected
            && let Some(&(pos, pressed)) = self.clicks.front()
        {
            input.events.push(egui::Event::PointerMoved(pos));
            if let Some(pressed) = pressed {
                input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            self.click_injected = true;
        }
        if self.phase == 7 {
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .events
                .push(egui::ViewportEvent::Close);
            self.observed
                .lock()
                .unwrap()
                .closed_at
                .get_or_insert_with(Instant::now);
            self.phase = 8;
        }
        self.app.raw_input_hook(ctx, input);
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        ctx.request_repaint_after(Duration::from_millis(20));
        let observed = self.observed.lock().unwrap();
        let (since, timeout) = if let Some(closed_at) = observed.closed_at {
            (closed_at, Duration::from_secs(4))
        } else {
            (
                self.interaction_started.unwrap_or(self.started),
                Duration::from_secs(15),
            )
        };
        assert!(
            since.elapsed() < timeout,
            "native window fixture timed out at phase {} ({}); paints={}, focused={}",
            self.phase,
            phase_name(self.phase),
            observed.paints,
            ctx.input(|input| input.focused),
        );
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if self.observed.lock().unwrap().paints == 0 {
            eprintln!("Linux fixture first UI paint: {:?}", self.started.elapsed());
            assert!(
                self.started.elapsed() < Duration::from_secs(5),
                "window startup exceeded five seconds"
            );
        }
        self.app.ui(ui, frame);
        if std::mem::take(&mut self.click_injected) {
            self.clicks.pop_front();
        }
        self.observed.lock().unwrap().paints += 1;
        if self.phase_started.elapsed() < Duration::from_millis(800)
            || self.phase >= 7
            || !self.clicks.is_empty()
        {
            return;
        }
        let ctx = ui.ctx();
        match self.phase {
            0 => {
                if !ctx.input(|input| input.focused) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    return;
                }
                let ready = ctx.graphics(|layers| {
                    layers.get(egui::LayerId::background()).is_some_and(|list| {
                        list.all_entries()
                            .any(|entry| settings_icon(&entry.shape).is_some())
                    })
                });
                if !ready {
                    return;
                }
                self.interaction_started = Some(Instant::now());
                screenshot(ctx, "library");
            }
            1 => ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(380.0, 640.0))),
            2 => {
                screenshot(ctx, "resized-library");
                let position = ctx
                    .graphics(|layers| {
                        layers.get(egui::LayerId::background()).and_then(|list| {
                            list.all_entries()
                                .find_map(|entry| settings_icon(&entry.shape))
                        })
                    })
                    .expect("shared library settings action was not rendered");
                self.clicks
                    .extend([None, Some(true), Some(false)].map(|pressed| (position, pressed)));
            }
            3 => {
                assert!(
                    ctx.graphics(|layers| {
                        layers.get(egui::LayerId::background()).is_some_and(|list| {
                            list.all_entries()
                                .any(|entry| has_text(&entry.shape, "App settings"))
                        })
                    }),
                    "the native click did not open App settings"
                );
                screenshot(ctx, "settings");
            }
            4 => ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true)),
            5 => {
                if ctx.input(|input| input.viewport().fullscreen) != Some(true) {
                    return;
                }
                screenshot(ctx, "fullscreen-settings");
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            }
            6 => {
                if ctx.input(|input| input.viewport().fullscreen) != Some(false) {
                    return;
                }
            }
            _ => unreachable!(),
        }
        self.phase += 1;
        self.phase_started = Instant::now();
        eprintln!("Linux fixture phase: {}", phase_name(self.phase));
    }

    fn on_exit(&mut self) {
        self.app.on_exit();
        self.observed.lock().unwrap().exited = true;
    }
}

fn phase_name(phase: u8) -> &'static str {
    match phase {
        0 => "waiting for window focus and library rendering",
        1 => "resizing the library",
        2 => "opening App settings",
        3 => "checking App settings",
        4 => "entering fullscreen",
        5 => "leaving fullscreen",
        6 => "waiting for windowed mode",
        7 => "requesting close",
        8 => "waiting for shutdown",
        _ => unreachable!(),
    }
}

fn screenshot(ctx: &egui::Context, name: &str) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
        name.to_owned(),
    )));
}

fn settings_icon(shape: &egui::Shape) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == "⚙" => {
            Some(text.visual_bounding_rect().center())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(settings_icon),
        _ => None,
    }
}

fn has_text(shape: &egui::Shape, expected: &str) -> bool {
    match shape {
        egui::Shape::Text(text) => text.galley.text() == expected,
        egui::Shape::Vec(shapes) => shapes.iter().any(|shape| has_text(shape, expected)),
        _ => false,
    }
}
