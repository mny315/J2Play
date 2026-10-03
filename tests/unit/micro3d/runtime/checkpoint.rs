use super::*;

#[test]
fn checkpoint_rejects_undercharged_payloads_exceeding_the_profile_budget() {
    let budget = size_of::<ObjectKind>() + 64;
    let mut runtime = Runtime::new(4, budget, 1, 1).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Texture(TextureData {
                width: 1,
                height: 1,
                pixels: vec![0xff12_3456].into(),
                for_model: true,
                color_key: 0,
            }),
        )
        .unwrap();
    let ObjectKind::Texture(texture) = runtime.kind_mut(1).unwrap() else {
        unreachable!();
    };
    texture.width = 1024;
    texture.pixels = vec![0xff12_3456; 1024].into();
    let encoded = save_state::encode(&runtime).unwrap();
    let mut decoded: Runtime = save_state::decode(&encoded).unwrap();
    assert_eq!(
        decoded
            .validate_checkpoint(4, budget, RenderLimits::default(), |_| true)
            .unwrap_err()
            .code(),
        "checkpoint-arena"
    );
}

#[test]
fn checkpoint_rebuilds_charges_and_preserves_a_large_layout_at_its_exact_budget() {
    let affines = vec![10; 200_000];
    let budget = size_of::<ObjectKind>() + affines.len() * size_of::<u64>();
    let mut runtime = Runtime::new(2, budget, 1, 1).unwrap();
    runtime.create(2, ObjectKind::Graphics).unwrap();
    runtime.dispose(2).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Layout(FigureLayoutState {
                affines,
                ..FigureLayoutState::default()
            }),
        )
        .unwrap();
    // Derived counters cannot be used as the authority for retained allocations.
    runtime.metrics.live_bytes = 0;
    runtime.metrics.live_objects = 0;
    for object in runtime.objects.values_mut() {
        object.bytes = 0;
    }
    let encoded = save_state::encode(&runtime).unwrap();
    let mut decoded: Runtime = save_state::decode(&encoded).unwrap();
    decoded
        .validate_checkpoint(2, budget, RenderLimits::default(), |guest| {
            matches!(guest, 1 | 2 | 10)
        })
        .unwrap();
    assert_eq!(decoded.layout_affines(1).unwrap(), vec![10; 200_000]);
    assert_eq!(decoded.metrics.live_objects, 1);
    assert_eq!(decoded.metrics.live_bytes, budget);
    assert_eq!(decoded.objects[&1].bytes, budget);
    assert_eq!(decoded.objects[&2].bytes, 0);
    assert!(decoded.kind(2).is_err());
    decoded.dispose(1).unwrap();
    assert_eq!(decoded.metrics.live_bytes, 0);
    assert_eq!(decoded.metrics.live_objects, 0);
}

#[test]
fn checkpoint_reaccounts_nested_figure_and_action_buffers() {
    let mut runtime = Runtime::new(8, 1 << 20, 1, 1).unwrap();
    let figure = FigureState {
        data: FigureData {
            format_version: 5,
            uv_bits: 8,
            vertices: vec![Vector3D::default(); 3],
            normals: vec![Vector3D::new(0, 0, 4096); 3],
            faces: vec![Face {
                indices: [0, 1, 2],
                uv: [[0; 2]; 3],
                attributes: 0,
                color: None,
                pattern: 0,
                material: Some(0),
            }],
            bones: vec![Bone {
                vertex_count: 3,
                parent: -1,
                transform: AffineTrans::IDENTITY,
            }],
            pattern_count: 1,
            material_count: 1,
        },
        textures: vec![3],
        selected_texture: 0,
        pattern: 0,
        posture: Some((2, 0, 0)),
    };
    runtime
        .create(1, ObjectKind::Figure(figure.clone()))
        .unwrap();
    runtime
        .create_action(2, &translating_action(), LoaderLimits::default())
        .unwrap();
    runtime
        .create(
            3,
            ObjectKind::Texture(TextureData {
                width: 1,
                height: 1,
                pixels: vec![0xff12_3456].into(),
                for_model: true,
                color_key: 0,
            }),
        )
        .unwrap();
    let expected_bytes = runtime.metrics.live_bytes;
    for object in runtime.objects.values_mut() {
        object.bytes = 0;
    }
    let encoded = save_state::encode(&runtime).unwrap();
    let mut decoded: Runtime = save_state::decode(&encoded).unwrap();
    decoded
        .validate_checkpoint(8, 1 << 20, RenderLimits::default(), |guest| {
            (1..=3).contains(&guest)
        })
        .unwrap();
    assert_eq!(decoded.metrics.live_bytes, expected_bytes);
    assert_eq!(decoded.metrics.live_objects, 3);
    assert_eq!(decoded.kind(1).unwrap(), &ObjectKind::Figure(figure));
    assert_eq!(decoded.kind(2).unwrap(), runtime.kind(2).unwrap());
    for guest in [1, 2, 3] {
        let retained = decoded.objects[&guest].bytes;
        let previous = decoded.metrics.live_bytes;
        decoded.dispose(guest).unwrap();
        assert_eq!(decoded.metrics.live_bytes, previous - retained);
    }
    assert_eq!(decoded.metrics.live_bytes, 0);
}
