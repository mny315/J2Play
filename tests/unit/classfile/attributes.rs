use super::*;

#[test]
fn rejects_empty_code_attribute() {
    let body = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut materialization = MaterializationBudget::for_input(body.len());
    assert_eq!(
        parse_code(1, &body, 20, &[None], &mut materialization)
            .unwrap_err()
            .code(),
        "code-limit"
    );
}

#[test]
fn repeated_long_attribute_names_are_materialization_bounded() {
    let name = "x".repeat(1_024);
    let pool = vec![None, Some(Constant::Utf8(name.clone()))];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&128_u16.to_be_bytes());
    for _ in 0..128 {
        bytes.extend_from_slice(&1_u16.to_be_bytes());
        bytes.extend_from_slice(&0_u32.to_be_bytes());
    }
    let mut reader = Reader::new(&bytes);
    let mut materialization =
        MaterializationBudget::for_input(bytes.len().saturating_add(name.len()));

    assert_eq!(
        parse_attributes(
            &mut reader,
            &pool,
            AttributeLocation::Class,
            &mut materialization,
        )
        .unwrap_err()
        .code(),
        "class-materialization-limit"
    );
}

#[test]
fn rejects_known_attribute_in_wrong_location() {
    assert_eq!(
        validate_known_attribute(
            "SourceFile",
            &[0, 1],
            20,
            &[None],
            AttributeLocation::Method,
        )
        .unwrap_err()
        .code(),
        "invalid-attribute-location"
    );
}
