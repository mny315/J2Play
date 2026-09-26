//! HTTP request policy, redirect handling and bounded URL/header validation.

use super::{Limits, Runtime, TransportRequest, TransportResponse};
use crate::{
    HTTP_PERMISSION, HTTPS_PERMISSION, LEGACY_CMWAP_PROXY_HOST, LEGACY_WAP_TARGET_HEADER,
    api_error, api_source,
};
use diagnostics::EmuError;
use natives::{GcfHttpRequest, GcfHttpResponse};
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

impl Runtime {
    /// Executes a bounded request after permission, offline and SSRF checks.
    ///
    /// # Errors
    /// Returns a policy, validation, DNS, timeout, transport or size-limit diagnostic.
    pub fn http(&mut self, request: GcfHttpRequest) -> Result<GcfHttpResponse, EmuError> {
        let started = Instant::now();
        let budget = self
            .limits
            .connect_timeout
            .saturating_add(self.limits.read_timeout);
        validate_request(&request, &self.limits)?;
        if let Some(target) = legacy_wap_target(&request, self.limits.max_url_bytes)? {
            self.permissions.require(HTTP_PERMISSION)?;
            if self.offline {
                return Err(api_error(
                    "network-offline",
                    "network is disabled by host policy",
                ));
            }
            return Err(api_error(
                "connection-not-found",
                format!(
                    "legacy CMWAP proxy {LEGACY_CMWAP_PROXY_HOST}:80 is unavailable; target={target}"
                ),
            ));
        }
        let mut current = TransportRequest {
            url: request.url,
            method: request.method,
            headers: request.headers,
            body: request.body,
            connect_timeout: self.limits.connect_timeout,
            read_timeout: self.limits.read_timeout,
            total_timeout: budget,
            addresses: Vec::new(),
        };
        let mut parsed = ParsedHttpUrl::parse(&current.url, self.limits.max_url_bytes)?;
        for redirect in 0..=self.limits.max_redirects {
            self.permissions.require(parsed.permission())?;
            if self.offline {
                return Err(api_error(
                    "network-offline",
                    "network is disabled by host policy",
                ));
            }
            let remaining = remaining_http_budget(started, budget)?;
            let addresses = self.resolver.resolve(
                &parsed.host,
                parsed.port,
                self.limits.connect_timeout.min(remaining),
            )?;
            if addresses.is_empty() || addresses.iter().any(|address| !is_public_address(*address))
            {
                return Err(api_error(
                    "network-ssrf",
                    "network target resolves to a non-public address",
                ));
            }
            current.addresses = addresses
                .into_iter()
                .map(|address| SocketAddr::new(address, parsed.port))
                .collect();
            current.total_timeout = remaining_http_budget(started, budget)?;
            let response = self
                .transport
                .execute(&current, self.limits.max_response_bytes)?;
            remaining_http_budget(started, budget)?;
            validate_response(&response, &self.limits)?;
            let Some(location) = redirect_location(&response) else {
                return Ok(GcfHttpResponse {
                    status: i32::from(response.status),
                    reason: response.reason,
                    headers: response.headers,
                    body: response.body,
                });
            };
            if redirect == self.limits.max_redirects {
                return Err(api_error(
                    "network-redirect",
                    "HTTP redirect limit exceeded",
                ));
            }
            let next_url = resolve_redirect(&current.url, location, self.limits.max_url_bytes)?;
            let next = ParsedHttpUrl::parse(&next_url, self.limits.max_url_bytes)?;
            if !parsed.same_origin(&next) {
                current
                    .headers
                    .retain(|(name, _)| !matches_sensitive_redirect_header(name));
            }
            current.url = next_url;
            parsed = next;
            // HEAD remains a metadata-only retrieval after a 303 (RFC 9110, 15.4.4).
            if (response.status == 303 && current.method != "HEAD")
                || ((response.status == 301 || response.status == 302) && current.method == "POST")
            {
                "GET".clone_into(&mut current.method);
                current.body.clear();
                current
                    .headers
                    .retain(|(name, _)| !matches_entity_header(name));
            }
        }
        Err(api_error(
            "network-redirect",
            "HTTP redirect processing ended without a response",
        ))
    }
}

fn remaining_http_budget(started: Instant, budget: Duration) -> Result<Duration, EmuError> {
    budget
        .checked_sub(started.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| api_error("network-timeout", "HTTP request deadline exceeded"))
}

pub(crate) struct ParsedHttpUrl {
    scheme: &'static str,
    host: String,
    port: u16,
    origin: String,
}

impl ParsedHttpUrl {
    pub(crate) fn parse(value: &str, max_bytes: usize) -> Result<Self, EmuError> {
        if value.len() > max_bytes || value.chars().any(char::is_control) {
            return Err(api_error(
                "network-url",
                "URL is invalid or exceeds configured limit",
            ));
        }
        let (scheme, remainder) = value
            .split_once("://")
            .ok_or_else(|| api_error("network-url", "absolute HTTP URL required"))?;
        let scheme = if scheme.eq_ignore_ascii_case("http") {
            "http"
        } else if scheme.eq_ignore_ascii_case("https") {
            "https"
        } else {
            return Err(api_error(
                "connection-not-found",
                "unsupported GCF protocol",
            ));
        };
        let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
        let authority = &remainder[..authority_end];
        if authority.is_empty() || authority.contains('@') {
            return Err(api_error("network-url", "URL authority is invalid"));
        }
        let default_port = if scheme == "https" { 443 } else { 80 };
        let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
            let end = bracketed
                .find(']')
                .ok_or_else(|| api_error("network-url", "invalid IPv6 authority"))?;
            let host = &bracketed[..end];
            let suffix = &bracketed[end + 1..];
            let port = if suffix.is_empty() {
                default_port
            } else {
                parse_port(
                    suffix
                        .strip_prefix(':')
                        .ok_or_else(|| api_error("network-url", "invalid IPv6 port"))?,
                )?
            };
            (host, port)
        } else if let Some((host, port)) = authority.rsplit_once(':') {
            if host.contains(':') {
                return Err(api_error("network-url", "IPv6 host must use brackets"));
            }
            (host, parse_port(port)?)
        } else {
            (authority, default_port)
        };
        if host.is_empty()
            || !host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b':'))
        {
            return Err(api_error("network-url", "URL host is invalid"));
        }
        let origin = format!("{scheme}://{authority}");
        Ok(Self {
            scheme,
            host: host.to_ascii_lowercase(),
            port,
            origin,
        })
    }
    fn permission(&self) -> &'static str {
        if self.scheme == "https" {
            HTTPS_PERMISSION
        } else {
            HTTP_PERMISSION
        }
    }

    fn same_origin(&self, other: &Self) -> bool {
        self.scheme == other.scheme && self.host == other.host && self.port == other.port
    }
}

/// Returns a bounded, credential-free destination suitable for frontend
/// diagnostics. Legacy CMWAP requests report their `X-Online-Host` target
/// rather than the obsolete private proxy address.
#[must_use]
pub fn http_request_destination(request: &GcfHttpRequest) -> Option<String> {
    if let Ok(Some(target)) = legacy_wap_target(request, Limits::default().max_url_bytes) {
        return Some(target);
    }
    ParsedHttpUrl::parse(&request.url, Limits::default().max_url_bytes)
        .ok()
        .map(|parsed| parsed.origin)
}

fn legacy_wap_target(
    request: &GcfHttpRequest,
    max_url_bytes: usize,
) -> Result<Option<String>, EmuError> {
    let mut targets = request
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(LEGACY_WAP_TARGET_HEADER))
        .map(|(_, value)| value.trim());
    let Some(target_authority) = targets.next() else {
        return Ok(None);
    };
    let proxy = ParsedHttpUrl::parse(&request.url, max_url_bytes)?;
    if proxy.scheme != "http" || proxy.host != LEGACY_CMWAP_PROXY_HOST || proxy.port != 80 {
        return Ok(None);
    }
    if targets.next().is_some() {
        return Err(api_error(
            "network-header",
            "multiple X-Online-Host headers are ambiguous",
        ));
    }
    if target_authority.is_empty()
        || target_authority.contains(['/', '?', '#', '@'])
        || target_authority.chars().any(char::is_control)
    {
        return Err(api_error(
            "network-header",
            "X-Online-Host must contain one HTTP authority",
        ));
    }

    let direct_url = format!("http://{target_authority}/");
    let target = ParsedHttpUrl::parse(&direct_url, max_url_bytes).map_err(|_| {
        api_error(
            "network-header",
            "X-Online-Host does not contain a valid HTTP authority",
        )
    })?;
    Ok(Some(target.origin))
}

fn matches_sensitive_redirect_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("authorization")
        || name.eq_ignore_ascii_case("proxy-authorization")
        || name.eq_ignore_ascii_case("cookie")
}

fn matches_entity_header(name: &str) -> bool {
    [
        "content-encoding",
        "content-language",
        "content-length",
        "content-location",
        "content-type",
        "transfer-encoding",
    ]
    .iter()
    .any(|value| name.eq_ignore_ascii_case(value))
}

fn parse_port(value: &str) -> Result<u16, EmuError> {
    value
        .parse::<u16>()
        .ok()
        .filter(|value| *value != 0)
        .ok_or_else(|| api_error("network-url", "URL port is invalid"))
}

pub(crate) fn is_public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => {
            let [first, second, third, _] = value.octets();
            !(value.is_private()
                || value.is_loopback()
                || value.is_link_local()
                || value.is_multicast()
                || first == 0
                || (first == 100 && (64..=127).contains(&second))
                || (first == 192 && second == 0 && third == 0)
                || (first == 192 && second == 0 && third == 2)
                || (first == 198 && (second == 18 || second == 19))
                || (first == 198 && second == 51 && third == 100)
                || (first == 203 && second == 0 && third == 113)
                || first >= 240)
        }
        IpAddr::V6(value) => {
            if let Some(mapped) = value.to_ipv4_mapped() {
                return is_public_address(IpAddr::V4(mapped));
            }
            match value.segments() {
                [0x2001, 0 | 0x0db8, ..] => false,
                [0x2002, high, low, ..] => {
                    let [first, second] = high.to_be_bytes();
                    let [third, fourth] = low.to_be_bytes();
                    is_public_address(IpAddr::V4(std::net::Ipv4Addr::new(
                        first, second, third, fourth,
                    )))
                }
                // This prefix check also excludes unspecified, loopback,
                // multicast, unique-local and link/site-local addresses.
                [first, ..] => (first & 0xe000) == 0x2000,
            }
        }
    }
}

fn validate_request(request: &GcfHttpRequest, limits: &Limits) -> Result<(), EmuError> {
    if !matches!(request.method.as_str(), "GET" | "HEAD" | "POST") {
        return Err(api_error(
            "network-method",
            "only GET, HEAD and POST are supported",
        ));
    }
    if request.body.len() > limits.max_request_bytes {
        return Err(api_error(
            "network-limit",
            "HTTP request body exceeds configured limit",
        ));
    }
    validate_headers(&request.headers, limits)
}

fn validate_response(response: &TransportResponse, limits: &Limits) -> Result<(), EmuError> {
    if response.body.len() > limits.max_response_bytes {
        return Err(api_error(
            "network-limit",
            "HTTP response exceeds configured limit",
        ));
    }
    validate_headers(&response.headers, limits)
}

pub(super) fn validate_headers(
    headers: &[(String, String)],
    limits: &Limits,
) -> Result<(), EmuError> {
    if headers.len() > limits.max_headers {
        return Err(api_error("network-limit", "too many HTTP headers"));
    }
    let mut bytes = 0usize;
    for (name, value) in headers {
        bytes = bytes.saturating_add(name.len()).saturating_add(value.len());
        if name.is_empty()
            || !name.bytes().all(is_header_name_byte)
            || value.chars().any(|value| value == '\r' || value == '\n')
        {
            return Err(api_error("network-header", "invalid HTTP header"));
        }
    }
    if bytes > limits.max_header_bytes {
        return Err(api_error(
            "network-limit",
            "HTTP headers exceed configured limit",
        ));
    }
    Ok(())
}

fn is_header_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn redirect_location(response: &TransportResponse) -> Option<&str> {
    matches!(response.status, 301 | 302 | 303 | 307 | 308)
        .then(|| {
            response
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("location"))
                .map(|(_, value)| value.as_str())
        })
        .flatten()
}

fn resolve_redirect(base_url: &str, location: &str, max_bytes: usize) -> Result<String, EmuError> {
    if location.len() > max_bytes || location.chars().any(char::is_control) {
        return Err(api_error(
            "network-redirect",
            "redirect Location is invalid or exceeds configured limit",
        ));
    }
    // Reuse the transport's URL parser; constructing a request performs no I/O.
    let base = ureq::get(base_url)
        .request_url()
        .map_err(|error| api_source("network-url", "invalid redirect base URL", error))?;
    let mut result = base
        .as_url()
        .join(location)
        .map_err(|error| api_source("network-redirect", "invalid redirect Location", error))?;
    if result.fragment().is_none() {
        result.set_fragment(base.as_url().fragment());
    }
    Ok(result.into())
}
