#[cfg(target_os = "android")]
use android_activity::AndroidApp;
#[cfg(target_os = "android")]
use eframe::egui;
#[cfg(target_os = "android")]
use frontend_ui::{FrontendApp, PlatformBridge};

#[cfg(any(target_os = "android", test))]
#[cfg_attr(all(test, not(target_os = "android")), allow(dead_code))]
mod platform_bridge;

#[cfg(target_os = "android")]
mod android_adapter;

#[cfg(any(target_os = "android", test))]
mod android_theme;

#[cfg(any(target_os = "android", test))]
mod physical_input;

#[cfg(test)]
#[path = "../build-support/callback_classes.rs"]
mod callback_classes;

#[cfg(target_os = "android")]
struct StartupErrorApp {
    presentation: frontend_ui::StartupErrorApp,
    platform: Option<platform_bridge::AndroidPlatformBridge>,
}

#[cfg(target_os = "android")]
impl StartupErrorApp {
    fn new(detail: &str, platform: Option<platform_bridge::AndroidPlatformBridge>) -> Self {
        Self {
            presentation: frontend_ui::StartupErrorApp::new(
                detail,
                frontend_core::Language::English,
            ),
            platform,
        }
    }
}

#[cfg(target_os = "android")]
impl eframe::App for StartupErrorApp {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(platform) = &self.platform {
            self.presentation.set_language(platform.system_language());
            self.presentation
                .set_text_scale(platform.system_text_scale());
        }
        self.presentation.logic(ctx, frame);
        if self
            .platform
            .as_ref()
            .is_some_and(PlatformBridge::needs_periodic_poll)
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let platform_back = self
            .platform
            .as_mut()
            .is_some_and(PlatformBridge::poll_navigation_back);
        let fallback_back = ctx.input(|input| {
            (input.key_pressed(egui::Key::BrowserBack)
                && !self
                    .platform
                    .as_ref()
                    .is_some_and(PlatformBridge::uses_platform_navigation_back))
                || input.viewport().close_requested()
        });
        if platform_back || fallback_back {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.presentation.ui(ui, frame);
    }

    fn on_exit(&mut self) {
        if let Some(platform) = &mut self.platform
            && let Err(error) = platform.shutdown()
        {
            eprintln!("j2play: platform[startup-error-cleanup]: {error}");
        }
    }
}

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
extern "Rust" fn android_main(android_app: AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    let startup_app: Box<dyn eframe::App> = android_app.internal_data_path().map_or_else(
        || {
            Box::new(StartupErrorApp::new(
                "platform[android-data-path]: NativeActivity returned no internal data path",
                None,
            )) as Box<dyn eframe::App>
        },
        |data_path| {
            platform_bridge::AndroidPlatformBridge::new()
                .and_then(|platform| FrontendApp::new(&data_path, Box::new(platform)))
                .map_or_else(
                    |error| {
                        Box::new(StartupErrorApp::new(
                            &error.to_string(),
                            platform_bridge::AndroidPlatformBridge::new().ok(),
                        )) as Box<dyn eframe::App>
                    },
                    |frontend| Box::new(frontend) as Box<dyn eframe::App>,
                )
        },
    );
    let mut options = eframe::NativeOptions {
        android_app: Some(android_app),
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_input_handler(android_adapter::physical_input::native_event);
        })),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        // Android windows can be connected to only one graphics API at a time.
        // Probing Vulkan before selecting GLES can leave the window connected
        // and make GLES surface creation fail (wgpu issue #2384).
        setup.instance_descriptor.backends = eframe::wgpu::Backends::GL;
    }
    if let Err(error) = eframe::run_native("J2Play", options, Box::new(move |_| Ok(startup_app))) {
        eprintln!("j2play: platform[android-ui]: {error}");
    }
}

#[cfg(test)]
#[path = "../../../tests/support/storage.rs"]
mod test_storage;
