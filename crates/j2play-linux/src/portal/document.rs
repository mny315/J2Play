use super::request;
use crate::{lifecycle::Lifecycle, operation::error, sleep_monitor::drive};
use diagnostics::EmuError;
use frontend_ui::{DocumentKind, DocumentOutcome, PickedDocument};
use futures_lite::future;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::time::{Duration, Instant};
use zbus::zvariant::Value;

// Match the shared archive/descriptor import bounds before allocating bytes.
const JAR_LIMIT: usize = 64 * 1024 * 1024;
const JAD_LIMIT: usize = 1024 * 1024;

pub(super) fn pick(
    id: u64,
    kind: DocumentKind,
    title: &str,
    cancel: &async_channel::Receiver<()>,
    lifecycle: &Lifecycle,
) -> Result<DocumentOutcome, EmuError> {
    let result = future::block_on(async {
        let connection = request::connect().await?;
        let (label, glob) = match kind {
            DocumentKind::Jar => ("JAR", "*.[jJ][aA][rR]"),
            DocumentKind::Jad => ("JAD", "*.[jJ][aA][dD]"),
        };
        let mut options = HashMap::new();
        options.insert("multiple", Value::from(false));
        options.insert("directory", Value::from(false));
        options.insert("filters", Value::from(vec![(label, vec![(0_u32, glob)])]));
        drive(
            &connection,
            request::request(
                &connection,
                id,
                "org.freedesktop.portal.FileChooser",
                "OpenFile",
                title,
                options,
                (cancel, lifecycle),
            ),
        )
        .await
    })?;
    let Some(mut result) = result else {
        return Ok(DocumentOutcome::Cancelled { kind });
    };
    let uris = result
        .remove("uris")
        .and_then(|value| Vec::<String>::try_from(value).ok())
        .filter(|uris| uris.len() == 1)
        .ok_or_else(|| {
            error(
                "linux-document-response",
                "The file picker did not return one file.",
            )
        })?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let document = read_uri(&uris[0], kind, || {
        request::read_cancelled(cancel, lifecycle, deadline)
    });
    // Cancellation is an ordinary picker outcome and retains the existing JAD draft.
    match document {
        Err(error) if error.code() == "linux-document-cancelled" => {
            Ok(DocumentOutcome::Cancelled { kind })
        }
        result => result.map(DocumentOutcome::Selected),
    }
}

fn read_uri(
    uri: &str,
    kind: DocumentKind,
    mut check: impl FnMut() -> Result<(), EmuError>,
) -> Result<PickedDocument, EmuError> {
    check()?;
    if uri.len() > 16 * 1024 {
        return Err(invalid_file());
    }
    let url = url::Url::parse(uri).map_err(|_| invalid_file())?;
    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
        return Err(invalid_file());
    }
    let path = url.to_file_path().map_err(|()| invalid_file())?;
    read_path(&path, kind, check)
}

pub(super) fn read_path(
    path: &std::path::Path,
    kind: DocumentKind,
    mut check: impl FnMut() -> Result<(), EmuError>,
) -> Result<PickedDocument, EmuError> {
    check()?;
    // Nonblocking open avoids waiting on a FIFO before its type can be checked.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(
            i32::try_from((rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOCTTY).bits())
                .map_err(|_| invalid_file())?,
        )
        .open(path)
        .map_err(|_| invalid_file())?;
    let metadata = file.metadata().map_err(|_| invalid_file())?;
    let limit = match kind {
        DocumentKind::Jar => JAR_LIMIT,
        DocumentKind::Jad => JAD_LIMIT,
    };
    if !metadata.is_file() {
        return Err(invalid_file());
    }
    if metadata.len() > u64::try_from(limit).unwrap_or(u64::MAX) {
        return Err(too_large());
    }
    let display_name = path
        .file_name()
        .and_then(|leaf| leaf.to_str())
        .filter(|leaf| leaf.len() <= 512 && !leaf.chars().any(char::is_control))
        .map(str::to_owned);
    let bytes = read_bounded(file, limit, &mut check)?;
    Ok(PickedDocument {
        kind,
        display_name,
        bytes,
    })
}

fn read_bounded(
    mut reader: impl Read,
    limit: usize,
    mut check: impl FnMut() -> Result<(), EmuError>,
) -> Result<Vec<u8>, EmuError> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 16 * 1024];
    loop {
        check()?;
        let size = chunk.len().min(limit.saturating_sub(bytes.len()) + 1);
        let read = match reader.read(&mut chunk[..size]) {
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err(invalid_file()),
        };
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > limit {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    check()?;
    Ok(bytes)
}

fn invalid_file() -> EmuError {
    error(
        "linux-document-read",
        "The selected resource is not a readable local file. Choose a JAR or JAD file and try again.",
    )
}
fn too_large() -> EmuError {
    error(
        "linux-document-size",
        "The selected file exceeds the import size limit.",
    )
}

#[cfg(test)]
#[path = "../../../../tests/unit/j2play-linux/portal/document.rs"]
mod tests;
