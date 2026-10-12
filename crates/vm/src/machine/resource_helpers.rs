use super::{EmuError, vm_error};

pub(super) fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub(super) fn data_input_utf_length(value: i32) -> Result<usize, EmuError> {
    u16::try_from(value).map(usize::from).map_err(|_| {
        vm_error(
            "invalid-stream-read",
            format!("DataInput.readUnsignedShort returned {value}; expected 0..=65535"),
        )
    })
}

pub(super) fn data_input_utf_byte(value: i32) -> Result<u8, EmuError> {
    u8::try_from(value).map_err(|_| {
        vm_error(
            "invalid-stream-read",
            format!("DataInput.readUnsignedByte returned {value}; expected 0..=255"),
        )
    })
}

pub(super) fn decode_data_input_utf(bytes: &[u8]) -> Result<Vec<u16>, ()> {
    let mut units = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let first = bytes[index];
        // CLDC readUTF groups bytes by their prefix bits. It accepts literal
        // NUL and does not require the shortest encoding of a UTF-16 unit.
        if first <= 0x7f {
            units.push(u16::from(first));
            index += 1;
        } else if first & 0xe0 == 0xc0 {
            let second = bytes.get(index + 1).copied().ok_or(())?;
            if second & 0xc0 != 0x80 {
                return Err(());
            }
            let unit = (u16::from(first & 0x1f) << 6) | u16::from(second & 0x3f);
            units.push(unit);
            index += 2;
        } else if first & 0xf0 == 0xe0 {
            let second = bytes.get(index + 1).copied().ok_or(())?;
            let third = bytes.get(index + 2).copied().ok_or(())?;
            if second & 0xc0 != 0x80 || third & 0xc0 != 0x80 {
                return Err(());
            }
            let unit = (u16::from(first & 0x0f) << 12)
                | (u16::from(second & 0x3f) << 6)
                | u16::from(third & 0x3f);
            units.push(unit);
            index += 3;
        } else {
            return Err(());
        }
    }
    Ok(units)
}
