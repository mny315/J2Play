//! Bounded subprocess capture for test targets; never linked into a product.

use std::io::{self, Read};
use std::process::{Child, Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const MAX_CAPTURE_BYTES: usize = 1024 * 1024;

pub struct TestOutput {
    pub output: Output,
    pub timed_out: bool,
}

/// On timeout or an I/O error, kill and reap the child before joining readers.
pub fn run(command: &mut Command, timeout: Duration) -> io::Result<TestOutput> {
    let mut running = RunningChild {
        child: command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
        stdout: None,
        stderr: None,
    };
    let started = Instant::now();
    running.stdout = Some(capture(running.child.stdout.take().unwrap())?);
    running.stderr = Some(capture(running.child.stderr.take().unwrap())?);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = running.child.try_wait()? {
            break status;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            // The process may have exited between try_wait and kill.
            let _ = running.child.kill();
            break running.child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = join_capture(running.stdout.take().unwrap())?;
    let stderr = join_capture(running.stderr.take().unwrap())?;
    Ok(TestOutput {
        output: Output {
            status,
            stdout,
            stderr,
        },
        timed_out,
    })
}

type Capture = JoinHandle<io::Result<Vec<u8>>>;

struct RunningChild {
    child: Child,
    stdout: Option<Capture>,
    stderr: Option<Capture>,
}

impl Drop for RunningChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        for reader in [self.stdout.take(), self.stderr.take()]
            .into_iter()
            .flatten()
        {
            let _ = reader.join();
        }
    }
}

fn join_capture(reader: Capture) -> io::Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| io::Error::other("test output reader panicked"))?
}

fn capture(mut stream: impl Read + Send + 'static) -> io::Result<Capture> {
    std::thread::Builder::new()
        .name("test-output".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let count = match stream.read(&mut buffer) {
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    other => other?,
                };
                if count == 0 {
                    return Ok(bytes);
                }
                // Retain one excess byte to report overflow, then keep draining
                // both pipes so excessive output cannot deadlock the child.
                let keep = count.min((MAX_CAPTURE_BYTES + 1).saturating_sub(bytes.len()));
                bytes.extend_from_slice(&buffer[..keep]);
            }
        })
}
