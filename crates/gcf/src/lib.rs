//! Bounded Generic Connection Framework host runtime.
//!
//! The runtime checks suite permissions before resolving a network name or
//! touching the filesystem. HTTP redirects are revalidated as fresh requests;
//! file URLs are resolved below one suite root and symbolic links are rejected.

use diagnostics::{Category, EmuError};

pub const HTTP_PERMISSION: &str = "javax.microedition.io.Connector.http";
pub const HTTPS_PERMISSION: &str = "javax.microedition.io.Connector.https";
pub const FILE_READ_PERMISSION: &str = "javax.microedition.io.Connector.file.read";
pub const FILE_WRITE_PERMISSION: &str = "javax.microedition.io.Connector.file.write";

const LEGACY_CMWAP_PROXY_HOST: &str = "10.0.0.172";
const LEGACY_WAP_TARGET_HEADER: &str = "X-Online-Host";

mod runtime;

pub use runtime::*;

#[cfg(test)]
use runtime::{ParsedHttpUrl, is_public_address};

fn api_error(code: &'static str, message: impl Into<String>) -> EmuError {
    EmuError::new(Category::Api, code, message)
}
fn api_source(
    code: &'static str,
    message: impl Into<String>,
    source: impl std::error::Error + Send + Sync + 'static,
) -> EmuError {
    EmuError::with_source(Category::Api, code, message, source)
}
fn platform_source(
    code: &'static str,
    message: impl Into<String>,
    source: impl std::error::Error + Send + Sync + 'static,
) -> EmuError {
    EmuError::with_source(Category::Platform, code, message, source)
}

#[cfg(test)]
#[path = "../../../tests/unit/gcf/mod.rs"]
mod tests;
