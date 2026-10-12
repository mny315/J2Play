use super::*;
use crate::machine::tests::method;
use crate::machine::{FieldToken, parse_method_descriptor};

#[test]
fn sparse_constant_cache_does_not_expand_across_unused_high_indexes() {
    let mut cache = SparseConstantCache::default();
    cache.insert(u16::MAX, 7_u8);
    assert_eq!(cache.pages.len(), 1);
    assert_eq!(cache.get(u16::MAX), Some(&7));
    assert_eq!(cache.get(0), None);

    cache.insert(0, 3);
    assert_eq!(cache.pages.len(), 2);
    assert_eq!(cache.get(0), Some(&3));
    assert_eq!(cache.get(u16::MAX), Some(&7));
}

#[test]
fn field_inline_cache_treats_collisions_as_misses() {
    let field = |key: &str| {
        std::rc::Rc::new(Field {
            key: key.into(),
            declaring_class: "T".into(),
            kind: ValueKind::Int,
            is_static: false,
            field_token: FieldToken::new(),
            instance_slot: Some(0),
            initial: Value::Int(0),
            constant_string: None,
        })
    };
    let first = field("T.first:I");
    let colliding = field("T.colliding:I");
    let mut cache = FieldInlineCache::default();

    let first_slots = FieldRuntimeSlots {
        static_field: None,
        declaring_class: Some(3),
    };
    cache.insert_resolved(0, 1, std::rc::Rc::clone(&first), first_slots);
    let (resolved, slots) = cache.get_resolved(0, 1).unwrap();
    assert!(std::rc::Rc::ptr_eq(resolved, &first));
    assert_eq!(slots, first_slots);
    let colliding_index = u16::try_from(FIELD_INLINE_CACHE_ENTRIES + 1).unwrap();
    let colliding_slots = FieldRuntimeSlots {
        static_field: None,
        declaring_class: Some(5),
    };
    cache.insert_resolved(
        0,
        colliding_index,
        std::rc::Rc::clone(&colliding),
        colliding_slots,
    );

    assert!(cache.get_resolved(0, 1).is_none());
    let (resolved, slots) = cache.get_resolved(0, colliding_index).unwrap();
    assert!(std::rc::Rc::ptr_eq(resolved, &colliding));
    assert_eq!(slots, colliding_slots);
}

#[test]
fn method_ref_inline_cache_treats_collisions_as_misses() {
    let reference = |name: &str| {
        std::rc::Rc::new(ResolvedMethodRef {
            symbolic: MethodKey {
                class: "T".into(),
                name: name.into(),
                descriptor: "()I".into(),
            },
            descriptor: parse_method_descriptor("()I").unwrap(),
            is_static: false,
        })
    };
    let first = reference("first");
    let colliding = reference("colliding");
    let mut cache = MethodRefInlineCache::default();

    cache.insert(0, 1, std::rc::Rc::clone(&first));
    assert!(std::rc::Rc::ptr_eq(cache.get(0, 1).unwrap(), &first));
    let colliding_index = u16::try_from(METHOD_REF_INLINE_CACHE_ENTRIES + 1).unwrap();
    cache.insert(0, colliding_index, std::rc::Rc::clone(&colliding));

    assert!(cache.get(0, 1).is_none());
    assert!(std::rc::Rc::ptr_eq(
        cache.get(0, colliding_index).unwrap(),
        &colliding
    ));
}

#[test]
fn fixed_method_inline_cache_treats_collisions_as_misses() {
    let first = std::rc::Rc::new(method(&[0x03, 0xac], 1, 0));
    let colliding = std::rc::Rc::new(method(&[0x04, 0xac], 1, 0));
    let mut cache = FixedMethodInlineCache::default();

    cache.insert(0, 1, 0xb8, std::rc::Rc::clone(&first), Some(3));
    let (resolved, class) = cache.get(0, 1, 0xb8).unwrap();
    assert!(std::rc::Rc::ptr_eq(resolved, &first));
    assert_eq!(class, Some(3));
    let colliding_index = u16::try_from(FIXED_METHOD_INLINE_CACHE_ENTRIES + 1).unwrap();
    cache.insert(
        0,
        colliding_index,
        0xb8,
        std::rc::Rc::clone(&colliding),
        Some(5),
    );

    assert!(cache.get(0, 1, 0xb8).is_none());
    let (resolved, class) = cache.get(0, colliding_index, 0xb8).unwrap();
    assert!(std::rc::Rc::ptr_eq(resolved, &colliding));
    assert_eq!(class, Some(5));
    assert!(cache.get(0, colliding_index, 0xb7).is_none());

    cache.insert(0, 1, 0xb8, std::rc::Rc::clone(&first), Some(0));
    assert_eq!(cache.get(0, 1, 0xb8).unwrap().1, Some(0));
    cache.insert(0, 1, 0xb8, std::rc::Rc::clone(&first), None);
    let (resolved, class) = cache.get(0, 1, 0xb8).unwrap();
    assert!(std::rc::Rc::ptr_eq(resolved, &first));
    assert_eq!(class, None);
}
