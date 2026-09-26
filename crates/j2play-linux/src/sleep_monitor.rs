use crate::{
    lifecycle::Lifecycle,
    operation::{self, error as monitor_error},
};
use diagnostics::EmuError;
use eframe::egui;
use futures_lite::{StreamExt, future};
use std::future::Future;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DBUS_TIMEOUT: Duration = Duration::from_secs(2);
const SUSPEND_DEADLINE: Duration = Duration::from_secs(2);

/// The logind listener runs without the render loop and owns its D-Bus
/// executor. Cancelling drops even a pending connect/property/signal future.
pub(crate) struct SleepMonitor {
    cancel: async_channel::Sender<()>,
    thread: Option<JoinHandle<()>>,
    errors: mpsc::Receiver<EmuError>,
}

impl SleepMonitor {
    pub(crate) fn start(lifecycle: Lifecycle, ctx: egui::Context) -> Result<Self, EmuError> {
        let (cancel, worker_cancel) = async_channel::bounded(1);
        let (errors_tx, errors) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("j2play-linux-lifecycle".into())
            .spawn(move || {
                let result =
                    future::block_on(until_cancelled(&worker_cancel, monitor(&lifecycle, &ctx)));
                if let Err(error) = result {
                    lifecycle.sleep(true);
                    let _ = errors_tx.try_send(error);
                    ctx.request_repaint();
                }
            })
            .map_err(|_| {
                monitor_error(
                    "linux-lifecycle-thread",
                    "Could not start the Linux lifecycle listener.",
                )
            })?;
        Ok(Self {
            cancel,
            thread: Some(thread),
            errors,
        })
    }

    pub(crate) fn poll_error(&self) -> Option<EmuError> {
        self.errors.try_recv().ok()
    }

    pub(crate) fn cancel(&self) {
        self.cancel.close();
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), EmuError> {
        self.cancel();
        operation::join(&mut self.thread)
    }
}

impl Drop for SleepMonitor {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("j2play: {error}");
        }
    }
}

async fn until_cancelled<T>(
    cancel: &async_channel::Receiver<()>,
    operation: impl Future<Output = Result<T, EmuError>>,
) -> Result<(), EmuError> {
    future::race(async { operation.await.map(|_| ()) }, async {
        let _ = cancel.recv().await;
        Ok(())
    })
    .await
}

pub(crate) async fn timeout<T>(
    operation: impl Future<Output = zbus::Result<T>>,
) -> zbus::Result<T> {
    future::race(operation, async {
        async_io::Timer::after(DBUS_TIMEOUT).await;
        Err(zbus::Error::InputOutput(Arc::new(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "desktop service timed out",
        ))))
    })
    .await
}

pub(crate) async fn drive<T>(
    connection: &zbus::Connection,
    operation: impl Future<Output = T>,
) -> T {
    future::race(
        operation,
        Box::pin(async {
            loop {
                connection.executor().tick().await;
            }
        }),
    )
    .await
}

async fn monitor(lifecycle: &Lifecycle, ctx: &egui::Context) -> Result<(), EmuError> {
    let connected = async {
        zbus::connection::Builder::system()?
            .internal_executor(false)
            .max_queued(8)
            .method_timeout(DBUS_TIMEOUT)
            .build()
            .await
    };
    let connection = timeout(connected).await.map_err(|_| unavailable())?;
    monitor_connection(&connection, lifecycle, ctx).await
}

async fn monitor_connection(
    connection: &zbus::Connection,
    lifecycle: &Lifecycle,
    ctx: &egui::Context,
) -> Result<(), EmuError> {
    Box::pin(drive(connection, async {
        let (proxy, mut signals, mut owner_changes, sleeping, inhibitor) = timeout(async {
            let proxy = zbus::Proxy::new(
                connection,
                "org.freedesktop.login1",
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
            )
            .await?;
            let signals = proxy.receive_signal("PrepareForSleep").await?;
            let owner_changes = proxy.receive_owner_changed().await?;
            let inhibitor = inhibit_sleep(&proxy).await?;
            let sleeping = proxy.get_property::<bool>("PreparingForSleep").await?;
            Ok((proxy, signals, owner_changes, sleeping, inhibitor))
        })
        .await
        .map_err(|_| unavailable())?;
        let mut inhibitor = Some(inhibitor);
        lifecycle.sleep(sleeping);
        ctx.request_repaint();
        if sleeping {
            await_suspension(lifecycle).await?;
            inhibitor.take();
        }
        loop {
            let signal = future::race(
                signals.next(),
                future::race(
                    async {
                        connection.closed().await;
                        None
                    },
                    async {
                        // A restarted login manager invalidates its delay inhibitor.
                        // Fail closed instead of silently losing system sleep coverage.
                        owner_changes.next().await;
                        None
                    },
                ),
            )
            .await
            .ok_or_else(unavailable)?;
            let (sleeping,) = signal
                .body()
                .deserialize::<(bool,)>()
                .map_err(|_| unavailable())?;
            if !sleeping {
                inhibitor = Some(
                    timeout(inhibit_sleep(&proxy))
                        .await
                        .map_err(|_| unavailable())?,
                );
            }
            lifecycle.sleep(sleeping);
            ctx.request_repaint();
            if sleeping {
                // Sending the signal alone does not stop input/output. Hold
                // the delay until the VM owner acknowledges this exact cycle.
                await_suspension(lifecycle).await?;
                inhibitor.take();
            }
        }
    }))
    .await
}

async fn await_suspension(lifecycle: &Lifecycle) -> Result<(), EmuError> {
    let deadline = Instant::now() + SUSPEND_DEADLINE;
    while !lifecycle.signal.suspension_acknowledged() {
        if Instant::now() >= deadline {
            return Err(monitor_error(
                "linux-system-suspend-deadline",
                "The emulator did not confirm suspension within two seconds. Games remain suspended. Restart J2Play.",
            ));
        }
        async_io::Timer::after(Duration::from_millis(5)).await;
    }
    Ok(())
}

async fn inhibit_sleep(proxy: &zbus::Proxy<'_>) -> zbus::Result<zbus::zvariant::OwnedFd> {
    proxy
        .call(
            "Inhibit",
            &(
                "sleep",
                "J2Play",
                "Suspend emulator input and output",
                "delay",
            ),
        )
        .await
}

fn unavailable() -> EmuError {
    monitor_error(
        "linux-system-lifecycle",
        "System sleep notifications are unavailable. Games are suspended. Check that logind or elogind is running and accessible, then restart J2Play.",
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/sleep_monitor.rs"]
mod tests;
