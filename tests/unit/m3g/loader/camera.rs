use super::*;

fn camera_file(kind: u8, parameters: [f32; 4]) -> Vec<u8> {
    let mut data = valid_group_data();
    data.truncate(data.len() - 4); // Camera inherits Node without Group children.
    data.push(kind);
    for value in parameters {
        data.extend_from_slice(&value.to_le_bytes());
    }
    file_with_sections(false, &[section(&object(5, &data), false)])
}

#[test]
fn serialized_cameras_preserve_reversed_equal_and_parallel_negative_clip_distances() {
    for (kind, parameters) in [
        (49, [4.0, 2.0, -2.0, 3.0]),
        (49, [4.0, 2.0, 3.0, -2.0]),
        (49, [4.0, 2.0, 0.0, 0.0]),
        (50, [60.0, 2.0, 10.0, 1.0]),
        (50, [60.0, 2.0, 1.0, 1.0]),
    ] {
        let file = M3gFile::parse(&camera_file(kind, parameters), LoaderLimits::default()).unwrap();
        let mut runtime = crate::Runtime::default();
        let loaded = crate::instantiate_file(&file, &mut runtime, &[None; 2], &[None; 2]).unwrap();
        let crate::ObjectKind::Camera { projection, .. } =
            runtime.kind(loaded.handles[1].unwrap()).unwrap()
        else {
            unreachable!()
        };
        let expected = if kind == 49 {
            crate::CameraProjection::Parallel {
                height: parameters[0],
                aspect_ratio: parameters[1],
                near: parameters[2],
                far: parameters[3],
            }
        } else {
            crate::CameraProjection::Perspective {
                field_of_view: parameters[0],
                aspect_ratio: parameters[1],
                near: parameters[2],
                far: parameters[3],
            }
        };
        assert_eq!(*projection, expected);
    }
}

#[test]
fn serialized_cameras_reject_out_of_range_field_of_view_and_dimensions() {
    for (kind, parameters) in [
        (49, [0.0, 2.0, 1.0, 10.0]),
        (49, [4.0, 0.0, 1.0, 10.0]),
        (50, [0.0, 2.0, 1.0, 10.0]),
        (50, [180.0, 2.0, 1.0, 10.0]),
        (50, [270.0, 2.0, 1.0, 10.0]),
        (50, [60.0, 0.0, 1.0, 10.0]),
        (50, [60.0, 2.0, 0.0, 10.0]),
        (50, [60.0, 2.0, 1.0, 0.0]),
    ] {
        let error =
            M3gFile::parse(&camera_file(kind, parameters), LoaderLimits::default()).unwrap_err();
        assert_eq!(error.code(), "invalid-projection", "{kind}, {parameters:?}");
    }
}
