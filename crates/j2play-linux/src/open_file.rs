//! Bounded file activation for the process owning the library lock.

use crate::operation;
use diagnostics::EmuError;
use eframe::egui;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_PATH: usize = 16 * 1024;
const TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone)]
pub(crate) struct FileDrops {
    sender: mpsc::SyncSender<PathBuf>,
    rejected: Arc<AtomicBool>,
}

impl FileDrops {
    pub(crate) fn take(&self, input: &mut egui::RawInput) {
        // Native paths stay in the shell, including hover-only metadata.
        input.hovered_files.clear();
        for file in input.dropped_files.drain(..) {
            let path = file.path();
            let accepted = path
                .extension()
                .filter(|extension| extension.as_bytes().eq_ignore_ascii_case(b"jar"))
                .and_then(|_| validate(path.as_os_str().as_bytes()).ok());
            if accepted.is_none_or(|path| self.sender.try_send(path).is_err()) {
                self.rejected.store(true, Ordering::Release);
            }
        }
    }
}

pub(crate) fn argument(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Option<PathBuf>, EmuError> {
    let Some(mut value) = arguments.next() else {
        return Ok(None);
    };
    let options_ended = value == "--";
    if options_ended {
        let Some(next) = arguments.next() else {
            return Ok(None);
        };
        value = next;
    }
    if arguments.next().is_some() || value.len() > MAX_PATH || value.is_empty() {
        return Err(error());
    }
    let path = if value.as_bytes().starts_with(b"file:") {
        let uri = url::Url::parse(value.to_str().ok_or_else(error)?).map_err(|_| error())?;
        if uri.query().is_some() || uri.fragment().is_some() {
            return Err(error());
        }
        uri.to_file_path().map_err(|()| error())?
    } else {
        if (!options_ended && value.as_bytes().starts_with(b"-"))
            || value.as_bytes().windows(3).any(|part| part == b"://")
        {
            return Err(error());
        }
        std::path::absolute(PathBuf::from(value)).map_err(|_| error())?
    };
    validate(path.as_os_str().as_bytes()).map(Some)
}

fn validate(bytes: &[u8]) -> Result<PathBuf, EmuError> {
    let path = PathBuf::from(OsString::from_vec(bytes.to_vec()));
    if bytes.len() > MAX_PATH || bytes.contains(&0) || !path.is_absolute() {
        return Err(error());
    }
    Ok(path)
}

pub(crate) fn forward(data: &Path, path: Option<&Path>) -> Result<(), EmuError> {
    let directory = std::fs::File::open(data).map_err(|_| error())?;
    let socket = endpoint(&directory);
    let mut stream = (0..20)
        .find_map(|_| {
            let stream = UnixStream::connect(&socket).ok();
            if stream.is_none() {
                thread::sleep(Duration::from_millis(50));
            }
            stream
        })
        .ok_or_else(error)?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(|_| error())?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(|_| error())?;
    let bytes = path.map_or(&[][..], |path| path.as_os_str().as_bytes());
    let length = u32::try_from(bytes.len()).map_err(|_| error())?;
    stream
        .write_all(&length.to_le_bytes())
        .and_then(|()| stream.write_all(bytes))
        .map_err(|_| error())?;
    let mut accepted = [0];
    stream.read_exact(&mut accepted).map_err(|_| error())?;
    if accepted != [1] {
        return Err(error());
    }
    Ok(())
}

pub(crate) struct OpenFiles {
    requests: mpsc::Receiver<PathBuf>,
    drops: FileDrops,
    initial: Option<PathBuf>,
    focus: Arc<AtomicBool>,
    stop: async_channel::Sender<()>,
    thread: Option<JoinHandle<()>>,
    context: Arc<Mutex<Option<egui::Context>>>,
}

impl OpenFiles {
    /// Called only after taking the library lock; stale endpoints then have no owner.
    pub(crate) fn start(data: &Path, initial: Option<PathBuf>) -> Result<Self, EmuError> {
        let socket = data.join("open.sock");
        match std::fs::remove_file(&socket) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(error()),
        }
        // /proc keeps sockaddr_un short even under a deeply nested XDG root.
        let directory = std::fs::File::open(data).map_err(|_| error())?;
        let listener = UnixListener::bind(endpoint(&directory)).map_err(|_| error())?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| error())?;
        let listener = async_io::Async::new(listener).map_err(|_| error())?;
        let (sender, requests) = mpsc::sync_channel(8);
        let drops = FileDrops {
            sender: sender.clone(),
            rejected: Arc::new(AtomicBool::new(false)),
        };
        let (stop, stopped) = async_channel::bounded::<()>(1);
        let focus = Arc::new(AtomicBool::new(false));
        let focused = Arc::clone(&focus);
        let context = Arc::new(Mutex::new(None::<egui::Context>));
        let waker = Arc::clone(&context);
        let thread = thread::Builder::new()
            .name("j2play-file-open".into())
            .spawn(move || {
                futures_lite::future::block_on(futures_lite::future::race(
                    async {
                        let _ = stopped.recv().await;
                    },
                    async {
                        while !stopped.is_closed() {
                            let Ok((mut stream, _)) =
                                listener.read_with(UnixListener::accept).await
                            else {
                                break;
                            };
                            let accepted = receive(&mut stream, &sender).is_ok();
                            if accepted {
                                focused.store(true, Ordering::Release);
                                if let Some(ctx) = waker
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                                    .as_ref()
                                {
                                    ctx.request_repaint();
                                }
                            }
                            let _ = stream.write_all(&[u8::from(accepted)]);
                        }
                    },
                ));
            })
            .map_err(|_| error())?;
        Ok(Self {
            requests,
            drops,
            initial,
            focus,
            stop,
            thread: Some(thread),
            context,
        })
    }

    pub(crate) fn attach(&self, ctx: egui::Context) {
        *self
            .context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ctx);
    }

    pub(crate) fn poll(&mut self) -> Option<PathBuf> {
        self.initial
            .take()
            .or_else(|| self.requests.try_recv().ok())
    }

    pub(crate) fn drops(&self) -> FileDrops {
        self.drops.clone()
    }

    pub(crate) fn poll_drop_error(&self) -> Option<EmuError> {
        self.drops.rejected.swap(false, Ordering::AcqRel).then(|| {
            operation::error(
                "linux-file-drop",
                "Could not import one or more dropped files. Drop local JAR files, at most eight at a time.",
            )
        })
    }

    pub(crate) fn focus(&self, ctx: &egui::Context) {
        if self.focus.swap(false, Ordering::AcqRel) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), EmuError> {
        self.stop.close();
        operation::join(&mut self.thread)
    }
}

impl Drop for OpenFiles {
    fn drop(&mut self) {
        let _ = self.shutdown();
        // The next library-lock owner removes the endpoint. Unlinking during
        // Drop could remove a replacement after the repository released its lock.
    }
}

fn receive(stream: &mut UnixStream, sender: &mpsc::SyncSender<PathBuf>) -> Result<(), EmuError> {
    stream.set_nonblocking(true).map_err(|_| error())?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(|_| error())?;
    let deadline = Instant::now() + TIMEOUT;
    let mut size = [0; 4];
    read_until(stream, &mut size, deadline)?;
    let size = usize::try_from(u32::from_le_bytes(size)).map_err(|_| error())?;
    if size > MAX_PATH {
        return Err(error());
    }
    if size == 0 {
        return Ok(());
    }
    let mut bytes = vec![0; size];
    read_until(stream, &mut bytes, deadline)?;
    sender.try_send(validate(&bytes)?).map_err(|_| error())
}

fn endpoint(directory: &std::fs::File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}/open.sock", directory.as_raw_fd()))
}

fn read_until(
    stream: &mut UnixStream,
    mut buffer: &mut [u8],
    deadline: Instant,
) -> Result<(), EmuError> {
    while !buffer.is_empty() {
        if Instant::now() >= deadline {
            return Err(error());
        }
        match stream.read(buffer) {
            Ok(0) => return Err(error()),
            Ok(count) => buffer = &mut buffer[count..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(error()),
        }
    }
    Ok(())
}

fn error() -> EmuError {
    operation::error(
        "linux-file-open",
        "Could not open the file in J2Play. Choose one local JAR file, or return to the existing window and import it there.",
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-linux/open_file.rs"]
mod tests;
