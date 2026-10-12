use super::*;

#[test]
fn serialized_background_crop_rejects_negative_dimensions_but_keeps_signed_origins() {
    for [width, height] in [
        [0_i32, 0],
        [1, 2],
        [i32::MAX, i32::MAX],
        [-1, 2],
        [1, i32::MIN],
    ] {
        let mut data = vec![0; 20]; // Object3D, color, null image.
        data.extend_from_slice(&[32, 32]);
        for value in [i32::MIN, i32::MAX, width, height] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        data.extend_from_slice(&[1, 1]);
        for compressed in [false, true] {
            let bytes = file_with_sections(false, &[section(&object(4, &data), compressed)]);
            let parsed = M3gFile::parse(&bytes, LoaderLimits::default());
            if width < 0 || height < 0 {
                assert_eq!(parsed.unwrap_err().code(), "invalid-crop");
            } else {
                let mut runtime = crate::Runtime::default();
                let loaded =
                    crate::instantiate_file(&parsed.unwrap(), &mut runtime, &[None; 2], &[None; 2])
                        .unwrap();
                let crate::ObjectKind::Background(state) =
                    runtime.kind(loaded.handles[1].unwrap()).unwrap()
                else {
                    unreachable!()
                };
                assert_eq!(state.crop, [i32::MIN, i32::MAX, width, height]);
            }
        }
    }
}
