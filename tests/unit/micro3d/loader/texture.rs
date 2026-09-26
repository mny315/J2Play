use super::*;

fn palette_bmp(declared_size: u32, pixel_offset: u32) -> Vec<u8> {
    let mut bytes = vec![0_u8; 66];
    bytes[0..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&declared_size.to_le_bytes());
    bytes[10..14].copy_from_slice(&pixel_offset.to_le_bytes());
    bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&1_i32.to_le_bytes());
    bytes[22..26].copy_from_slice(&1_i32.to_le_bytes());
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&8_u16.to_le_bytes());
    bytes[34..38].copy_from_slice(&4_u32.to_le_bytes());
    bytes[46..50].copy_from_slice(&2_u32.to_le_bytes());
    bytes[54..58].copy_from_slice(&[255, 0, 0, 0]);
    bytes[58..62].copy_from_slice(&[255, 255, 255, 0]);
    bytes
}

#[test]
fn model_texture_preserves_palette_zero_until_color_key_rendering() {
    let bytes = palette_bmp(66, 62);

    let model = TextureData::parse(&bytes, true, LoaderLimits::default()).unwrap();
    let primitive = TextureData::parse(
        &bytes,
        false,
        LoaderLimits {
            decoded_bytes: size_of::<u32>(),
            ..LoaderLimits::default()
        },
    )
    .unwrap();
    assert_eq!(model.pixels.as_ref(), [0xff00_00ff]);
    assert_eq!(model.color_key, 0x0000_00ff);
    assert_eq!(primitive.pixels.as_ref(), [0xff00_00ff]);
    assert_eq!(primitive.color_key, 0x0000_00ff);
    assert_eq!(model.allocated_bytes(), size_of::<u32>());
    assert_eq!(
        TextureData::parse(
            &bytes,
            true,
            LoaderLimits {
                decoded_bytes: size_of::<u32>() - 1,
                ..LoaderLimits::default()
            }
        )
        .unwrap_err()
        .code(),
        "resource-budget"
    );
}

#[test]
fn texture_accepts_legacy_header_offset_before_appended_palette() {
    let mut bytes = palette_bmp(58, 54);

    let texture = TextureData::parse(&bytes, false, LoaderLimits::default()).unwrap();
    assert_eq!(texture.pixels.as_ref(), [0xff00_00ff]);
    assert_eq!(texture.color_key, 0x0000_00ff);

    bytes[2..6].copy_from_slice(&66_u32.to_le_bytes());
    assert_eq!(
        TextureData::parse(&bytes, false, LoaderLimits::default())
            .unwrap_err()
            .code(),
        "texture-palette"
    );
}

#[test]
fn texture_decoded_budget_is_checked_before_reading_palette_and_pixels() {
    let mut bytes = vec![0_u8; 54];
    bytes[..2].copy_from_slice(b"BM");
    bytes[10..14].copy_from_slice(&58_u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&1024_i32.to_le_bytes());
    bytes[22..26].copy_from_slice(&1024_i32.to_le_bytes());
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&8_u16.to_le_bytes());
    assert_eq!(
        TextureData::parse(
            &bytes,
            true,
            LoaderLimits {
                decoded_bytes: 4096,
                ..LoaderLimits::default()
            }
        )
        .unwrap_err()
        .code(),
        "resource-budget"
    );
}

#[test]
fn texture_rejects_invalid_dib_sizes_without_overflowing_the_header_offset() {
    for dib_size in [0, 39, u32::MAX - 13, u32::MAX] {
        let mut bytes = palette_bmp(66, u32::MAX);
        bytes[14..18].copy_from_slice(&dib_size.to_le_bytes());
        assert_eq!(
            TextureData::parse(&bytes, false, LoaderLimits::default())
                .unwrap_err()
                .code(),
            "texture-header",
            "dib_size={dib_size}"
        );
    }
}
