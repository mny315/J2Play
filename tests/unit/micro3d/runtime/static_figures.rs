use super::*;

fn static_triangle(with_skeleton: bool) -> Vec<u8> {
    let mut bytes = b"MB\x03\0".to_vec();
    for value in [3_u16, 1, 0, u16::from(with_skeleton)] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for vertex in [[-6_i16, -6, 0], [6, -6, 0], [0, 6, 0]] {
        for value in vertex {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for value in [FIGURE_ATTR_DOUBLE_FACE as u16, 0, 1, 2, 0, 0, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    if with_skeleton {
        bytes.extend_from_slice(&3_u16.to_le_bytes());
        bytes.extend_from_slice(&(-1_i16).to_le_bytes());
        for value in AffineTrans::IDENTITY.values {
            bytes.extend_from_slice(&i16::try_from(value).unwrap().to_le_bytes());
        }
    }
    bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");
    bytes
}

#[test]
fn loaded_static_figure_without_bones_preserves_its_geometry() {
    let mut images = Vec::new();
    for with_skeleton in [true, false] {
        let mut runtime = Runtime::new(4, 1 << 20, 32, 32).unwrap();
        runtime
            .create_figure(1, &static_triangle(with_skeleton), LoaderLimits::default())
            .unwrap();
        runtime
            .load_target(32, 32, &vec![0xff00_0000; 32 * 32])
            .unwrap();
        runtime
            .render_figure(
                1,
                0,
                0,
                FigureLayoutState {
                    center: [16, 16],
                    scale: [4096, 4096],
                    ..FigureLayoutState::default()
                },
                AffineTrans::rotation_z(512),
                EffectState::default(),
            )
            .unwrap();
        assert!(runtime.target_pixels().contains(&0xffff_ffff));
        images.push(runtime.target_pixels().to_vec());
    }
    assert_eq!(images[0], images[1]);
}

#[test]
fn incomplete_skeleton_still_rejects_uncovered_vertices() {
    let mut figure = FigureData::parse(&static_triangle(true), LoaderLimits::default()).unwrap();
    figure.bones[0].vertex_count = 2;
    assert_eq!(
        figure_geometry(&figure, None, false).unwrap_err().code(),
        "figure-bone-coverage"
    );
}
