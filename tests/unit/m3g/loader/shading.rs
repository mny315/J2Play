use super::*;

#[test]
fn serialized_lights_preserve_negative_intensity() {
    for intensity in [-0.25_f32, 0.0, 1.0] {
        let mut data = valid_group_data();
        data.truncate(data.len() - 4); // Node without Group children.
        for value in [1.0_f32, 0.0, 0.0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        data.extend_from_slice(&[255, 255, 255, 129]); // White directional light.
        for value in [intensity, 45.0, 0.0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        for compressed in [false, true] {
            let bytes = file_with_sections(false, &[section(&object(12, &data), compressed)]);
            let file = M3gFile::parse(&bytes, LoaderLimits::default()).unwrap();
            let mut runtime = crate::Runtime::default();
            let loaded =
                crate::instantiate_file(&file, &mut runtime, &[None; 2], &[None; 2]).unwrap();
            let crate::ObjectKind::Light { light, .. } =
                runtime.kind(loaded.handles[1].unwrap()).unwrap()
            else {
                unreachable!()
            };
            assert_eq!(light.intensity.to_bits(), intensity.to_bits());
        }
    }
}

#[test]
fn serialized_linear_fog_preserves_either_order_and_equal_distances() {
    for [near, far] in [[3.0_f32, 1.0], [-2.0, 3.0], [3.0, -2.0], [1.0, 1.0]] {
        let mut data = vec![0; 12]; // Object3D metadata.
        data.extend_from_slice(&[0, 0, 255, 81]); // Blue linear fog.
        for value in [near, far] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        for compressed in [false, true] {
            let bytes = file_with_sections(false, &[section(&object(7, &data), compressed)]);
            let file = M3gFile::parse(&bytes, LoaderLimits::default()).unwrap();
            let mut runtime = crate::Runtime::default();
            let loaded =
                crate::instantiate_file(&file, &mut runtime, &[None; 2], &[None; 2]).unwrap();
            let crate::ObjectKind::Fog(fog) = runtime.kind(loaded.handles[1].unwrap()).unwrap()
            else {
                unreachable!()
            };
            assert_eq!(
                [fog.near.to_bits(), fog.far.to_bits()],
                [near.to_bits(), far.to_bits()]
            );
            if near.to_bits() != far.to_bits() {
                assert_eq!(fog.apply(0xffff_0000, f64::from(near)), 0xffff_0000);
                assert_eq!(fog.apply(0xffff_0000, f64::from(far)), 0xff00_00ff);
            }
        }
    }
}

#[test]
fn serialized_texture_filters_distinguish_mipmap_and_image_modes() {
    let mut image = vec![0; 12];
    image.extend_from_slice(&[99, 1]); // Mutable RGB, 2 by 2.
    image.extend_from_slice(&2_u32.to_le_bytes());
    image.extend_from_slice(&2_u32.to_le_bytes());
    for level in 207..=211 {
        for filter in 207..=211 {
            let mut texture = vec![0; 14]; // Object3D and Transformable.
            texture.extend_from_slice(&2_u32.to_le_bytes());
            texture.extend_from_slice(&[0, 0, 0, 227, 241, 241, level, filter]);
            let content = [object(10, &image), object(17, &texture)].concat();
            for compressed in [false, true] {
                let bytes = file_with_sections(false, &[section(&content, compressed)]);
                let parsed = M3gFile::parse(&bytes, LoaderLimits::default());
                if matches!(level, 208..=210) && matches!(filter, 209 | 210) {
                    let mut runtime = crate::Runtime::default();
                    let loaded = crate::instantiate_file(
                        &parsed.unwrap(),
                        &mut runtime,
                        &[None; 3],
                        &[None; 3],
                    )
                    .unwrap();
                    let crate::ObjectKind::Texture2D(state) =
                        runtime.kind(loaded.handles[2].unwrap()).unwrap()
                    else {
                        unreachable!()
                    };
                    assert_eq!(
                        (state.level_filter, state.image_filter),
                        (i32::from(level), i32::from(filter))
                    );
                } else {
                    assert_eq!(parsed.unwrap_err().code(), "invalid-enum");
                }
            }
        }
    }
}
