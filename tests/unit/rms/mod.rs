use super::*;
#[path = "../../support/storage.rs"]
mod storage;
use storage::Scratch;

pub(crate) fn repository() -> (Scratch, Repository, SuiteId) {
    let root = Scratch::new();
    let repository = Repository::new(&root.0, Limits::default());
    let suite = SuiteId::new("J2Play", "Fixture").unwrap();
    (root, repository, suite)
}

pub(crate) fn refresh_checksum(bytes: &mut [u8]) {
    let payload_length = bytes.len() - CHECKSUM_BYTES;
    let checksum = Sha256::digest(&bytes[..payload_length]);
    bytes[payload_length..].copy_from_slice(&checksum);
}
