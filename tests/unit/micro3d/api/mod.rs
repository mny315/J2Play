use super::*;

#[test]
fn public_inventory_keeps_its_class_method_and_field_counts() {
    let (classes, methods, fields) = api_counts();
    assert_eq!(classes, 10);
    assert_eq!(methods, 136);
    assert_eq!(fields, 64);
}

#[test]
fn every_signature_registers_once() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    assert_eq!(registry.len(), api_counts().1);
}

#[test]
fn bootstrap_classes_are_java_4_classfiles() {
    for class in bootstrap_classes() {
        assert_eq!(class.major_version, 48);
    }
}
