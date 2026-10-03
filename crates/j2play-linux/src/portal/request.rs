use super::{DESTINATION, PATH, REQUEST_TIMEOUT};
use crate::{
    lifecycle::Lifecycle,
    operation::error,
    sleep_monitor::{drive, timeout},
};
use diagnostics::EmuError;
use futures_lite::{StreamExt, future};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

pub(super) type Results = HashMap<String, OwnedValue>;

pub(super) async fn connect() -> Result<zbus::Connection, EmuError> {
    timeout(async {
        zbus::connection::Builder::session()?
            .internal_executor(false)
            .max_queued(8)
            .method_timeout(Duration::from_secs(2))
            .build()
            .await
    })
    .await
    .map_err(|_| unavailable())
}

pub(super) fn unavailable() -> EmuError {
    error(
        "linux-portal-unavailable",
        "The desktop portal is unavailable. Check xdg-desktop-portal and its FileChooser/OpenURI backend, then try again.",
    )
}

pub(super) async fn cancelled(cancel: &async_channel::Receiver<()>, lifecycle: &Lifecycle) {
    loop {
        if cancel.is_closed() || lifecycle.system_suspended() {
            return;
        }
        async_io::Timer::after(Duration::from_millis(20)).await;
    }
}

/// Subscribe to the predicted request path before calling the method so even
/// immediate responses cannot be lost. A mismatched path fails explicitly.
pub(super) async fn request(
    connection: &zbus::Connection,
    id: u64,
    interface: &str,
    method: &str,
    argument: &str,
    mut options: HashMap<&str, Value<'_>>,
    cancellation: (&async_channel::Receiver<()>, &Lifecycle),
) -> Result<Option<Results>, EmuError> {
    let (cancel, lifecycle) = cancellation;
    if cancel.is_closed() || lifecycle.system_suspended() {
        return Ok(None);
    }
    let (token, path) = request_path(connection, id)?;
    let request = timeout(zbus::Proxy::new(
        connection,
        DESTINATION,
        path.as_str(),
        "org.freedesktop.portal.Request",
    ))
    .await
    .map_err(|_| unavailable())?;
    let mut responses = timeout(request.receive_signal("Response"))
        .await
        .map_err(|_| unavailable())?;
    let portal = timeout(zbus::Proxy::new(connection, DESTINATION, PATH, interface))
        .await
        .map_err(|_| unavailable())?;
    let mut owners = timeout(portal.receive_owner_changed())
        .await
        .map_err(|_| unavailable())?;
    options.insert("handle_token", Value::from(token.as_str()));
    // Empty parent is supported by the portal specification. The shell does
    // not leak native pointers or fabricate a Wayland exported window handle.
    let result = Box::pin(future::race(
        async {
            let handle: OwnedObjectPath = timeout(portal.call(method, &("", argument, options)))
                .await
                .map_err(|_| unavailable())?;
            if handle.as_str() != path {
                return Err(error(
                    "linux-portal-handle",
                    "The desktop portal returned an unexpected request handle.",
                ));
            }
            let response = future::race(
                responses.next(),
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
            .ok_or_else(unavailable)?;
            if response.body().len() > 64 * 1024 {
                return Err(error(
                    "linux-portal-response",
                    "The desktop portal response exceeded its size limit.",
                ));
            }
            let (code, values): (u32, Results) =
                response.body().deserialize().map_err(|_| unavailable())?;
            match code {
                0 => Ok(Some(values)),
                1 => Ok(None),
                _ => Err(error(
                    "linux-portal-rejected",
                    "The desktop portal could not complete this request.",
                )),
            }
        },
        future::race(
            async {
                cancelled(cancel, lifecycle).await;
                Ok(None)
            },
            async {
                async_io::Timer::after(REQUEST_TIMEOUT).await;
                Err(error(
                    "linux-portal-timeout",
                    "The desktop request timed out. Please try again.",
                ))
            },
        ),
    ))
    .await;
    if !matches!(result, Ok(Some(_))) {
        let _ = timeout(request.call::<_, _, ()>("Close", &())).await;
    }
    result
}

fn request_path(connection: &zbus::Connection, id: u64) -> Result<(String, String), EmuError> {
    let token = format!("j2play_{id}");
    let sender = connection
        .unique_name()
        .ok_or_else(unavailable)?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    Ok((token, path))
}

pub(super) fn validate_url(text: &str) -> Result<(), EmuError> {
    if text.is_empty() || text.len() > 4096 || text.chars().any(char::is_control) {
        return Err(error(
            "platform-request-url",
            "The external URL is invalid or too long.",
        ));
    }
    let url = url::Url::parse(text)
        .map_err(|_| error("platform-request-url", "The external URL is invalid."))?;
    if !matches!(url.scheme(), "http" | "https" | "mailto" | "tel" | "sms")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(error(
            "platform-request-scheme",
            "Only web, email, phone and SMS links without credentials can be opened.",
        ));
    }
    Ok(())
}

pub(super) async fn open_url(
    id: u64,
    url: &str,
    cancel: &async_channel::Receiver<()>,
    lifecycle: &Lifecycle,
) -> Result<(), EmuError> {
    validate_url(url)?;
    let connection = connect().await?;
    drive(
        &connection,
        request(
            &connection,
            id,
            "org.freedesktop.portal.OpenURI",
            "OpenURI",
            url,
            HashMap::new(),
            (cancel, lifecycle),
        ),
    )
    .await?;
    Ok(())
}

pub(super) fn read_cancelled(
    cancel: &async_channel::Receiver<()>,
    lifecycle: &Lifecycle,
    deadline: Instant,
) -> Result<(), EmuError> {
    if cancel.is_closed() || lifecycle.system_suspended() {
        return Err(error(
            "linux-document-cancelled",
            "File import was cancelled.",
        ));
    }
    if Instant::now() >= deadline {
        return Err(error(
            "linux-document-timeout",
            "Reading the selected file timed out.",
        ));
    }
    Ok(())
}
