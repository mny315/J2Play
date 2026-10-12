//! Nonblocking sockets keep cancellation independent of the HTTP deadline.

use super::{CANCEL_POLL, Cancellation};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Instant;
use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, LazyBuffers, NextTimeout, Transport,
};

#[derive(Debug)]
pub(super) struct PinnedResolver(pub Vec<SocketAddr>);

impl Resolver for PinnedResolver {
    fn resolve(
        &self,
        _: &ureq::http::Uri,
        _: &ureq::config::Config,
        _: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let mut result = self.empty();
        for address in self.0.iter().take(16) {
            result.push(*address);
        }
        if result.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(result)
    }
}

#[derive(Debug)]
pub(super) struct CancellableConnector {
    pub cancellation: Cancellation,
    pub deadline: Instant,
}

impl Connector for CancellableConnector {
    type Out = CancellableTransport;

    fn connect(
        &self,
        details: &ConnectionDetails<'_>,
        _: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        let mut deadline = Deadline::new(details.timeout, self.cancellation.clone())?;
        deadline.at = deadline.at.min(self.deadline);
        let mut last = io::Error::from(io::ErrorKind::NotConnected).into();
        for (index, address) in details.addrs.iter().enumerate() {
            deadline.check()?;
            // A silent address must leave time for the other resolved addresses.
            let remaining = deadline.at.saturating_duration_since(Instant::now());
            let count = u32::try_from(details.addrs.len() - index).unwrap_or(u32::MAX);
            let attempt = Deadline {
                at: Instant::now() + remaining / count,
                ..deadline.clone()
            };
            match connect_socket(*address, &attempt) {
                Ok(stream) => {
                    stream.set_nodelay(details.config.no_delay())?;
                    return Ok(Some(CancellableTransport {
                        stream,
                        buffers: LazyBuffers::new(
                            details.config.input_buffer_size(),
                            details.config.output_buffer_size(),
                        ),
                        cancellation: self.cancellation.clone(),
                        deadline: self.deadline,
                        connect_deadline: deadline.at,
                    }));
                }
                Err(error) => last = error,
            }
        }
        deadline.check()?;
        Err(last)
    }
}

fn connect_socket(address: SocketAddr, deadline: &Deadline) -> Result<TcpStream, ureq::Error> {
    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(
        Domain::for_address(address),
        Type::STREAM,
        Some(Protocol::TCP),
    )?;
    socket.set_nonblocking(true)?;
    match socket.connect(&address.into()) {
        Ok(()) => {}
        Err(error) if connect_pending(&error) => loop {
            deadline.check()?;
            if let Some(error) = socket.take_error()? {
                return Err(error.into());
            }
            if socket.peer_addr().is_ok() {
                break;
            }
            deadline.wait()?;
        },
        Err(error) => return Err(error.into()),
    }
    deadline.check()?;
    Ok(socket.into())
}

fn connect_pending(error: &io::Error) -> bool {
    #[cfg(unix)]
    if error.raw_os_error() == Some(libc::EINPROGRESS) {
        return true;
    }
    error.kind() == io::ErrorKind::WouldBlock
}

#[derive(Clone)]
struct Deadline {
    at: Instant,
    reason: ureq::Timeout,
    cancellation: Cancellation,
}

impl Deadline {
    fn new(timeout: NextTimeout, cancellation: Cancellation) -> Result<Self, ureq::Error> {
        let at = Instant::now().checked_add(*timeout.after).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "finite network deadline required",
            )
        })?;
        Ok(Self {
            at,
            reason: timeout.reason,
            cancellation,
        })
    }

    fn check(&self) -> Result<(), ureq::Error> {
        if self.cancellation.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "network request cancelled by host",
            )
            .into());
        }
        if Instant::now() >= self.at {
            return Err(ureq::Error::Timeout(self.reason));
        }
        Ok(())
    }

    fn wait(&self) -> Result<(), ureq::Error> {
        self.check()?;
        std::thread::sleep(CANCEL_POLL.min(self.at.saturating_duration_since(Instant::now())));
        self.check()
    }
}

pub(super) struct CancellableTransport {
    stream: TcpStream,
    buffers: LazyBuffers,
    cancellation: Cancellation,
    deadline: Instant,
    connect_deadline: Instant,
}

impl CancellableTransport {
    fn io_deadline(&self, timeout: NextTimeout) -> Result<Deadline, ureq::Error> {
        let mut deadline = Deadline::new(timeout, self.cancellation.clone())?;
        deadline.at = deadline.at.min(self.deadline);
        // TLS complete_io can perform several socket reads with the same
        // timeout. None of them may restart the connection's time budget.
        if timeout.reason == ureq::Timeout::Connect {
            deadline.at = deadline.at.min(self.connect_deadline);
        }
        Ok(deadline)
    }
}

impl std::fmt::Debug for CancellableTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellableTransport")
            .finish_non_exhaustive()
    }
}

impl Transport for CancellableTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        let deadline = self.io_deadline(timeout)?;
        let mut written = 0;
        while written < amount {
            deadline.check()?;
            match self.stream.write(&self.buffers.output()[written..amount]) {
                Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero).into()),
                Ok(count) => written += count,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => deadline.wait()?,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        }
        deadline.check()
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        let deadline = self.io_deadline(timeout)?;
        loop {
            deadline.check()?;
            match self.stream.read(self.buffers.input_append_buf()) {
                Ok(count) => {
                    self.buffers.input_appended(count);
                    return Ok(count != 0);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => deadline.wait()?,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn is_open(&mut self) -> bool {
        !self.cancellation.is_cancelled()
            && matches!(self.stream.peek(&mut [0]), Err(error) if error.kind() == io::ErrorKind::WouldBlock)
    }
}
