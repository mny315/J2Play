use super::*;

#[test]
fn version_five_pattern_tables_label_material_and_visibility_groups() {
    let placeholder = Face {
        indices: [0, 1, 2],
        uv: [[0, 0]; 3],
        attributes: 0,
        color: None,
        pattern: 0,
        material: Some(0),
    };
    let mut faces = vec![placeholder; 9];
    let bytes = [1_u16, 0, 1, 1, 0, 1, 0, 1, 1, 0, 0, 0]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let patterns = parse_patterns(&mut Cursor::new(&bytes), 2, 2).unwrap();

    validate_pattern_polygon_counts(&patterns, 2, 2, 1, 1).unwrap();
    annotate_pattern_materials(&mut faces, &patterns, 2, 2, 1, 1).unwrap();

    assert_eq!(
        faces
            .iter()
            .map(|face| (face.pattern, face.material))
            .collect::<Vec<_>>(),
        [
            (0, Some(0)),
            (1, Some(0)),
            (0, Some(0)),
            (0, Some(0)),
            (0, Some(1)),
            (0, Some(1)),
            (0, None),
            (1, None),
            (1, None),
        ]
    );
}

#[test]
fn pattern_tables_reject_truncation_and_header_count_mismatches() {
    let bytes = [0; 24];
    for length in 0..bytes.len() {
        assert_eq!(
            parse_patterns(&mut Cursor::new(&bytes[..length]), 2, 2)
                .unwrap_err()
                .code(),
            "resource-truncated"
        );
    }
    let mut cursor = Cursor::new(&bytes);
    let patterns = parse_patterns(&mut cursor, 2, 2).unwrap();
    assert_eq!(cursor.position(), bytes.len());
    validate_pattern_polygon_counts(&patterns, 0, 0, 0, 0).unwrap();
    for counts in [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]] {
        assert_eq!(
            validate_pattern_polygon_counts(&patterns, counts[0], counts[1], counts[2], counts[3])
                .unwrap_err()
                .code(),
            "figure-pattern-count"
        );
    }
    let empty = parse_patterns(&mut Cursor::new(&[]), 0, u16::MAX.into()).unwrap();
    validate_pattern_polygon_counts(&empty, 0, 0, 0, 0).unwrap();
    annotate_pattern_materials(&mut [], &empty, 0, 0, 0, 0).unwrap();
}
