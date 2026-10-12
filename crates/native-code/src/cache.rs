//! Authenticated, bounded derived artifacts in host-owned app-private storage.
//! This directory and its authentication key must be outside every guest file
//! root. A cache is trusted like the installed runtime, never like an input JAR.
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use vm::acceleration::IntegerMethodSpec;

const MAGIC: &[u8] = b"J2PLAY-NUMERIC-AOT-1\0";
const MAX_CODE_BYTES: usize = 128 * 1_024;
const MAX_CACHE_BYTES: u64 = 16 * 1_024 * 1_024;
const MAX_CACHE_FILES: usize = 1_024;

pub(crate) struct Cache {
    root: PathBuf,
    key: [u8; 32],
    identity_prefix: Sha256,
    usage: Option<CacheUsage>,
}

#[derive(Clone, Copy)]
struct CacheUsage {
    bytes: u64,
    files: usize,
}

impl Cache {
    pub fn open(root: &Path, target: &str) -> std::io::Result<Self> {
        fs::create_dir_all(root)?;
        if !fs::symlink_metadata(root)?.is_dir() {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        }
        let key_path = root.join("authentication-key");
        let mut key = [0; 32];
        match private_file(&key_path) {
            Ok(mut file) => {
                getrandom::fill(&mut key)
                    .map_err(|error| std::io::Error::other(error.to_string()))?;
                file.write_all(&key)?;
                file.sync_all()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if !fs::symlink_metadata(&key_path)?.is_file()
                    || fs::metadata(&key_path)?.len() != 32
                {
                    return Err(std::io::ErrorKind::InvalidData.into());
                }
                fs::File::open(key_path)?.read_exact(&mut key)?;
            }
            Err(error) => return Err(error),
        }
        let mut bytes = 0_u64;
        let mut files = 0;
        for entry in fs::read_dir(root)?.take(MAX_CACHE_FILES + 1) {
            let metadata = entry?.metadata()?;
            bytes = bytes.saturating_add(metadata.len());
            files += 1;
        }
        let mut identity_prefix = Sha256::new();
        identity_prefix.update(MAGIC);
        identity_prefix.update(b"cranelift-0.135.1;numeric-abi-2;speed;");
        identity_prefix.update(include_bytes!("compiler.rs"));
        identity_prefix.update(include_bytes!("verify.rs"));
        identity_prefix.update(include_bytes!("executable.rs"));
        identity_prefix.update(target.as_bytes());
        Ok(Self {
            root: root.to_owned(),
            key,
            identity_prefix,
            usage: Some(CacheUsage { bytes, files }),
        })
    }

    pub fn identity(&self, spec: &IntegerMethodSpec) -> [u8; 32] {
        let mut hash = self.identity_prefix.clone();
        hash.update([spec.max_locals, spec.max_stack]);
        hash.update(
            u32::try_from(spec.parameter_slots.len())
                .expect("verified parameter bound")
                .to_le_bytes(),
        );
        hash.update(&spec.parameter_slots);
        hash.update(
            u32::try_from(spec.static_integer_parameters.len())
                .expect("verified parameter bound")
                .to_le_bytes(),
        );
        for &(field, slot) in &spec.static_integer_parameters {
            hash.update(field.to_le_bytes());
            hash.update([slot]);
        }
        hash.update(
            u32::try_from(spec.code.len())
                .expect("verified code bound")
                .to_le_bytes(),
        );
        hash.update(&spec.code);
        for &(index, value) in &spec.integer_constants {
            hash.update(index.to_le_bytes());
            hash.update(value.to_le_bytes());
        }
        hash.finalize().into()
    }

    fn path(&self, identity: &[u8; 32]) -> PathBuf {
        use std::fmt::Write as _;
        let mut name = identity
            .iter()
            .fold(String::with_capacity(71), |mut name, byte| {
                let _ = write!(name, "{byte:02x}");
                name
            });
        name.push_str(".native");
        self.root.join(name)
    }

    pub fn load(&self, identity: &[u8; 32]) -> Option<Vec<u8>> {
        let path = self.path(identity);
        let metadata = fs::symlink_metadata(&path).ok()?;
        if !metadata.is_file() || metadata.len() > u64::try_from(MAX_CODE_BYTES + 32).ok()? {
            return None;
        }
        let mut file = fs::File::open(path).ok()?;
        let mut tag = [0; 32];
        file.read_exact(&mut tag).ok()?;
        let mut code = Vec::new();
        file.take((MAX_CODE_BYTES + 1) as u64)
            .read_to_end(&mut code)
            .ok()?;
        if code.is_empty() || code.len() > MAX_CODE_BYTES {
            return None;
        }
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.key).ok()?;
        mac.update(MAGIC);
        mac.update(identity);
        mac.update(&code);
        mac.verify_slice(&tag).ok()?;
        Some(code)
    }

    pub fn store(&mut self, identity: &[u8; 32], code: &[u8]) -> std::io::Result<()> {
        let Some(mut usage) = self.usage else {
            return Ok(());
        };
        if code.is_empty() || code.len() > MAX_CODE_BYTES || usage.files > MAX_CACHE_FILES {
            return Ok(());
        }
        let path = self.path(identity);
        let temporary = path.with_extension("pending");
        // Successful publication replaces both an older artifact and any
        // pending file left by an interrupted write before this cache opened.
        for replaced in [&path, &temporary] {
            match fs::symlink_metadata(replaced) {
                Ok(metadata) => {
                    usage.bytes = usage.bytes.saturating_sub(metadata.len());
                    usage.files = usage.files.saturating_sub(1);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        usage.bytes = usage.bytes.saturating_add(code.len() as u64 + 32);
        usage.files = usage.files.saturating_add(1);
        if usage.files > MAX_CACHE_FILES || usage.bytes > MAX_CACHE_BYTES {
            return Ok(());
        }
        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(&self.key).map_err(std::io::Error::other)?;
        mac.update(MAGIC);
        mac.update(identity);
        mac.update(code);
        let tag = mac.finalize().into_bytes();
        // A failed write can leave partial pending bytes. Keep reads available,
        // but require a fresh directory measurement before any further writes.
        self.usage = None;
        match fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut file = private_file(&temporary)?;
        file.write_all(&tag)?;
        file.write_all(code)?;
        file.sync_all()?;
        fs::rename(temporary, path)?;
        self.usage = Some(usage);
        Ok(())
    }
}

fn private_file(path: &Path) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
#[path = "../../../tests/unit/native-code/cache/mod.rs"]
mod tests;
