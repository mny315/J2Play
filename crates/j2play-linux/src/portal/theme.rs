use super::{DESTINATION, PATH, request};
use crate::{
    operation,
    sleep_monitor::{drive, timeout},
};
use diagnostics::EmuError;
use eframe::egui;
use frontend_ui::{PlatformTheme, PlatformThemeMode};
use futures_lite::{StreamExt, future};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use std::thread::{self, JoinHandle};
use zbus::zvariant::OwnedValue;

pub(super) struct ThemeMonitor {
    cancel: async_channel::Sender<()>,
    thread: Option<JoinHandle<()>>,
    preference: Arc<AtomicU32>,
    last: Option<PlatformThemeMode>,
}

impl ThemeMonitor {
    pub(super) fn start(ctx: egui::Context) -> Result<Self, EmuError> {
        let (cancel, receiver) = async_channel::bounded(1);
        let preference = Arc::new(AtomicU32::new(0));
        let shared = preference.clone();
        let thread = thread::Builder::new()
            .name("j2play-linux-theme".into())
            .spawn(move || {
                future::block_on(future::race(
                    async {
                        if watch(&shared, &ctx).await.is_err() {
                            // Theme availability does not prevent running the app.
                            // winit's system theme is the fallback when the portal is absent.
                            shared.store(0, Ordering::Relaxed);
                            ctx.request_repaint();
                        }
                    },
                    async {
                        let _ = receiver.recv().await;
                    },
                ));
            })
            .map_err(|_| {
                operation::error(
                    "linux-theme-thread",
                    "Could not start the desktop theme listener.",
                )
            })?;
        Ok(Self {
            cancel,
            thread: Some(thread),
            preference,
            last: None,
        })
    }

    pub(super) fn poll(&mut self, ctx: &egui::Context) -> Option<Result<PlatformTheme, EmuError>> {
        let preference = self.preference.load(Ordering::Relaxed);
        let mode = match preference {
            1 => PlatformThemeMode::Dark,
            2 => PlatformThemeMode::Light,
            _ => {
                if ctx.input(|input| input.raw.system_theme) == Some(egui::Theme::Light) {
                    PlatformThemeMode::Light
                } else {
                    PlatformThemeMode::Dark
                }
            }
        };
        if self.last == Some(mode) {
            return None;
        }
        self.last = Some(mode);
        Some(Ok(PlatformTheme {
            mode,
            light_colors: None,
            dark_colors: None,
        }))
    }

    pub(super) fn cancel(&self) {
        self.cancel.close();
    }
    pub(super) fn shutdown(&mut self) -> Result<(), EmuError> {
        self.cancel();
        operation::join(&mut self.thread)
    }
}

impl Drop for ThemeMonitor {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("j2play: {error}");
        }
    }
}

async fn watch(preference: &AtomicU32, ctx: &egui::Context) -> Result<(), EmuError> {
    let connection = request::connect().await?;
    watch_connection(&connection, preference, ctx).await
}

async fn watch_connection(
    connection: &zbus::Connection,
    preference: &AtomicU32,
    ctx: &egui::Context,
) -> Result<(), EmuError> {
    drive(connection, async {
        let proxy = timeout(zbus::Proxy::new(
            connection,
            DESTINATION,
            PATH,
            "org.freedesktop.portal.Settings",
        ))
        .await?;
        let mut signals = timeout(proxy.receive_signal("SettingChanged")).await?;
        let mut owners = timeout(proxy.receive_owner_changed()).await?;
        // Read is available in both Settings versions. Unwrap at most the
        // historical double variant; malformed values mean no preference.
        let value: OwnedValue =
            timeout(proxy.call("Read", &("org.freedesktop.appearance", "color-scheme"))).await?;
        update(preference, &value, ctx);
        loop {
            let Some(signal) = future::race(
                signals.next(),
                future::race(
                    async {
                        connection.closed().await;
                        None
                    },
                    async {
                        owners.next().await;
                        None
                    },
                ),
            )
            .await
            else {
                return Err(zbus::Error::Failure("settings disconnected".into()));
            };
            let body = signal.body();
            if body.len() > 4096 {
                continue;
            }
            let (namespace, key, value): (&str, &str, zbus::zvariant::Value<'_>) =
                body.deserialize()?;
            if namespace == "org.freedesktop.appearance" && key == "color-scheme" {
                update(preference, &value, ctx);
            }
        }
    })
    .await
    .map_err(|_| request::unavailable())
}

fn update(preference: &AtomicU32, value: &zbus::zvariant::Value<'_>, ctx: &egui::Context) {
    use zbus::zvariant::Value;
    let value = match value {
        Value::Value(inner) => &**inner,
        value => value,
    };
    let mode = u32::try_from(value).unwrap_or(0);
    if preference.swap(mode, Ordering::Relaxed) == mode {
        return;
    }
    ctx.request_repaint();
}

#[cfg(test)]
#[path = "../../../../tests/unit/j2play-linux/portal/theme.rs"]
mod tests;
