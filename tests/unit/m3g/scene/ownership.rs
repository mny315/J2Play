use super::*;

#[test]
fn rollback_detaches_surviving_external_children_and_skeletons() {
    let mut runtime = Runtime::default();
    let external_child = runtime
        .create(Some(11), ObjectKind::Node(NodeState::default()))
        .unwrap();
    let external_skeleton = runtime
        .create(
            Some(12),
            ObjectKind::Group {
                node: NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    let vertices = runtime.create(Some(13), ObjectKind::Object).unwrap();
    let indices = runtime.create(Some(14), ObjectKind::Object).unwrap();
    let baseline = runtime.counters();

    let created_group = group(&mut runtime);
    runtime.add_child(created_group, external_child).unwrap();
    let created_skin = runtime
        .create(
            None,
            ObjectKind::SkinnedMesh {
                mesh: MeshState {
                    node: NodeState::default(),
                    vertices,
                    submeshes: vec![indices],
                    appearances: vec![None],
                },
                skeleton: external_skeleton,
                bones: Vec::new(),
                bind_transforms: Vec::new(),
                influences: Vec::new(),
            },
        )
        .unwrap();
    runtime.bind_skin_skeleton(created_skin).unwrap();
    assert_eq!(runtime.parent(external_child).unwrap(), Some(created_group));
    assert_eq!(
        runtime.parent(external_skeleton).unwrap(),
        Some(created_skin)
    );

    runtime.rollback_created(&[created_group, created_skin]);

    assert_eq!(runtime.parent(external_child).unwrap(), None);
    assert_eq!(runtime.parent(external_skeleton).unwrap(), None);
    assert_eq!(runtime.resolve_guest(11).unwrap(), external_child);
    assert_eq!(runtime.resolve_guest(12).unwrap(), external_skeleton);
    assert_eq!(runtime.counters().0, baseline.0);
    assert_eq!(runtime.counters().2, baseline.2);
    assert_eq!(
        runtime.kind(created_group).unwrap_err().code(),
        "stale-handle"
    );
    assert_eq!(
        runtime.kind(created_skin).unwrap_err().code(),
        "stale-handle"
    );
}

#[test]
fn guest_closure_retains_parent_hidden_from_public_references() {
    let mut runtime = Runtime::default();
    let parent = runtime
        .create(
            Some(11),
            ObjectKind::Group {
                node: NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    let child = runtime
        .create(
            Some(22),
            ObjectKind::Group {
                node: NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    runtime.add_child(parent, child).unwrap();

    assert!(runtime.references(child).unwrap().is_empty());
    assert_eq!(runtime.guest_closure([22]), vec![11, 22]);
}

#[test]
fn guest_reverse_binding_follows_duplicate_rollback_and_sweep() {
    let mut runtime = Runtime::default();
    let source = runtime.create(Some(11), ObjectKind::Object).unwrap();
    let unbound = runtime.create(None, ObjectKind::Object).unwrap();
    let duplicate = runtime.create_duplicate(source, 22).unwrap();

    assert_eq!(runtime.guest_reference(source), Some(11));
    assert_eq!(runtime.guest_reference(unbound), None);
    assert_eq!(runtime.guest_reference(duplicate), Some(22));

    runtime.rollback_created(&[duplicate]);
    assert_eq!(runtime.guest_reference(duplicate), None);
    assert_eq!(
        runtime.resolve_guest(22).unwrap_err().code(),
        "unbound-guest-object"
    );

    runtime.sweep_guest_objects(|reference| reference != 11);
    assert_eq!(runtime.guest_reference(source), None);
    assert_eq!(
        runtime.resolve_guest(11).unwrap_err().code(),
        "unbound-guest-object"
    );
}

#[test]
fn duplicate_copies_descendants_user_state_and_remaps_edges() {
    let mut runtime = Runtime::default();
    let root = runtime
        .create(
            Some(1),
            ObjectKind::Group {
                node: NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    let child = runtime
        .create(
            Some(2),
            ObjectKind::Group {
                node: NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    runtime.set_user_id(root, 41).unwrap();
    runtime.set_user_id(child, 42).unwrap();
    runtime.add_child(root, child).unwrap();
    assert_eq!(runtime.duplicate_sources(root).unwrap(), [root, child]);
    let duplicate_root = runtime.create_duplicate(root, 3).unwrap();
    let duplicate_child = runtime.create_duplicate(child, 4).unwrap();
    let mapping = BTreeMap::from([(root, duplicate_root), (child, duplicate_child)]);
    runtime.finish_duplicate(root, &mapping).unwrap();
    assert_eq!(runtime.parent(duplicate_root).unwrap(), None);
    assert_eq!(
        runtime.parent(duplicate_child).unwrap(),
        Some(duplicate_root)
    );
    assert_eq!(
        runtime.children(duplicate_root).unwrap(),
        &[duplicate_child]
    );
    assert_eq!(runtime.user_id(duplicate_root).unwrap(), 41);
    assert_eq!(runtime.user_id(duplicate_child).unwrap(), 42);
}

#[test]
fn teardown_releases_guest_bindings_and_native_accounting() {
    let mut runtime = Runtime::default();
    runtime.create(Some(41), ObjectKind::Object).unwrap();
    assert!(runtime.resolve_guest(41).is_ok());
    runtime.teardown();
    assert_eq!(
        runtime.resolve_guest(41).unwrap_err().code(),
        "unbound-guest-object"
    );
    assert_eq!(runtime.counters().0, 0);
    assert_eq!(runtime.counters().2, 0);
}
