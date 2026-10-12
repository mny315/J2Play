use super::*;

#[test]
fn arena_budget_counts_image_and_vertex_storage() {
    let mut runtime = Runtime::new(ArenaLimits {
        objects: 8,
        bytes: 1024,
    });
    let image = Image2DState::mutable(crate::ImageFormat::Rgba, 32, 32).unwrap();
    assert_eq!(
        runtime
            .create(None, ObjectKind::Image2D(image))
            .unwrap_err()
            .code(),
        "resource-limit"
    );
    let vertices = VertexArrayState::new(512, 4, crate::VertexComponent::Short).unwrap();
    assert_eq!(
        runtime
            .create(None, ObjectKind::VertexArray(vertices))
            .unwrap_err()
            .code(),
        "resource-limit"
    );
    assert_eq!(runtime.counters(), (0, 0, 0, 0));
}

#[test]
fn arena_budget_rejects_animation_track_growth_transactionally() {
    let sequence = ObjectKind::KeyframeSequence(
        KeyframeSequenceState::new(1, 1, crate::Interpolation::Step).unwrap(),
    );
    let budget = estimated_bytes(&ObjectKind::Object) * 2 + estimated_bytes(&sequence);
    let mut runtime = Runtime::new(ArenaLimits {
        objects: 3,
        bytes: budget,
    });
    let sequence = runtime.create(None, sequence).unwrap();
    let object = runtime
        .create(None, ObjectKind::Node(NodeState::default()))
        .unwrap();
    let track = runtime
        .create(
            None,
            ObjectKind::AnimationTrack {
                sequence,
                controller: None,
                property: 256,
            },
        )
        .unwrap();
    let before = runtime.counters();

    assert_eq!(
        runtime
            .add_animation_track(object, track)
            .unwrap_err()
            .code(),
        "resource-limit"
    );
    assert!(runtime.animation_tracks(object).unwrap().is_empty());
    assert_eq!(runtime.counters(), before);
}

#[test]
fn arena_budget_rejects_child_growth_without_parenting() {
    let group = ObjectKind::Group {
        node: NodeState::default(),
        children: Vec::new(),
    };
    let child = ObjectKind::Node(NodeState::default());
    let budget = estimated_bytes(&group) + estimated_bytes(&child);
    let mut runtime = Runtime::new(ArenaLimits {
        objects: 2,
        bytes: budget,
    });
    let group = runtime.create(None, group).unwrap();
    let child = runtime.create(None, child).unwrap();
    let before = runtime.counters();

    assert_eq!(
        runtime.add_child(group, child).unwrap_err().code(),
        "resource-limit"
    );
    assert!(runtime.children(group).unwrap().is_empty());
    assert_eq!(runtime.parent(child).unwrap(), None);
    assert_eq!(runtime.counters(), before);
}

#[test]
fn child_growth_preserves_existing_spare_capacity() {
    let children = Vec::with_capacity(8);
    let group_kind = ObjectKind::Group {
        node: NodeState::default(),
        children,
    };
    let child_kind = ObjectKind::Node(NodeState::default());
    let budget = estimated_bytes(&group_kind) + estimated_bytes(&child_kind);
    let mut runtime = Runtime::new(ArenaLimits {
        objects: 2,
        bytes: budget,
    });
    let group = runtime.create(None, group_kind).unwrap();
    let child = runtime.create(None, child_kind).unwrap();
    let before = runtime.counters();

    runtime.add_child(group, child).unwrap();

    assert_eq!(runtime.children(group).unwrap(), [child]);
    assert_eq!(runtime.parent(child).unwrap(), Some(group));
    assert_eq!(runtime.counters(), before);
    let ObjectKind::Group { children, .. } = runtime.kind(group).unwrap() else {
        unreachable!();
    };
    assert_eq!(children.capacity(), 8);
}

#[test]
fn near_full_child_and_track_vectors_do_not_reallocate() {
    let node_kind = ObjectKind::Node(NodeState::default());
    let sequence_kind = ObjectKind::KeyframeSequence(
        KeyframeSequenceState::new(1, 1, crate::Interpolation::Step).unwrap(),
    );
    let mut runtime = Runtime::new(ArenaLimits {
        objects: 11,
        bytes: estimated_bytes(&node_kind) * 4
            + estimated_bytes(&ObjectKind::Object) * 5
            + estimated_bytes(&sequence_kind)
            + estimated_bytes(&ObjectKind::Group {
                node: NodeState::default(),
                children: Vec::with_capacity(4),
            })
            + 4 * std::mem::size_of::<Handle>(),
    });
    let children = (0..4)
        .map(|_| runtime.create(None, node_kind.clone()).unwrap())
        .collect::<Vec<_>>();
    let mut initial_children = Vec::with_capacity(4);
    initial_children.extend_from_slice(&children[..3]);
    let group = runtime
        .create(
            None,
            ObjectKind::Group {
                node: NodeState::default(),
                children: initial_children,
            },
        )
        .unwrap();
    for child in &children[..3] {
        runtime.kind_mut(*child).unwrap().node_mut().unwrap().parent = Some(group);
    }

    let sequence = runtime.create(None, sequence_kind).unwrap();
    let tracks = (0..4)
        .map(|_| {
            runtime
                .create(
                    None,
                    ObjectKind::AnimationTrack {
                        sequence,
                        controller: None,
                        property: 256,
                    },
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let object = Object {
        guest_reference: None,
        base: ObjectState {
            user_id: 0,
            user_object: None,
            animation_tracks: Vec::with_capacity(4),
        },
        kind: node_kind,
    };
    let object_bytes = estimated_object_bytes(&object);
    let object = runtime.objects.insert(object, object_bytes).unwrap();
    for track in &tracks[..3] {
        runtime.add_animation_track(object, *track).unwrap();
    }
    let before = runtime.counters();

    runtime.add_child(group, children[3]).unwrap();
    runtime.add_animation_track(object, tracks[3]).unwrap();

    assert_eq!(runtime.counters(), before);
    let ObjectKind::Group { children, .. } = runtime.kind(group).unwrap() else {
        unreachable!();
    };
    assert_eq!(children.capacity(), 4);
    assert_eq!(
        runtime
            .objects
            .get(object)
            .unwrap()
            .base
            .animation_tracks
            .capacity(),
        4
    );
}

#[test]
fn skin_transform_growth_is_transactional_under_arena_pressure() {
    let placeholder_kind = ObjectKind::Object;
    let skin_kind = |placeholder| ObjectKind::SkinnedMesh {
        mesh: MeshState {
            node: NodeState::default(),
            vertices: placeholder,
            submeshes: Vec::new(),
            appearances: Vec::new(),
        },
        skeleton: placeholder,
        bones: Vec::new(),
        bind_transforms: Vec::new(),
        influences: Vec::new(),
    };
    let mut prototype = Runtime::default();
    let prototype_placeholder = prototype.create(None, ObjectKind::Object).unwrap();
    let budget =
        estimated_bytes(&placeholder_kind) + estimated_bytes(&skin_kind(prototype_placeholder));
    let mut runtime = Runtime::new(ArenaLimits {
        objects: 2,
        bytes: budget,
    });
    let placeholder = runtime.create(None, placeholder_kind).unwrap();
    let skin = runtime.create(None, skin_kind(placeholder)).unwrap();
    let before = runtime.counters();

    assert_eq!(
        runtime
            .add_skin_transform(skin, placeholder, Mat4::IDENTITY, 0, 1, 1)
            .unwrap_err()
            .code(),
        "resource-limit"
    );
    let ObjectKind::SkinnedMesh {
        bones,
        bind_transforms,
        influences,
        ..
    } = runtime.kind(skin).unwrap()
    else {
        unreachable!();
    };
    assert!(bones.is_empty());
    assert!(bind_transforms.is_empty());
    assert!(influences.is_empty());
    assert_eq!(runtime.counters(), before);
}

#[test]
fn skin_binding_does_not_install_parent_when_bind_storage_exceeds_budget() {
    let mut prototype = Runtime::default();
    let placeholder = prototype.create(None, ObjectKind::Object).unwrap();
    let one_child = vec![placeholder];
    let one_submesh = vec![placeholder];
    let one_appearance = vec![None];
    let one_bone = vec![placeholder];
    let budget = estimated_bytes(&ObjectKind::Object) * 2
        + estimated_bytes(&ObjectKind::Node(NodeState::default()))
        + estimated_bytes(&ObjectKind::Group {
            node: NodeState::default(),
            children: one_child,
        })
        + estimated_bytes(&ObjectKind::SkinnedMesh {
            mesh: MeshState {
                node: NodeState::default(),
                vertices: placeholder,
                submeshes: one_submesh,
                appearances: one_appearance,
            },
            skeleton: placeholder,
            bones: one_bone,
            bind_transforms: Vec::new(),
            influences: Vec::new(),
        });

    let mut runtime = Runtime::new(ArenaLimits {
        objects: 5,
        bytes: budget,
    });
    let vertices = runtime.create(None, ObjectKind::Object).unwrap();
    let indices = runtime.create(None, ObjectKind::Object).unwrap();
    let bone = runtime
        .create(None, ObjectKind::Node(NodeState::default()))
        .unwrap();
    let children = vec![bone];
    let skeleton = runtime
        .create(
            None,
            ObjectKind::Group {
                node: NodeState::default(),
                children,
            },
        )
        .unwrap();
    runtime.kind_mut(bone).unwrap().node_mut().unwrap().parent = Some(skeleton);
    let submeshes = vec![indices];
    let appearances = vec![None];
    let bones = vec![bone];
    let skin = runtime
        .create(
            None,
            ObjectKind::SkinnedMesh {
                mesh: MeshState {
                    node: NodeState::default(),
                    vertices,
                    submeshes,
                    appearances,
                },
                skeleton,
                bones,
                bind_transforms: Vec::new(),
                influences: Vec::new(),
            },
        )
        .unwrap();
    let before = runtime.counters();

    assert_eq!(
        runtime.bind_skin_skeleton(skin).unwrap_err().code(),
        "resource-limit"
    );
    assert_eq!(runtime.parent(skeleton).unwrap(), None);
    let ObjectKind::SkinnedMesh {
        bind_transforms, ..
    } = runtime.kind(skin).unwrap()
    else {
        unreachable!();
    };
    assert!(bind_transforms.is_empty());
    assert_eq!(runtime.counters(), before);
}
