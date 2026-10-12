use super::*;

#[test]
fn graph_rejects_cycles_and_second_parent_but_readd_is_idempotent() {
    let mut runtime = Runtime::default();
    let a = group(&mut runtime);
    let b = group(&mut runtime);
    let c = group(&mut runtime);
    runtime.add_child(a, b).unwrap();
    runtime.add_child(b, c).unwrap();
    assert_eq!(runtime.add_child(c, a).unwrap_err().code(), "graph-cycle");
    runtime.add_child(a, b).unwrap();
    assert_eq!(runtime.children(a).unwrap(), &[b]);
    let other = group(&mut runtime);
    assert_eq!(
        runtime.add_child(other, c).unwrap_err().code(),
        "already-parented"
    );
}

#[test]
fn ancestry_queries_reject_a_corrupted_parent_cycle() {
    let mut runtime = Runtime::default();
    let first = group(&mut runtime);
    let second = group(&mut runtime);
    let unrelated = group(&mut runtime);
    runtime.add_child(first, second).unwrap();
    runtime
        .objects
        .get_mut(first)
        .unwrap()
        .kind
        .node_mut()
        .unwrap()
        .parent = Some(second);
    assert_eq!(runtime.graph_root(second).unwrap_err().code(), "cycle");
    assert_eq!(
        runtime.contains_node(unrelated, second).unwrap_err().code(),
        "cycle"
    );
    assert_eq!(
        runtime.world_transform(second).unwrap_err().code(),
        "graph-cycle"
    );
    assert_eq!(
        runtime.transform_to(second, unrelated).unwrap_err().code(),
        "graph-cycle"
    );
}

#[test]
fn object_search_preserves_depth_first_order_with_pending_siblings() {
    let mut runtime = Runtime::default();
    let root = group(&mut runtime);
    let first = group(&mut runtime);
    let second = group(&mut runtime);
    let leaf = group(&mut runtime);
    runtime.add_child(root, first).unwrap();
    runtime.add_child(root, second).unwrap();
    runtime.add_child(first, leaf).unwrap();
    runtime.set_user_id(second, 77).unwrap();
    runtime.set_user_id(leaf, 77).unwrap();
    assert_eq!(runtime.find(root, 77).unwrap(), Some(leaf));
    runtime.set_user_id(leaf, 0).unwrap();
    assert_eq!(runtime.find(root, 77).unwrap(), Some(second));
    assert_eq!(runtime.find(root, 88).unwrap(), None);
}

#[test]
fn deep_graph_find_and_transform_are_iterative() {
    let mut runtime = Runtime::new(ArenaLimits {
        objects: 6_000,
        bytes: 8 * 1024 * 1024,
    });
    let root = group(&mut runtime);
    let mut previous = root;
    for index in 1..5_000 {
        let next = group(&mut runtime);
        runtime.add_child(previous, next).unwrap();
        runtime
            .set_translation(next, Vec3::new(0.0, 0.0, 0.001))
            .unwrap();
        if index == 4_999 {
            runtime.set_user_id(next, 77).unwrap();
        }
        previous = next;
    }
    assert_eq!(runtime.find(root, 77).unwrap(), Some(previous));
    let transform = runtime.transform_to(previous, root).unwrap();
    assert!((transform.as_array()[14] - 4.999).abs() < 0.001);
}

#[test]
fn find_matches_the_default_zero_user_id() {
    let mut runtime = Runtime::default();
    let root = group(&mut runtime);

    assert_eq!(runtime.user_id(root).unwrap(), 0);
    assert_eq!(runtime.find(root, 0).unwrap(), Some(root));
}

#[test]
fn configured_graph_depth_is_enforced_when_linking() {
    let mut runtime = Runtime::new_with_graph_depth(ArenaLimits::default(), 2);
    let root = group(&mut runtime);
    let child = group(&mut runtime);
    let grandchild = group(&mut runtime);

    runtime.add_child(root, child).unwrap();
    assert_eq!(
        runtime.add_child(child, grandchild).unwrap_err().code(),
        "graph-depth"
    );
    assert_eq!(runtime.parent(grandchild).unwrap(), None);
}

#[test]
fn graph_depth_rejects_attaching_an_existing_subtree_atomically() {
    let mut runtime = Runtime::new_with_graph_depth(ArenaLimits::default(), 2);
    let root = group(&mut runtime);
    let child = group(&mut runtime);
    let grandchild = group(&mut runtime);
    runtime.add_child(child, grandchild).unwrap();
    let before = runtime.counters();

    assert_eq!(
        runtime.add_child(root, child).unwrap_err().code(),
        "graph-depth"
    );
    assert!(runtime.children(root).unwrap().is_empty());
    assert_eq!(runtime.parent(child).unwrap(), None);
    assert_eq!(runtime.parent(grandchild).unwrap(), Some(child));
    assert_eq!(runtime.counters(), before);
}

#[test]
fn implicit_skeleton_links_obey_the_same_depth_and_cycle_limits() {
    let mut runtime = Runtime::new_with_graph_depth(ArenaLimits::default(), 3);
    let skeleton = group(&mut runtime);
    let bone = group(&mut runtime);
    runtime.add_child(skeleton, bone).unwrap();
    let vertices = runtime.create(None, ObjectKind::Object).unwrap();
    let skin = runtime
        .create(
            None,
            ObjectKind::SkinnedMesh {
                mesh: MeshState {
                    node: NodeState::default(),
                    vertices,
                    submeshes: Vec::new(),
                    appearances: Vec::new(),
                },
                skeleton,
                bones: vec![bone],
                bind_transforms: Vec::new(),
                influences: Vec::new(),
            },
        )
        .unwrap();
    let root = group(&mut runtime);
    let before = runtime.counters();
    assert_eq!(
        runtime.add_child(root, skin).unwrap_err().code(),
        "graph-depth"
    );
    assert_eq!(runtime.counters(), before);

    runtime.max_graph_depth = 2;
    assert_eq!(
        runtime.bind_skin_skeleton(skin).unwrap_err().code(),
        "graph-depth"
    );
    assert_eq!(runtime.parent(skeleton).unwrap(), None);
    assert_eq!(runtime.counters(), before);

    runtime.max_graph_depth = 3;
    runtime.bind_skin_skeleton(skin).unwrap();
    assert_eq!(runtime.parent(skeleton).unwrap(), Some(skin));
    assert_eq!(
        runtime.add_child(bone, skin).unwrap_err().code(),
        "graph-cycle"
    );
    assert_eq!(runtime.parent(skin).unwrap(), None);
}

#[test]
fn failed_child_validation_is_atomic() {
    let mut runtime = Runtime::default();
    let parent = group(&mut runtime);
    let non_node = runtime.create(None, ObjectKind::Object).unwrap();
    assert_eq!(
        runtime.add_child(parent, non_node).unwrap_err().code(),
        "not-node"
    );
    assert!(runtime.children(parent).unwrap().is_empty());
}
