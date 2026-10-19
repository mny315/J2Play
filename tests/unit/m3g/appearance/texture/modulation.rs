use super::*;

#[test]
fn modulation_keeps_channel_products_independent_at_every_byte_pair() {
    for a in 0..=u8::MAX {
        for b in 0..=u8::MAX {
            let texture = [a, a.wrapping_mul(13), !a, a.rotate_left(3)];
            let fragment = [b, !b, b.wrapping_mul(31), b.rotate_right(1)];
            let reference = std::array::from_fn(|index| {
                ((u16::from(texture[index]) * u16::from(fragment[index]) + 127) / 255) as u8
            });
            let expected = u32::from_be_bytes(reference);
            let texture = u32::from_be_bytes(texture);
            let fragment = u32::from_be_bytes(fragment);
            assert_eq!(modulate_argb(texture, fragment), expected);
            assert_eq!(modulate_rgb(texture, fragment), expected & 0x00ff_ffff);
        }
    }
}
