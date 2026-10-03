//! Single HTTP exchanges and bounded DNS workers behind injectable host traits.

use crate::{api_error, api_source, platform_source};
use diagnostics::EmuError;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

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
        receiver
            .recv_timeout(timeout)
            .map_err(|_| api_error("network-timeout", "DNS resolution timed out"))?
            .map_err(|error| platform_source("network-dns", "host resolution failed", error))
    }
}

#[derive(Default)]
pub struct UreqTransport;

impl HttpTransport for UreqTransport {
    fn execute(
        &mut self,
        request: &TransportRequest,
        max_response: usize,
    ) -> Result<TransportResponse, EmuError> {
        let agent = ureq::AgentBuilder::new()
            // Per-read timeouts alone let a server keep the VM blocked by
            // trickling one byte before every deadline, indefinitely.
            .timeout(request.total_timeout)
            .timeout_connect(request.connect_timeout)
            .timeout_read(request.read_timeout)
            .timeout_write(request.read_timeout)
            .redirects(0)
            .try_proxy_from_env(false)
            .resolver({
                let addresses = request.addresses.clone();
                move |_: &str| Ok(addresses.clone())
            })
            .build();
        let mut call = agent.request(&request.method, &request.url);
        for (name, value) in &request.headers {
            call = call.set(name, value);
        }
        let result = if request.body.is_empty() {
            call.call()
        } else {
            call.send_bytes(&request.body)
        };
        let response = match result {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(error) => return Err(api_source("network-io", "HTTP exchange failed", error)),
        };
        let status = response.status();
        let reason = response.status_text().to_owned();
        let headers = response_headers(&response)?;
        let mut body = Vec::new();
        response
            .into_reader()
            .take(
                u64::try_from(max_response)
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_to_end(&mut body)
            .map_err(|error| api_source("network-io", "cannot read HTTP response", error))?;
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

fn response_headers(response: &ureq::Response) -> Result<Vec<(String, String)>, EmuError> {
    let mut values = std::collections::HashMap::new();
    response
        .headers_names()
        .into_iter()
        .map(|name| {
            // names includes every occurrence; header(name) always returns the first.
            let group = values
                .entry(name.clone())
                .or_insert_with(|| response.all(&name).into_iter());
            let value = group.next().ok_or_else(|| {
                api_error("network-header", "HTTP header value is not valid UTF-8")
            })?;
            Ok((name, value.to_owned()))
        })
        .collect()
}
