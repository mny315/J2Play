use super::*;
use natives::GcfHttpRequest;
use std::cell::RefCell;
use std::fs;
use std::net::IpAddr;
use std::rc::Rc;
use std::time::Duration;

#[path = "../../support/storage.rs"]
mod storage;
use storage::Scratch;

#[path = "../../support/gcf_transport.rs"]
mod gcf_transport;
use gcf_transport::{FixedResolver, MockExchange, MockTransport};

mod file;
mod file_bounds;
mod http_policy;
mod http_redirects;
mod http_transport;

struct CountingTransport {
    calls: Rc<RefCell<usize>>,
    responses: Vec<TransportResponse>,
}
impl HttpTransport for CountingTransport {
    fn execute(&mut self, _: &TransportRequest, _: usize) -> Result<TransportResponse, EmuError> {
        *self.calls.borrow_mut() += 1;
        Ok(self.responses.remove(0))
    }
}
fn response(status: u16, headers: Vec<(String, String)>, body: &[u8]) -> TransportResponse {
    TransportResponse {
        status,
        reason: "test".to_owned(),
        headers,
        body: body.to_vec(),
    }
}
fn runtime(
    allowed: &[&str],
    calls: Rc<RefCell<usize>>,
    responses: Vec<TransportResponse>,
    addresses: Vec<IpAddr>,
    offline: bool,
) -> (Runtime, Scratch) {
    let directory = Scratch::new();
    let runtime = Runtime::new(
        PermissionPolicy::new(allowed.iter().map(|value| (*value).to_owned())),
        Box::new(CountingTransport { calls, responses }),
        Box::new(FixedResolver { addresses }),
        &directory.0,
        Limits::default(),
        offline,
    )
    .unwrap();
    (runtime, directory)
}
