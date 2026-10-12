use super::*;
use crate::{
    Attribute, AttributeLocation, Constant, attributes::parse_attributes,
    constant_pool::parse_prefix_with_budget, tests::minimal_class,
};

#[test]
fn empty_attributes_charge_container_overhead_before_allocation() {
    let pool = vec![None, Some(Constant::Utf8("x".into()))];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&8_u16.to_be_bytes());
    for _ in 0..8 {
        bytes.extend_from_slice(&1_u16.to_be_bytes());
        bytes.extend_from_slice(&0_u32.to_be_bytes());
    }
    let mut reader = Reader::new(&bytes);
    let mut materialization = MaterializationBudget {
        remaining: std::mem::size_of::<Attribute>() * 8 - 1,
    };

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
fn constant_pool_charges_text_and_container_storage() {
    let bytes = minimal_class();
    let mut budget = MaterializationBudget::for_input(bytes.len());
    let before = budget.remaining;
    let prefix = parse_prefix_with_budget(&bytes, &mut budget).unwrap();
    let containers = prefix.constant_pool.capacity() * std::mem::size_of::<Option<Constant>>()
        + prefix.constant_offsets.capacity() * std::mem::size_of::<Option<usize>>();
    assert!(budget.remaining < before - containers);

    let mut budget = MaterializationBudget {
        remaining: containers - 1,
    };
    assert_eq!(
        parse_prefix_with_budget(&bytes, &mut budget)
            .unwrap_err()
            .code(),
        "class-materialization-limit"
    );
}
