use super::*;

#[test]
fn retained_accounting_includes_spare_capacity_at_every_level() {
    let figure = FigureData {
        format_version: 5,
        uv_bits: 8,
        vertices: Vec::with_capacity(2),
        normals: Vec::with_capacity(3),
        faces: Vec::with_capacity(4),
        bones: Vec::with_capacity(5),
        pattern_count: 0,
        material_count: 0,
    };
    assert!(figure.vertices.capacity() > figure.vertices.len());
    assert!(figure.normals.capacity() > figure.normals.len());
    assert_eq!(
        figure.allocated_bytes(),
        figure.vertices.capacity() * size_of::<Vector3D>()
            + figure.normals.capacity() * size_of::<Vector3D>()
            + figure.faces.capacity() * size_of::<Face>()
            + figure.bones.capacity() * size_of::<Bone>()
    );

    let mut translation = Vec::with_capacity(2);
    translation.push(VectorKeyframe {
        frame: 0,
        value: Vector3D::default(),
    });
    let scale = Vec::with_capacity(3);
    let rotation = Vec::with_capacity(4);
    let roll = Vec::with_capacity(5);
    let mut segments = Vec::with_capacity(6);
    segments.push(ActionSegmentData::Components {
        translation,
        scale,
        rotation,
        roll,
    });
    let mut pattern_keys = Vec::with_capacity(7);
    pattern_keys.push([0; 3]);
    let mut actions = Vec::with_capacity(8);
    actions.push(ActionData {
        frame_count: 1,
        segments,
        pattern_keys,
    });
    let table = ActionTableData {
        format_version: 5,
        frame_counts: Vec::with_capacity(9),
        actions,
    };
    let action = &table.actions[0];
    let ActionSegmentData::Components {
        translation,
        scale,
        rotation,
        roll,
    } = &action.segments[0]
    else {
        unreachable!();
    };
    assert!(table.actions.capacity() > table.actions.len());
    assert!(action.segments.capacity() > action.segments.len());
    let expected = table.frame_counts.capacity() * size_of::<u16>()
        + table.actions.capacity() * size_of::<ActionData>()
        + action.segments.capacity() * size_of::<ActionSegmentData>()
        + action.pattern_keys.capacity() * size_of::<[u16; 3]>()
        + translation.capacity() * size_of::<VectorKeyframe>()
        + scale.capacity() * size_of::<VectorKeyframe>()
        + rotation.capacity() * size_of::<VectorKeyframe>()
        + roll.capacity() * size_of::<ScalarKeyframe>();
    assert_eq!(table.allocated_bytes(), expected);

    let texture = TextureData {
        width: 3,
        height: 1,
        pixels: Arc::<[u32]>::from([1_u32, 2, 3]),
        for_model: false,
        color_key: 0,
    };
    assert_eq!(texture.allocated_bytes(), 3 * size_of::<u32>());
}

#[test]
fn malformed_resources_fail_before_allocation() {
    let limits = LoaderLimits::default();
    assert_eq!(
        FigureData::parse(b"MB", limits).unwrap_err().code(),
        "resource-truncated"
    );
    assert_eq!(
        TextureData::parse(b"BM", true, limits).unwrap_err().code(),
        "resource-truncated"
    );
    let tiny = LoaderLimits {
        file_bytes: 1,
        ..limits
    };
    assert_eq!(
        ActionTableData::parse(b"MT\x05\0", tiny)
            .unwrap_err()
            .code(),
        "resource-budget"
    );
}
