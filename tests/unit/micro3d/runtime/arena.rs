use super::*;

#[test]
fn replacement_reaccounts_dynamic_state_transactionally() {
    let base_bytes = size_of::<ObjectKind>();
    let mut textures = Vec::with_capacity(5);
    textures.extend([1, 2]);
    let texture_capacity = textures.capacity();
    let figure = ObjectKind::Figure(FigureState {
        data: FigureData {
            format_version: 5,
            uv_bits: 8,
            vertices: Vec::new(),
            normals: Vec::new(),
            faces: Vec::new(),
            bones: Vec::new(),
            pattern_count: 0,
            material_count: 0,
        },
        textures,
        selected_texture: 0,
        pattern: 0,
        posture: None,
    });
    assert!(texture_capacity > 2);
    assert_eq!(
        estimated_bytes(&figure),
        base_bytes + texture_capacity * size_of::<u64>()
    );
    let spare_affines = Vec::with_capacity(6);
    let affine_capacity = spare_affines.capacity();
    assert_eq!(
        estimated_bytes(&ObjectKind::Layout(FigureLayoutState {
            affines: spare_affines,
            ..FigureLayoutState::default()
        })),
        base_bytes + affine_capacity * size_of::<u64>()
    );

    let mut runtime = Runtime::new(4, base_bytes + size_of::<u64>(), 8, 8).unwrap();
    runtime
        .create(1, ObjectKind::Layout(FigureLayoutState::default()))
        .unwrap();
    assert_eq!(runtime.metrics().live_bytes, base_bytes);

    let spare_only = Vec::with_capacity(2);
    assert_eq!(
        runtime
            .set_layout_affines(1, spare_only)
            .unwrap_err()
            .code(),
        "resource-limit"
    );
    let too_large = vec![10, 11];
    assert_eq!(
        runtime.set_layout_affines(1, too_large).unwrap_err().code(),
        "resource-limit"
    );
    assert_eq!(
        runtime.kind(1).unwrap(),
        &ObjectKind::Layout(FigureLayoutState::default())
    );
    assert_eq!(runtime.metrics().live_bytes, base_bytes);

    runtime.set_layout_affines(1, vec![10]).unwrap();
    let ObjectKind::Layout(exact) = runtime.kind(1).unwrap() else {
        unreachable!();
    };
    assert_eq!(exact.affines, [10]);
    assert_eq!(runtime.metrics().live_bytes, base_bytes + size_of::<u64>());
    let (snapshot, selected) = runtime.layout_render_snapshot(1).unwrap();
    assert!(snapshot.affines.is_empty());
    assert_eq!(selected, Some(10));
    assert_eq!(runtime.layout_affine(1, 0).unwrap(), Some(10));

    runtime.set_layout_affines(1, Vec::new()).unwrap();
    assert_eq!(runtime.metrics().live_bytes, base_bytes);
    assert_eq!(runtime.metrics().peak_bytes, base_bytes + size_of::<u64>());

    let empty_figure = ObjectKind::Figure(FigureState {
        data: FigureData {
            format_version: 5,
            uv_bits: 8,
            vertices: Vec::new(),
            normals: Vec::new(),
            faces: Vec::new(),
            bones: Vec::new(),
            pattern_count: 0,
            material_count: 0,
        },
        textures: Vec::new(),
        selected_texture: 0,
        pattern: 0,
        posture: None,
    });
    let mut figure_runtime = Runtime::new(1, base_bytes + size_of::<u64>(), 8, 8).unwrap();
    figure_runtime.create(2, empty_figure).unwrap();
    let mut spare_textures = Vec::with_capacity(2);
    spare_textures.push(20);
    assert_eq!(
        figure_runtime
            .set_figure_textures(2, spare_textures)
            .unwrap_err()
            .code(),
        "resource-limit"
    );
    let ObjectKind::Figure(unchanged) = figure_runtime.kind(2).unwrap() else {
        unreachable!();
    };
    assert!(unchanged.textures.is_empty());
    figure_runtime.set_figure_textures(2, vec![20]).unwrap();
    let ObjectKind::Figure(updated) = figure_runtime.kind(2).unwrap() else {
        unreachable!();
    };
    assert_eq!(updated.textures, [20]);
    assert_eq!(
        figure_runtime.metrics().live_bytes,
        base_bytes + size_of::<u64>()
    );
}

#[test]
fn arena_is_suite_scoped_bounded_and_sweepable() {
    let mut runtime = Runtime::new(2, 4096, 8, 8).unwrap();
    runtime
        .create(1, ObjectKind::Light(LightState::default()))
        .unwrap();
    runtime
        .create(
            2,
            ObjectKind::Effect(EffectState {
                light: Some(1),
                ..EffectState::default()
            }),
        )
        .unwrap();
    assert_eq!(runtime.guest_closure([2]), [1, 2]);
    runtime.sweep_guest_objects(|guest| guest == 2);
    assert!(runtime.kind(1).is_err());
    assert!(runtime.kind(2).is_ok());
}

#[test]
fn disposed_slots_do_not_consume_the_live_object_budget() {
    let mut runtime = Runtime::new(1, 4096, 8, 8).unwrap();
    runtime
        .create(1, ObjectKind::Light(LightState::default()))
        .unwrap();
    runtime.dispose(1).unwrap();
    runtime
        .create(2, ObjectKind::Light(LightState::default()))
        .unwrap();

    assert_eq!(runtime.metrics().live_objects, 1);
    assert!(runtime.kind(1).is_err());
    assert!(runtime.kind(2).is_ok());
}

#[test]
fn sweeping_visits_live_and_disposed_wrappers_in_order_and_releases_each_charge_once() {
    let mut runtime = Runtime::new(8, 4096, 1, 1).unwrap();
    for guest in [5, 1, 4, 3, 2, 6] {
        runtime.create(guest, ObjectKind::Graphics).unwrap();
    }
    runtime.dispose(2).unwrap();
    runtime.dispose(6).unwrap();
    let mut visited = Vec::new();
    runtime.sweep_guest_objects(|guest| {
        visited.push(guest);
        guest % 3 == 0
    });
    assert_eq!(visited, [1, 2, 3, 4, 5, 6]);
    assert_eq!(runtime.objects.len(), 2);
    assert_eq!(runtime.metrics().live_objects, 1);
    assert_eq!(runtime.metrics().live_bytes, size_of::<ObjectKind>());
    assert!(runtime.kind(3).is_ok());
    assert!(runtime.kind(6).is_err());
    runtime.sweep_guest_objects(|_| false);
    assert!(runtime.objects.is_empty());
    assert_eq!(runtime.metrics().live_objects, 0);
    assert_eq!(runtime.metrics().live_bytes, 0);
}

#[test]
fn disposed_wrapper_history_is_bounded_independently_of_the_live_object_limit() {
    let mut runtime = Runtime::new(1, 3 * size_of::<ObjectKind>(), 1, 1).unwrap();
    for guest in 1..=3 {
        runtime.create(guest, ObjectKind::Graphics).unwrap();
        runtime.dispose(guest).unwrap();
    }
    assert_eq!(runtime.metrics().live_objects, 0);
    assert_eq!(runtime.metrics().live_bytes, 0);
    assert_eq!(
        runtime.create(4, ObjectKind::Graphics).unwrap_err().code(),
        "resource-limit"
    );
    assert_eq!(runtime.objects.len(), 3);
    runtime.dispose(1).unwrap();
    runtime.sweep_guest_objects(|guest| guest == 2);
    assert_eq!(runtime.objects.len(), 1);
    runtime.create(4, ObjectKind::Graphics).unwrap();
    assert_eq!(runtime.metrics().live_objects, 1);
}
