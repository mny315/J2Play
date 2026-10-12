//! Shared test-only HTTP exchanges and DNS answers; no socket is opened.

use super::{HttpTransport, Resolver, TransportRequest, TransportResponse};
use diagnostics::{Category, EmuError};
use std::collections::VecDeque;
use std::net::IpAddr;
use std::time::Duration;

pub struct MockExchange {
    pub expected_url: String,
    pub outcome: Result<TransportResponse, EmuError>,
}

#[derive(Default)]
pub struct MockTransport {
    exchanges: VecDeque<MockExchange>,
}

impl MockTransport {
    #[must_use]
    pub fn new(exchanges: impl IntoIterator<Item = MockExchange>) -> Self {
        Self {
            exchanges: exchanges.into_iter().collect(),
        }
    }
}

impl HttpTransport for MockTransport {
    fn execute(
        &mut self,
        request: &TransportRequest,
        _: usize,
    ) -> Result<TransportResponse, EmuError> {
        let exchange = self.exchanges.pop_front().ok_or_else(|| {
            EmuError::new(
                Category::Api,
                "network-mock",
                "unexpected mock HTTP request",
            )
        })?;
        if exchange.expected_url != request.url {
            return Err(EmuError::new(
                Category::Api,
                "network-mock",
                format!(
                    "expected {}, received {}",
                    exchange.expected_url, request.url
                ),
            ));
        }
        exchange.outcome
    }
}

pub struct FixedResolver {
    pub addresses: Vec<IpAddr>,
}

impl Resolver for FixedResolver {
    fn resolve(&mut self, _: &str, _: u16, _: Duration) -> Result<Vec<IpAddr>, EmuError> {
        Ok(self.addresses.clone())
    }
}
