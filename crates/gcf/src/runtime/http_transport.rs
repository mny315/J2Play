//! Single HTTP exchanges and bounded DNS workers behind injectable host traits.

use super::Cancellation;
use super::cancellation::CANCEL_POLL;
use crate::{api_error, api_source, platform_source};
use diagnostics::EmuError;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

mod cancellable;
mod response_head;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    /// Remaining budget for this exchange, including the complete response body.
    pub total_timeout: Duration,
    /// Policy-approved addresses pinned for the transport exchange.
    pub addresses: Vec<SocketAddr>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportResponse {
    pub status: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub trait HttpTransport {
    /// Performs exactly one HTTP exchange and never follows redirects.
    ///
    /// # Errors
    /// Returns a bounded transport diagnostic.
    fn execute(
        &mut self,
        request: &TransportRequest,
        max_response: usize,
    ) -> Result<TransportResponse, EmuError>;

    /// Executes one exchange, honouring host cancellation when supported.
    ///
    /// # Errors
    /// Returns cancellation, transport or response-limit errors.
    fn execute_cancellable(
        &mut self,
        request: &TransportRequest,
        max_response: usize,
        cancellation: &Cancellation,
    ) -> Result<TransportResponse, EmuError> {
        cancellation.check()?;
        self.execute(request, max_response)
    }
}

pub trait Resolver {
    /// Resolves all addresses for a host so policy can reject local targets.
    ///
    /// # Errors
    /// Returns a DNS or deterministic mock diagnostic.
    fn resolve(
        &mut self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<Vec<IpAddr>, EmuError>;

    /// Resolves a host with an optional cancellation-aware implementation.
    ///
    /// # Errors
    /// Returns cancellation, DNS or timeout errors.
    fn resolve_cancellable(
        &mut self,
        host: &str,
        port: u16,
        timeout: Duration,
        cancellation: &Cancellation,
    ) -> Result<Vec<IpAddr>, EmuError> {
        cancellation.check()?;
        self.resolve(host, port, timeout)
    }
}

pub struct SystemResolver {
    pub(crate) in_flight: Arc<AtomicUsize>,
    pub(crate) max_in_flight: usize,
}

impl Default for SystemResolver {
    fn default() -> Self {
        // A timed-out OS resolver may still be blocked. New launch attempts
        // must share its budget instead of creating four more threads each.
        static IN_FLIGHT: OnceLock<Arc<AtomicUsize>> = OnceLock::new();
        Self {
            in_flight: Arc::clone(IN_FLIGHT.get_or_init(|| Arc::new(AtomicUsize::new(0)))),
            max_in_flight: 4,
        }
    }
}

struct ResolverSlot(Arc<AtomicUsize>);

impl Drop for ResolverSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Resolver for SystemResolver {
    fn resolve(
        &mut self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<Vec<IpAddr>, EmuError> {
        self.resolve_cancellable(host, port, timeout, &Cancellation::default())
    }

    fn resolve_cancellable(
        &mut self,
        host: &str,
        port: u16,
        timeout: Duration,
        cancellation: &Cancellation,
    ) -> Result<Vec<IpAddr>, EmuError> {
        cancellation.check()?;
        let started = Instant::now();
        self.in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < self.max_in_flight).then(|| current + 1)
            })
            .map_err(|_| api_error("network-limit", "too many unfinished DNS resolutions"))?;
        let slot = ResolverSlot(self.in_flight.clone());
        let host = host.to_owned();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let _resolver_thread = std::thread::Builder::new()
            .name("j2play-dns".to_owned())
            .spawn(move || {
                let _slot = slot;
                let result = (host.as_str(), port)
                    .to_socket_addrs()
                    .map(|values| values.map(|value| value.ip()).collect::<Vec<_>>());
                let _ignored = sender.send(result);
            })
            .map_err(|error| platform_source("network-dns", "cannot start DNS resolver", error))?;
        receive_dns(&receiver, started, timeout, cancellation)
    }
}

fn receive_dns(
    receiver: &std::sync::mpsc::Receiver<std::io::Result<Vec<IpAddr>>>,
    started: Instant,
    timeout: Duration,
    cancellation: &Cancellation,
) -> Result<Vec<IpAddr>, EmuError> {
    loop {
        cancellation.check()?;
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(api_error("network-timeout", "DNS resolution timed out"));
        }
        match receiver.recv_timeout(remaining.min(CANCEL_POLL)) {
            Ok(result) => {
                return result.map_err(|error| {
                    platform_source("network-dns", "host resolution failed", error)
                });
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(api_error("network-dns", "DNS resolver stopped"));
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/gcf/dns_cancellation.rs"]
mod dns_cancellation_tests;

#[derive(Default)]
pub struct UreqTransport;

impl HttpTransport for UreqTransport {
    fn execute(
        &mut self,
        request: &TransportRequest,
        max_response: usize,
    ) -> Result<TransportResponse, EmuError> {
        self.execute_cancellable(request, max_response, &Cancellation::default())
    }

    fn execute_cancellable(
        &mut self,
        request: &TransportRequest,
        max_response: usize,
        cancellation: &Cancellation,
    ) -> Result<TransportResponse, EmuError> {
        use ureq::unversioned::transport::{Connector, RustlsConnector};
        cancellation.check()?;
        let deadline = Instant::now()
            .checked_add(request.total_timeout)
            .ok_or_else(|| api_error("network-timeout", "invalid HTTP deadline"))?;
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(request.total_timeout))
            .timeout_connect(Some(request.connect_timeout))
            .timeout_send_request(Some(request.read_timeout))
            .timeout_send_body(Some(request.read_timeout))
            .timeout_recv_response(Some(request.read_timeout))
            .timeout_recv_body(Some(request.read_timeout))
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .max_response_header_size(super::Limits::default().max_header_bytes)
            .max_idle_connections(0)
            .proxy(None)
            .build();
        let head = response_head::ResponseHead::default();
        let connector = cancellable::CancellableConnector {
            cancellation: cancellation.clone(),
            deadline,
        }
        .chain(RustlsConnector::default())
        .chain(head.clone());
        let agent = ureq::Agent::with_parts(
            config,
            connector,
            cancellable::PinnedResolver(request.addresses.clone()),
        );
        let mut call = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(&request.url);
        for (name, value) in &request.headers {
            call = call.header(name, value);
        }
        let call = call
            .body(request.body.as_slice())
            .map_err(|error| api_source("network-header", "invalid HTTP request", error))?;
        let mut response = agent
            .run(call)
            .map_err(|error| api_source("network-io", "HTTP exchange failed", error))?;
        cancellation.check()?;
        let status = response.status().as_u16();
        let (reason, headers) = head.fields()?;
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(
                u64::try_from(max_response)
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_to_end(&mut body)
            .map_err(|error| api_source("network-io", "cannot read HTTP response", error))?;
        cancellation.check()?;
        if body.len() > max_response {
            return Err(api_error(
                "network-limit",
                "HTTP response exceeds configured limit",
            ));
        }
        Ok(TransportResponse {
            status,
            reason,
            headers,
            body,
        })
    }
}
