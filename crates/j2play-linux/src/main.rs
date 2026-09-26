mod devices;
mod fullscreen;
mod input;
mod lifecycle;
mod open_file;
mod operation;
mod platform_bridge;
mod portal;
mod sleep_monitor;
mod startup_error;
mod storage;
mod window;

use eframe::{App, egui};
use frontend_ui::FrontendApp;
use lifecycle::{Lifecycle, WindowInput};
use platform_bridge::LinuxPlatformBridge;
use startup_error::StartupErrorApp;
use std::process::ExitCode;
use storage::AppPaths;
use window::native_options;

fn main() -> ExitCode {
    let setup = (|| {
        let paths = AppPaths::from_environment()?;
        let file = open_file::argument(std::env::args_os().skip(1))?;
        match paths.acquire() {
            Ok(guard) => {
                let opens = open_file::OpenFiles::start(&paths.data, file)?;
                Ok(Some((paths, guard, opens)))
            }
            Err(error) if error.code() == "linux-already-running" => {
                open_file::forward(&paths.data, file.as_deref())?;
                Ok(None)
            }
            Err(error) => Err(error),
        }
    })();
    if matches!(setup, Ok(None)) {
        return ExitCode::SUCCESS;
    }
    match eframe::run_native(
        "J2Play",
        native_options(),
        Box::new(move |creation| {
            let app = setup.and_then(|setup| {
                let (paths, guard, opens) =
                    setup.expect("forwarded activations returned before opening a window");
                LinuxApp::with_instance(&creation.egui_ctx, &paths, guard, opens)
            });
            Ok(create_app(app))
        }),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("j2play: platform[linux-window]: {error}");
            startup_error::show_without_gpu(
                "J2Play could not open its window. Check your Wayland or X11 session and graphics driver. The verified Linux graphics backend requires a Vulkan 1.1 or newer compatible driver.",
            );
            ExitCode::FAILURE
        }
    }
}

fn create_app(app: Result<LinuxApp, diagnostics::EmuError>) -> Box<dyn App> {
    match app {
        Ok(app) => Box::new(app),
        Err(error) => {
            eprintln!("j2play: {error}");
            Box::new(StartupErrorApp::new(
                error.message(),
                platform_bridge::system_language(),
            ))
        }
    }
}

impl LinuxApp {
    #[cfg(test)]
    fn new(ctx: &egui::Context, paths: &AppPaths) -> Result<Self, diagnostics::EmuError> {
        let instance = paths.acquire()?;
        let opens = open_file::OpenFiles::start(&paths.data, None)?;
        Self::with_instance(ctx, paths, instance, opens)
    }

    fn with_instance(
        ctx: &egui::Context,
        paths: &AppPaths,
        instance: storage::InstanceGuard,
        opens: open_file::OpenFiles,
    ) -> Result<Self, diagnostics::EmuError> {
        let lifecycle = Lifecycle::default();
        opens.attach(ctx.clone());
        let file_drops = opens.drops();
        let platform = LinuxPlatformBridge::new(ctx.clone(), lifecycle.clone(), opens)?;
        let frontend =
            FrontendApp::with_repository(paths.repository(instance)?, Box::new(platform))?;
        Ok(Self {
            frontend,
            lifecycle,
            window_input: WindowInput::default(),
            file_drops,
        })
    }
}

/// Owns the window-facing lifecycle adapter around the same shared App used
/// on Android. The repository retains the library lock through worker exit.
struct LinuxApp {
    frontend: FrontendApp,
    lifecycle: Lifecycle,
    window_input: WindowInput,
    file_drops: open_file::FileDrops,
}

impl App for LinuxApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.window_input.observe(&self.lifecycle, input);
        self.file_drops.take(input);
        self.frontend.raw_input_hook(ctx, input);
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(window) = frame.winit_window() {
            self.lifecycle
                .window(window.has_focus(), window.is_minimized().unwrap_or(false));
        }
        self.frontend.logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.window_input.painted();
        self.frontend.ui(ui, frame);
    }

    fn on_exit(&mut self) {
        self.lifecycle.destroy();
        self.frontend.on_exit();
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.frontend.clear_color(visuals)
    }
}

#[cfg(test)]
#[path = "../../../tests/support/storage.rs"]
mod test_storage;

#[cfg(test)]
#[path = "../../../tests/support/dbus.rs"]
mod test_bus;

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/native_window.rs"]
mod native_window_tests;
