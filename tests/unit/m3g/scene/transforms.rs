use super::*;
use crate::Vec4;

#[test]
fn single_axis_alignment_overwrites_existing_billboard_orientation() {
    let mut runtime = Runtime::default();
    let root = group(&mut runtime);
    let camera = runtime
        .create(
            None,
            ObjectKind::Camera {
                node: NodeState::default(),
                projection: CameraProjection::default(),
            },
        )
        .unwrap();
    let billboard = runtime
        .create(None, ObjectKind::Node(NodeState::default()))
        .unwrap();
    runtime.add_child(root, camera).unwrap();
    runtime.add_child(root, billboard).unwrap();
    runtime
        .set_orientation(billboard, -90.0, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    runtime
        .set_alignments(billboard, Some(camera), 148, None, 144)
        .unwrap();

    runtime.align_subtree(billboard, Some(camera)).unwrap();

    let orientation = runtime.transformable(billboard).unwrap().orientation;
    let (angle, _) = orientation.to_axis_angle().unwrap();
    assert!(angle.abs() < 1.0e-5, "unexpected alignment angle: {angle}");
}

fn alignment_fixture() -> (Runtime, Handle, Handle, Handle, Handle) {
    let mut runtime = Runtime::default();
    let root = group(&mut runtime);
    let parent = group(&mut runtime);
    let reference = group(&mut runtime);
    let aligned = group(&mut runtime);
    runtime.add_child(root, parent).unwrap();
    runtime.add_child(root, reference).unwrap();
    runtime.add_child(parent, aligned).unwrap();
    runtime
        .set_translation(parent, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    runtime
        .set_translation(reference, Vec3::new(4.0, 6.0, 8.0))
        .unwrap();
    runtime
        .set_translation(aligned, Vec3::new(1.0, 2.0, 3.0))
        .unwrap();
    runtime
        .set_orientation(reference, 90.0, Vec3::new(0.0, 1.0, 0.0))
        .unwrap();
    (runtime, root, parent, reference, aligned)
}

fn check_alignment_ignores_shared_transform(transform: Mat4) {
    for (z_target, y_target, axis, expected) in [
        (
            145,
            144,
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec3::new(1.0, 4.0, 5.0),
        ),
        (
            144,
            145,
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            Vec3::new(1.0, 4.0, 5.0),
        ),
        (
            148,
            147,
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        ),
    ] {
        let (mut runtime, root, _, reference, aligned) = alignment_fixture();
        runtime.set_transform(root, transform).unwrap();
        runtime
            .set_alignments(
                aligned,
                Some(reference),
                z_target,
                Some(reference),
                y_target,
            )
            .unwrap();
        runtime.align_subtree(aligned, None).unwrap();
        let actual = runtime
            .composite_transform(aligned)
            .unwrap()
            .transform(axis);
        let expected = expected.normalized().unwrap();
        assert!(
            (actual.x - expected.x).abs() < 1.0e-5,
            "{actual:?} vs {expected:?}"
        );
        assert!(
            (actual.y - expected.y).abs() < 1.0e-5,
            "{actual:?} vs {expected:?}"
        );
        assert!(
            (actual.z - expected.z).abs() < 1.0e-5,
            "{actual:?} vs {expected:?}"
        );
    }
}

#[test]
fn alignment_ignores_shared_singular_ancestors() {
    check_alignment_ignores_shared_transform(Mat4::IDENTITY);
    check_alignment_ignores_shared_transform(Mat4::scale(0.0, 0.0, 0.0).unwrap());
}

#[test]
fn alignment_preserves_local_distances_below_large_shared_translations() {
    check_alignment_ignores_shared_transform(Mat4::translation(1.0e20, 1.0e20, 1.0e20).unwrap());
}

#[test]
fn alignment_rejects_a_singular_relative_path_without_mutation() {
    let (mut runtime, _, parent, reference, aligned) = alignment_fixture();
    runtime.set_scale(parent, Vec3::new(0.0, 0.0, 0.0)).unwrap();
    runtime
        .set_orientation(aligned, 45.0, Vec3::new(0.0, 1.0, 0.0))
        .unwrap();
    runtime
        .set_alignments(aligned, Some(reference), 145, None, 144)
        .unwrap();
    let before = runtime.transform_components(aligned).unwrap();
    assert_eq!(
        runtime.align_subtree(aligned, None).unwrap_err().code(),
        "singular-transform"
    );
    assert_eq!(runtime.transform_components(aligned).unwrap(), before);
}

#[test]
#[ignore = "manual node alignment throughput measurement"]
fn node_alignment_throughput() {
    for depth in [1, 16] {
        let mut runtime = Runtime::default();
        let root = group(&mut runtime);
        let mut parent = root;
        for _ in 1..depth {
            let child = group(&mut runtime);
            runtime.add_child(parent, child).unwrap();
            parent = child;
        }
        let reference = group(&mut runtime);
        runtime.add_child(parent, reference).unwrap();
        runtime
            .set_translation(reference, Vec3::new(4.0, 6.0, 8.0))
            .unwrap();
        let mut aligned = Vec::new();
        for x in 0..128 {
            let node = group(&mut runtime);
            runtime.add_child(parent, node).unwrap();
            runtime
                .set_translation(node, Vec3::new(x as f32, 2.0, 3.0))
                .unwrap();
            runtime
                .set_alignments(node, Some(reference), 145, None, 144)
                .unwrap();
            aligned.push(node);
        }
        let started = std::time::Instant::now();
        for _ in 0..1024 {
            runtime
                .align_subtree(std::hint::black_box(root), None)
                .unwrap();
        }
        let elapsed = started.elapsed();
        let checksum = aligned.into_iter().fold(0_u64, |checksum, node| {
            runtime
                .composite_transform(node)
                .unwrap()
                .to_row_major()
                .into_iter()
                .fold(checksum, |checksum, value| {
                    checksum
                        .wrapping_mul(31)
                        .wrapping_add(u64::from(value.to_bits()))
                })
        });
        eprintln!("alignment depth={depth} elapsed={elapsed:?} checksum={checksum:016x}");
    }
}

#[test]
fn alignment_rejects_corrupted_child_cycles_before_changing_orientation() {
    let mut runtime = Runtime::default();
    let root = group(&mut runtime);
    let aligned = group(&mut runtime);
    let cycle = group(&mut runtime);
    runtime.add_child(root, aligned).unwrap();
    runtime.add_child(root, cycle).unwrap();
    runtime
        .set_orientation(aligned, 90.0, Vec3::new(0.0, 1.0, 0.0))
        .unwrap();
    runtime
        .set_alignments(aligned, Some(root), 148, None, 144)
        .unwrap();
    let before = runtime.composite_transform(aligned).unwrap();
    runtime
        .objects
        .get_mut(cycle)
        .unwrap()
        .kind
        .children_mut()
        .unwrap()
        .push(root);

    assert_eq!(
        runtime.align_subtree(root, None).unwrap_err().code(),
        "graph-cycle"
    );
    assert_eq!(runtime.composite_transform(aligned).unwrap(), before);
}

#[test]
fn skinned_mesh_preserves_bind_pose_and_tracks_bone_delta() {
    let mut runtime = Runtime::default();
    let skeleton = group(&mut runtime);
    let bone = runtime
        .create(None, ObjectKind::Node(NodeState::default()))
        .unwrap();
    runtime.add_child(skeleton, bone).unwrap();
    runtime
        .set_translation(bone, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    let vertices = runtime.create(None, ObjectKind::Object).unwrap();
    let indices = runtime.create(None, ObjectKind::Object).unwrap();
    let skin = runtime
        .create(
            None,
            ObjectKind::SkinnedMesh {
                mesh: MeshState {
                    node: NodeState::default(),
                    vertices,
                    submeshes: vec![indices],
                    appearances: vec![None],
                },
                skeleton,
                bones: vec![bone],
                bind_transforms: Vec::new(),
                influences: Vec::new(),
            },
        )
        .unwrap();

    runtime.bind_skin_skeleton(skin).unwrap();
    assert_eq!(runtime.parent(skeleton).unwrap(), Some(skin));
    let bind = match runtime.kind(skin).unwrap() {
        ObjectKind::SkinnedMesh {
            bind_transforms, ..
        } => bind_transforms[0],
        _ => unreachable!(),
    };
    let at_rest = runtime.transform_to(bone, skin).unwrap().multiplied(bind);
    assert_eq!(
        at_rest.transform(Vec4::new(0.0, 0.0, 0.0, 1.0)),
        Vec4::new(0.0, 0.0, 0.0, 1.0)
    );

    runtime
        .set_translation(bone, Vec3::new(3.0, 0.0, 0.0))
        .unwrap();
    let animated = runtime
        .transform_to(bone, skin)
        .unwrap()
        .multiplied(bind)
        .transform(Vec4::new(0.0, 0.0, 0.0, 1.0));
    assert_eq!(animated, Vec4::new(1.0, 0.0, 0.0, 1.0));
}

#[test]
fn node_transforms_reject_projective_rows_without_mutation() {
    let mut runtime = Runtime::default();
    let node = group(&mut runtime);
    let transformable = runtime
        .create(
            None,
            ObjectKind::Transformable(TransformableState::default()),
        )
        .unwrap();
    let before = Mat4::translation(3.0, 2.0, 1.0).unwrap();
    runtime.set_transform(node, before).unwrap();
    for (index, value) in [(12, 0.5), (13, 0.5), (14, 0.5), (15, 0.0), (15, 2.0)] {
        let mut values = before.to_row_major();
        values[index] = value;
        let projective = Mat4::from_row_major(values).unwrap();
        assert_eq!(
            runtime.set_transform(node, projective).unwrap_err().code(),
            "invalid-transform"
        );
        assert_eq!(runtime.general_transform(node).unwrap(), before);
        // Texture-style Transformables and standalone Transform values retain
        // the complete 4x4 contract.
        runtime.set_transform(transformable, projective).unwrap();
        assert_eq!(
            runtime.general_transform(transformable).unwrap(),
            projective
        );
    }
}

#[test]
fn relative_transforms_ignore_shared_singular_ancestors() {
    let mut runtime = Runtime::default();
    let root = group(&mut runtime);
    let source = group(&mut runtime);
    let target = group(&mut runtime);
    runtime.add_child(root, source).unwrap();
    runtime.add_child(root, target).unwrap();
    runtime.set_scale(root, Vec3::new(0.0, 0.0, 0.0)).unwrap();
    runtime
        .set_translation(source, Vec3::new(3.0, 2.0, 0.0))
        .unwrap();
    runtime
        .set_translation(target, Vec3::new(7.0, 0.0, 0.0))
        .unwrap();
    assert_eq!(
        runtime.transform_to(source, target).unwrap(),
        Mat4::translation(-4.0, 2.0, 0.0).unwrap()
    );
    assert_eq!(
        runtime.transform_to(source, root).unwrap(),
        Mat4::translation(3.0, 2.0, 0.0).unwrap()
    );
    assert_eq!(runtime.transform_to(root, root).unwrap(), Mat4::IDENTITY);
    runtime.set_scale(target, Vec3::new(0.0, 0.0, 0.0)).unwrap();
    assert_eq!(
        runtime.transform_to(target, target).unwrap(),
        Mat4::IDENTITY
    );
    assert_eq!(
        runtime.transform_to(source, target).unwrap_err().code(),
        "singular-transform"
    );
    let unrelated = group(&mut runtime);
    assert_eq!(
        runtime.transform_to(source, unrelated).unwrap_err().code(),
        "no-path"
    );
}

#[test]
fn relative_transforms_reject_overflow_without_poisoning_state() {
    let mut runtime = Runtime::default();
    let node = group(&mut runtime);
    let maximum = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
    runtime.set_translation(node, maximum).unwrap();
    runtime.set_scale(node, maximum).unwrap();
    let before = runtime.transform_components(node).unwrap();

    assert!(runtime.translate(node, maximum).is_err());
    assert_eq!(runtime.transform_components(node).unwrap(), before);
    assert!(runtime.scale_by(node, Vec3::new(2.0, 2.0, 2.0)).is_err());
    assert_eq!(runtime.transform_components(node).unwrap(), before);
}
