use crate::sleep_monitor::{drive, timeout};
use crate::storage::APP_ID;
pub(crate) use frontend_ui::StartupErrorApp;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A failed GPU/window initialization cannot draw an egui dialog. Ask the
/// desktop notification service, then use a toolkit dialog if it is absent.
pub(crate) fn show_without_gpu(message: &str) {
    let (title, explanation) =
        frontend_ui::startup_error_text(crate::platform_bridge::system_language(), message);
    let message = explanation.as_str();
    if futures_lite::future::block_on(notify(&title, message)).is_ok() {
        return;
    }
    let Ok(mut dialog) = Command::new("zenity")
        .env("GSK_RENDERER", "cairo")
        .args([
            "--error",
            "--no-markup",
            "--title",
            &title,
            "--text",
            message,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        eprintln!("j2play: Desktop error reporting needs a notification service or zenity.");
        return;
    };
    let deadline = Instant::now() + Duration::from_mins(1);
    loop {
        match dialog.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = dialog.kill();
                let _ = dialog.wait();
                return;
            }
        }
    }
}

async fn notify(title: &str, message: &str) -> zbus::Result<()> {
    let connection = timeout(async {
        zbus::connection::Builder::session()?
            .max_queued(8)
            .internal_executor(false)
            .method_timeout(Duration::from_secs(2))
            .build()
            .await
    })
    .await?;
    drive(
        &connection,
        timeout(async {
            let proxy = zbus::Proxy::new(
                &connection,
                "org.freedesktop.Notifications",
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications",
            )
            .await?;
            let hints = HashMap::from([
                ("urgency", zbus::zvariant::Value::U8(2)),
                ("desktop-entry", zbus::zvariant::Value::from(APP_ID)),
            ]);
            let _: u32 = proxy
                .call(
                    "Notify",
                    &(
                        "J2Play",
                        0_u32,
                        "dialog-error",
                        title,
                        message,
                        Vec::<&str>::new(),
                        hints,
                        0_i32,
                    ),
                )
                .await?;
            Ok(())
        }),
    )
    .await
}
