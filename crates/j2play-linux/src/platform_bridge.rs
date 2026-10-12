use crate::{
    devices::Devices, fullscreen::Fullscreen, input::Input, lifecycle::Lifecycle, portal::Portals,
    sleep_monitor::SleepMonitor,
};
use diagnostics::EmuError;
use eframe::egui;
use frontend_core::{AudioMailbox, PlatformLifecycleSignal, VibrationMailbox};
use frontend_ui::{
    DocumentKind, DocumentOutcome, OrientationControl, PhysicalInputConfig, PlatformBridge,
    PlatformLifecycleEvent, PlatformTheme,
};
use std::sync::Arc;

pub(crate) struct LinuxPlatformBridge {
    ctx: egui::Context,
    lifecycle: Lifecycle,
    monitor: SleepMonitor,
    portals: Portals,
    input: Input,
    devices: Option<Devices>,
    fullscreen: Fullscreen,
    opens: crate::open_file::OpenFiles,
}

impl LinuxPlatformBridge {
    pub(crate) fn new(
        ctx: egui::Context,
        lifecycle: Lifecycle,
        opens: crate::open_file::OpenFiles,
    ) -> Result<Self, EmuError> {
        let monitor = SleepMonitor::start(lifecycle.clone(), ctx.clone())?;
        let portals = Portals::new(ctx.clone(), lifecycle.clone())?;
        Ok(Self {
            ctx,
            lifecycle,
            monitor,
            portals,
            input: Input::default(),
            devices: None,
            fullscreen: Fullscreen::default(),
            opens,
        })
    }
}

impl PlatformBridge for LinuxPlatformBridge {
    fn system_language(&self) -> frontend_core::Language {
        system_language()
    }

    fn orientation_control(&self) -> OrientationControl {
        OrientationControl::Layout
    }

    fn escape_navigates_back(&self) -> bool {
        true
    }

    fn close_exits_app(&self) -> bool {
        true
    }

    fn uses_window_ime(&self) -> bool {
        true
    }

    fn cancel_pending_operations(&mut self) {
        self.lifecycle.destroy();
        self.monitor.cancel();
        self.portals.cancel();
        if let Some(devices) = &self.devices {
            devices.cancel();
        }
    }

    fn request_document(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        self.portals.pick(kind)
    }

    fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        self.portals.poll_document()
    }

    fn poll_external_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        if !self.portals.external_pending()
            && let Some(error) = self.opens.poll_drop_error()
        {
            return Some(Err(error));
        }
        if !self.portals.external_pending()
            && let Some(path) = self.opens.poll()
            && let Err(error) = self.portals.open_file(path)
        {
            return Some(Err(error));
        }
        self.portals.poll_external_document()
    }

    fn document_browser(&self) -> Option<&frontend_ui::DocumentBrowser> {
        self.portals.document_browser()
    }

    fn activate_document_entry(&mut self, id: u64) -> Result<(), EmuError> {
        self.portals.activate_document_entry(id)
    }

    fn cancel_document_browser(&mut self) {
        self.portals.cancel_document_browser();
    }

    fn poll_physical_input(&mut self) -> Option<frontend_core::physical_input::PhysicalInputEvent> {
        self.input.poll()
    }
    fn configure_physical_input(
        &mut self,
        _config: PhysicalInputConfig,
        bindings: &frontend_core::physical_input::PhysicalBindings,
    ) {
        self.input.configure(bindings.dead_zone_percent);
    }
    fn physical_devices(&self) -> Vec<String> {
        self.input.devices()
    }
    fn has_physical_devices(&self) -> bool {
        self.input.has_devices()
    }
    fn set_physical_input_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.input.waker(waker);
    }
    fn poll_theme(&mut self) -> Option<Result<PlatformTheme, EmuError>> {
        self.portals.poll_theme()
    }
    fn open_external_url(&mut self, url: &str) -> Result<(), EmuError> {
        self.portals.open_url(url)
    }
    fn cancel_guest_operations(&mut self) {
        self.portals.cancel_url();
    }
    fn needs_periodic_poll(&self) -> bool {
        self.portals.pending() || self.fullscreen.pending()
    }

    fn lifecycle_signal(&self) -> PlatformLifecycleSignal {
        self.lifecycle.signal.clone()
    }

    fn poll_lifecycle(&mut self) -> Option<PlatformLifecycleEvent> {
        self.lifecycle.poll()
    }

    fn set_fullscreen(&mut self, fullscreen: bool) -> Result<(), EmuError> {
        self.fullscreen
            .request(fullscreen, std::time::Instant::now());
        self.ctx
            .send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        Ok(())
    }

    fn poll_fullscreen(&mut self) -> Option<Result<bool, EmuError>> {
        self.fullscreen.poll(
            self.ctx.input(|input| input.viewport().fullscreen),
            std::time::Instant::now(),
        )
    }

    fn bind_runtime_effects(
        &mut self,
        audio: AudioMailbox,
        vibration: VibrationMailbox,
    ) -> Result<(), EmuError> {
        self.devices = Some(Devices::start(
            self.input.clone(),
            self.lifecycle.signal.clone(),
            audio,
            vibration,
        )?);
        Ok(())
    }

    fn pump_runtime_effects(&mut self) -> Result<(), EmuError> {
        self.opens.focus(&self.ctx);
        if let Some(error) = self.monitor.poll_error() {
            return Err(error);
        }
        if let Some(error) = self
            .portals
            .poll_error()
            .or_else(|| self.input.poll_error())
        {
            return Err(error);
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), EmuError> {
        self.cancel_pending_operations();
        let monitor = self.monitor.shutdown();
        let portals = self.portals.shutdown();
        let devices = self.devices.as_mut().map_or(Ok(()), Devices::shutdown);
        let opens = self.opens.shutdown();
        monitor.and(portals).and(devices).and(opens)
    }
}

pub(crate) fn system_language() -> frontend_core::Language {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok())
        .find(|value| !value.is_empty())
        .map_or(frontend_core::Language::English, |locale| {
            frontend_core::Language::from_locale(&locale)
        })
}
