use super::*;

fn texture(width: u32, height: u32) -> Texture2DState {
    let pixels = (0..width * height)
        .map(|index| 0xff00_0000 | index)
        .collect::<Vec<_>>();
    Texture2DState::new(Image2DState::from_argb(ImageFormat::Rgba, width, height, &pixels).unwrap())
}

fn restored(texture: &Texture2DState) -> Texture2DState {
    save_state::decode(&save_state::encode(texture).unwrap()).unwrap()
}

#[test]
fn checkpoint_rejects_inconsistent_identity_transform_cache() {
    for transform in [Mat4::IDENTITY, Mat4::translation(0.5, 0.0, 0.0).unwrap()] {
        let mut texture = texture(2, 2);
        texture.set_transform(transform);
        texture.identity_transform = !texture.identity_transform;
        assert_eq!(
            restored(&texture).validate_checkpoint().unwrap_err().code(),
            "checkpoint-texture"
        );
    }
}

#[test]
fn checkpoint_rejects_incomplete_and_repeated_mip_levels() {
    for corruption in 0..5 {
        let mut texture = texture(4, 2);
        texture.set_mipmap_filter(true, false);
        match corruption {
            0 => texture.mipmaps.truncate(1),
            1 => texture.mipmaps.truncate(2),
            2 => {
                texture.set_mipmap_filter(false, false);
                texture.mipmaps.truncate(2);
            }
            3 => texture
                .mipmaps
                .push(texture.mipmaps.last().unwrap().clone()),
            _ => texture.mipmaps.clear(),
        }
        assert_eq!(
            restored(&texture).validate_checkpoint().unwrap_err().code(),
            "checkpoint-texture",
            "corruption {corruption}"
        );
    }
}

#[test]
fn checkpoint_rejects_dimensions_unavailable_to_source_images() {
    for (width, height) in [(1_025, 1), (1, 1_025), (0, 1), (1, 0)] {
        let mut texture = texture(1, 1);
        texture.width = width;
        texture.height = height;
        texture.mipmaps[0] = MipLevel {
            width,
            height,
            pixels: vec![0; (width * height) as usize].into(),
        };
        assert_eq!(
            restored(&texture).validate_checkpoint().unwrap_err().code(),
            "checkpoint-texture"
        );
    }
}

#[test]
fn checkpoint_preserves_filter_and_color_key_transitions() {
    for (width, height) in [(1, 1), (4, 2), (3, 7), (1_024, 1), (1, 1_024)] {
        let mut texture = texture(width, height);
        texture.set_blend_function(BlendFunction::Replace);
        for transform in [Mat4::IDENTITY, Mat4::translation(0.5, -0.25, 0.0).unwrap()] {
            texture.set_transform(transform);
            for enabled in [false, true, false, true] {
                for linear in [false, true] {
                    texture.set_mipmap_filter(enabled, linear);
                    texture.set_linear_filter(linear);
                    for color_key in [None, Some(0), Some(1), None] {
                        texture.set_color_key(color_key);
                        let restored = restored(&texture);
                        restored.validate_checkpoint().unwrap();
                        assert_eq!(restored, texture);
                        for lod in [0.0, 0.5, 1.0, 20.0] {
                            assert_eq!(
                                restored.shade_coordinates_lod([0.25, 0.75, 0.0], u32::MAX, lod),
                                texture.shade_coordinates_lod([0.25, 0.75, 0.0], u32::MAX, lod)
                            );
                        }
                    }
                }
            }
        }
    }
}
