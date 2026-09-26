use crate::storage::APP_ID;
use eframe::egui;

pub(crate) fn native_options() -> eframe::NativeOptions {
    options(gaming_session(|name| std::env::var(name).ok()))
}

fn options(gaming: bool) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_app_id(APP_ID)
        .with_inner_size([960.0, 720.0])
        .with_min_inner_size([320.0, 320.0]);
    if gaming {
        // Map before Vulkan initialization: Gamescope/Steam must be able to
        // discover the window without waiting for a first present to finish.
        // The pinned eframe patch honors this explicit startup visibility.
        viewport = viewport.with_visible(true);
    }
    let mut options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        // App settings and library persistence are owned by frontend-core.
        persist_window: false,
        ..Default::default()
    };
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        use eframe::wgpu;
        setup.instance_descriptor.backends &= wgpu::Backends::VULKAN | wgpu::Backends::GL;
        setup.power_preference = wgpu::PowerPreference::LowPower;
    }
    options
}

pub(crate) fn gaming_session(get: impl Fn(&str) -> Option<String>) -> bool {
    ["XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP"]
        .iter()
        .any(|name| {
            get(name).is_some_and(|value| {
                value
                    .split(':')
                    .any(|part| part.eq_ignore_ascii_case("gamescope"))
            })
        })
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/window.rs"]
mod tests;
