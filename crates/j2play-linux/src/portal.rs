mod browser;
mod document;
mod request;
mod theme;

use crate::{lifecycle::Lifecycle, operation};
use diagnostics::EmuError;
use eframe::egui;
use frontend_ui::{DocumentBrowser, DocumentKind, DocumentOutcome, PlatformTheme};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) const DESTINATION: &str = "org.freedesktop.portal.Desktop";
pub(crate) const PATH: &str = "/org/freedesktop/portal/desktop";
const REQUEST_TIMEOUT: Duration = Duration::from_mins(2);

/// At most one selection and one approved URL can be pending. Each has a
/// fresh D-Bus handle and connection; late replies cannot reach a newer request.
pub(crate) struct Portals {
    ctx: egui::Context,
    lifecycle: Lifecycle,
    next_id: u64,
    document: Option<Pending<DocumentOutcome>>,
    external: Option<Pending<DocumentOutcome>>,
    document_kind: Option<DocumentKind>,
    browser: Option<browser::Browser>,
    browser_directory: std::path::PathBuf,
    in_window_picker: bool,
    url: Option<Pending<()>>,
    theme: theme::ThemeMonitor,
}

impl Portals {
    pub(crate) fn new(ctx: egui::Context, lifecycle: Lifecycle) -> Result<Self, EmuError> {
        Ok(Self {
            theme: theme::ThemeMonitor::start(ctx.clone())?,
            ctx,
            lifecycle,
            next_id: 0,
            document: None,
            external: None,
            document_kind: None,
            browser: None,
            browser_directory: browser::home_directory(),
            in_window_picker: crate::window::gaming_session(|name| std::env::var(name).ok()),
            url: None,
        })
    }

    fn next_id(&mut self) -> Result<u64, EmuError> {
        self.next_id = self.next_id.checked_add(1).ok_or_else(|| {
            operation::error(
                "linux-portal-id",
                "Desktop request identifiers were exhausted.",
            )
        })?;
        Ok(self.next_id)
    }

    pub(crate) fn pick(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        if self.document.is_some() || self.browser.is_some() {
            return Err(operation::error(
                "document-picker-busy",
                "Another document selection is still active.",
            ));
        }
        if self.in_window_picker {
            return self.start_browser(kind);
        }
        let id = self.next_id()?;
        let title = kind.picker_title(&self.ctx);
        let lifecycle = self.lifecycle.clone();
        self.document = Some(Pending::start(&self.ctx, move |cancel| {
            document::pick(id, kind, &title, &cancel, &lifecycle)
        })?);
        self.document_kind = Some(kind);
        Ok(())
    }

    pub(crate) fn poll_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        if let Some(browser) = &mut self.browser {
            let result = browser.poll();
            if result.is_some() {
                self.browser_directory = browser.current_directory().to_owned();
                self.browser = None;
            }
            return result;
        }
        let result = Pending::poll_slot(&mut self.document)?;
        if result.as_ref().is_err_and(|error| {
            matches!(
                error.code(),
                "linux-portal-unavailable" | "linux-portal-rejected"
            )
        }) && let Some(kind) = self.document_kind.take()
        {
            self.in_window_picker = true;
            return self.start_browser(kind).err().map(Err);
        }
        self.document_kind = None;
        Some(result)
    }

    fn start_browser(&mut self, kind: DocumentKind) -> Result<(), EmuError> {
        self.browser = Some(browser::Browser::start(
            kind,
            self.browser_directory.clone(),
            self.ctx.clone(),
            self.lifecycle.clone(),
        )?);
        Ok(())
    }

    pub(crate) fn external_pending(&self) -> bool {
        self.external.is_some()
    }

    pub(crate) fn open_file(&mut self, path: std::path::PathBuf) -> Result<(), EmuError> {
        let lifecycle = self.lifecycle.clone();
        self.external = Some(Pending::start(&self.ctx, move |cancel| {
            let deadline = Instant::now() + Duration::from_secs(15);
            document::read_path(&path, DocumentKind::Jar, || {
                request::read_cancelled(&cancel, &lifecycle, deadline)
            })
            .map(DocumentOutcome::Selected)
        })?);
        Ok(())
    }

    pub(crate) fn poll_external_document(&mut self) -> Option<Result<DocumentOutcome, EmuError>> {
        Pending::poll_slot(&mut self.external)
    }

    pub(crate) fn document_browser(&self) -> Option<&DocumentBrowser> {
        self.browser.as_ref().and_then(browser::Browser::view)
    }

    pub(crate) fn activate_document_entry(&mut self, id: u64) -> Result<(), EmuError> {
        self.browser
            .as_mut()
            .ok_or_else(|| {
                operation::error("linux-document-selection", "File selection has closed.")
            })?
            .activate(id)
    }

    pub(crate) fn cancel_document_browser(&mut self) {
        if let Some(browser) = &mut self.browser {
            browser.cancel();
        }
    }

    pub(crate) fn pending(&self) -> bool {
        self.document.is_some()
            || self.external.is_some()
            || self.browser.is_some()
            || self.url.is_some()
    }

    pub(crate) fn open_url(&mut self, url: &str) -> Result<(), EmuError> {
        request::validate_url(url)?;
        if self.url.is_some() {
            return Err(operation::error(
                "linux-url-busy",
                "Another external link is still being opened.",
            ));
        }
        let id = self.next_id()?;
        let url = url.to_owned();
        let lifecycle = self.lifecycle.clone();
        self.url = Some(Pending::start(&self.ctx, move |cancel| {
            futures_lite::future::block_on(request::open_url(id, &url, &cancel, &lifecycle))
        })?);
        Ok(())
    }

    pub(crate) fn poll_error(&mut self) -> Option<EmuError> {
        Pending::poll_slot(&mut self.url).and_then(Result::err)
    }

    pub(crate) fn poll_theme(&mut self) -> Option<Result<PlatformTheme, EmuError>> {
        self.theme.poll(&self.ctx)
    }

    pub(crate) fn cancel_url(&mut self) {
        if let Some(pending) = &mut self.url {
            pending.cancel_result();
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.cancel_url();
        self.cancel_document_browser();
        if let Some(pending) = &self.document {
            pending.cancel.close();
        }
        if let Some(pending) = &self.external {
            pending.cancel.close();
        }
        self.theme.cancel();
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), EmuError> {
        self.cancel();
        let document = self.document.as_mut().map_or(Ok(()), Pending::shutdown);
        let external = self.external.as_mut().map_or(Ok(()), Pending::shutdown);
        let browser = self
            .browser
            .as_mut()
            .map_or(Ok(()), browser::Browser::shutdown);
        let url = self.url.as_mut().map_or(Ok(()), Pending::shutdown);
        let theme = self.theme.shutdown();
        document.and(external).and(browser).and(url).and(theme)
    }
}

struct Pending<T> {
    cancel: async_channel::Sender<()>,
    result: mpsc::Receiver<Result<T, EmuError>>,
    thread: Option<JoinHandle<()>>,
    deadline: Instant,
    delivered: bool,
}

impl<T: Send + 'static> Pending<T> {
    fn start(
        ctx: &egui::Context,
        run: impl FnOnce(async_channel::Receiver<()>) -> Result<T, EmuError> + Send + 'static,
    ) -> Result<Self, EmuError> {
        let (cancel, receiver) = async_channel::bounded(1);
        let (sender, result) = mpsc::sync_channel(1);
        let ctx = ctx.clone();
        let thread = thread::Builder::new()
            .name("j2play-linux-portal".into())
            .spawn(move || {
                let _ = sender.send(run(receiver));
                ctx.request_repaint();
            })
            .map_err(|_| {
                operation::error("linux-portal-thread", "Could not start a desktop request.")
            })?;
        Ok(Self {
            cancel,
            result,
            thread: Some(thread),
            deadline: Instant::now() + REQUEST_TIMEOUT + Duration::from_secs(20),
            delivered: false,
        })
    }

    fn poll_slot(slot: &mut Option<Self>) -> Option<Result<T, EmuError>> {
        let pending = slot.as_mut()?;
        if pending.thread.as_ref().is_none_or(JoinHandle::is_finished) {
            let joined = operation::join(&mut pending.thread);
            let delivered = pending.delivered;
            let result = pending.result.try_recv().ok();
            *slot = None;
            return if delivered {
                None
            } else {
                Some(joined.and_then(|()| {
                    result.unwrap_or_else(|| {
                        Err(operation::error(
                            "linux-portal-thread",
                            "The desktop request ended without a response.",
                        ))
                    })
                }))
            };
        }
        if !pending.delivered && Instant::now() >= pending.deadline {
            pending.delivered = true;
            pending.cancel.close();
            return Some(Err(operation::error(
                "linux-portal-timeout",
                "The desktop request timed out. Cancel the picker and try again.",
            )));
        }
        None
    }

    fn shutdown(&mut self) -> Result<(), EmuError> {
        self.cancel.close();
        operation::join(&mut self.thread)
    }

    fn cancel_result(&mut self) {
        self.cancel.close();
        // Stop invalidates this game's request, including an error already
        // queued while a new game is starting. The thread must still be joined.
        self.delivered = true;
    }
}

impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        self.cancel.close();
        if let Err(error) = operation::join(&mut self.thread) {
            eprintln!("j2play: {error}");
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/portal.rs"]
mod tests;
