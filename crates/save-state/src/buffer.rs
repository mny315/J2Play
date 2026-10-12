//! Bounded in-memory encoding without routing every scalar through `io::Write`.

use super::{EmuError, MAX_COMPONENT_BYTES, Serialize, error};
use postcard::{Error, ser_flavors::Flavor};

const CANCEL_INTERVAL: usize = 64 * 1024;

pub(super) fn encode<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EmuError> {
    if cancelled() {
        return Err(error(
            "checkpoint-cancelled",
            "Automatic saving was interrupted.",
        ));
    }
    postcard::serialize_with_flavor(
        value,
        Buffer {
            bytes: Vec::new(),
            limit: limit.min(MAX_COMPONENT_BYTES),
            checked: 0,
            cancelled,
        },
    )
    .map_err(|_| {
        error(
            "checkpoint-encode",
            "Automatic saving exceeded its time or size limit, or could not write its data.",
        )
    })
}

struct Buffer<'a> {
    bytes: Vec<u8>,
    limit: usize,
    checked: usize,
    cancelled: &'a dyn Fn() -> bool,
}

impl Buffer<'_> {
    #[inline]
    fn check(&mut self, length: usize) -> Result<(), Error> {
        if length > self.limit.saturating_sub(self.bytes.len()) {
            return Err(Error::SerializeBufferFull);
        }
        if self.bytes.len() - self.checked >= CANCEL_INTERVAL {
            if (self.cancelled)() {
                return Err(Error::SerializeBufferFull);
            }
            self.checked = self.bytes.len();
        }
        let required = self.bytes.len() + length;
        if required > self.bytes.capacity() {
            // Vec's implicit doubling can exceed a non-power-of-two owner
            // limit even when the encoded length fits. Bound the allocation
            // as well, and turn allocation failure into an encoding error.
            let capacity = self
                .bytes
                .capacity()
                .saturating_mul(2)
                .max(8)
                .max(required)
                .min(self.limit);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|_| Error::SerializeBufferFull)?;
        }
        Ok(())
    }
}

impl Flavor for Buffer<'_> {
    type Output = Vec<u8>;

    #[inline]
    fn try_push(&mut self, byte: u8) -> Result<(), Error> {
        self.check(1)?;
        self.bytes.push(byte);
        Ok(())
    }

    #[inline]
    fn try_extend(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.check(bytes.len())?;
        // Scalars take the common single-copy path. Large binary components
        // still poll cancellation between bounded chunks.
        if bytes.len() <= CANCEL_INTERVAL {
            self.bytes.extend_from_slice(bytes);
        } else {
            for chunk in bytes.chunks(CANCEL_INTERVAL) {
                self.check(chunk.len())?;
                self.bytes.extend_from_slice(chunk);
            }
        }
        Ok(())
    }

    fn finalize(self) -> Result<Self::Output, Error> {
        Ok(self.bytes)
    }
}
