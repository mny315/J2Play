//! Preserve the wire header order and reason phrase required by the guest API.
//! HTTP framing and validation remain the responsibility of ureq.

use crate::{Limits, api_error};
use diagnostics::EmuError;
use std::io;
use std::sync::{Arc, Mutex};
use ureq::unversioned::transport::{Buffers, ConnectionDetails, Connector, NextTimeout, Transport};

#[derive(Debug, Default)]
struct Head {
    bytes: Vec<u8>,
    complete: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ResponseHead(Arc<Mutex<Head>>);

impl ResponseHead {
    fn append(&self, bytes: &[u8]) -> Result<(), ureq::Error> {
        let mut head = self
            .0
            .lock()
            .map_err(|_| io::Error::other("response header lock"))?;
        for byte in bytes {
            if head.complete {
                break;
            }
            if head.bytes.len() >= Limits::default().max_header_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "HTTP headers exceed limit",
                )
                .into());
            }
            head.bytes.push(*byte);
            if head.bytes.ends_with(b"\r\n\r\n") {
                // Interim responses precede the final header block.
                if head.bytes.get(9) == Some(&b'1') && head.bytes.get(9..12) != Some(b"101") {
                    head.bytes.clear();
                } else {
                    head.complete = true;
                }
            }
        }
        Ok(())
    }

    pub(super) fn fields(&self) -> Result<(String, Vec<(String, String)>), EmuError> {
        let head = self
            .0
            .lock()
            .map_err(|_| api_error("network-header", "response header lock"))?;
        let text = std::str::from_utf8(&head.bytes)
            .map_err(|_| api_error("network-header", "HTTP header value is not valid UTF-8"))?;
        let mut lines = text.split("\r\n");
        let reason = lines
            .next()
            .and_then(|line| line.splitn(3, ' ').nth(2))
            .unwrap_or_default()
            .to_owned();
        let mut headers = Vec::new();
        for line in lines.take_while(|line| !line.is_empty()) {
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| api_error("network-header", "invalid HTTP header"))?;
            headers.push((
                name.to_ascii_lowercase(),
                value.trim_matches([' ', '\t']).to_owned(),
            ));
        }
        Ok((reason, headers))
    }
}

impl<In: Transport> Connector<In> for ResponseHead {
    type Out = CaptureTransport<In>;

    fn connect(
        &self,
        _: &ConnectionDetails<'_>,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        let Some(mut inner) = chained else {
            return Err(io::Error::from(io::ErrorKind::NotConnected).into());
        };
        self.append(inner.buffers().input())?;
        Ok(Some(CaptureTransport {
            inner,
            head: self.clone(),
        }))
    }
}

#[derive(Debug)]
pub(super) struct CaptureTransport<T> {
    inner: T,
    head: ResponseHead,
}

impl<T: Transport> Transport for CaptureTransport<T> {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.inner.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        let previous = self.inner.buffers().input().len();
        let progress = self.inner.await_input(timeout)?;
        let input = self.inner.buffers().input();
        let added = input.get(previous..).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP transport lost buffered input",
            )
        })?;
        self.head.append(added)?;
        Ok(progress)
    }

    fn is_open(&mut self) -> bool {
        self.inner.is_open()
    }
    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}
