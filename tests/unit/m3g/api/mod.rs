use super::*;

#[test]
fn maintenance_inventory_is_complete() {
    assert_eq!(api_counts(), (30, 308, 79));
    let classes = bootstrap_classes();
    assert_eq!(classes.len(), 30);
    assert!(classes.iter().all(|class| {
        class
            .methods
            .iter()
            .all(|method| method.access_flags & ACC_NATIVE != 0)
    }));
}

#[test]
fn every_native_signature_registers_exactly_once() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    assert_eq!(registry.len(), 308);
}
