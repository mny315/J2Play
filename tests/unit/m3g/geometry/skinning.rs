use super::*;
use crate::{VertexArrayState, VertexComponent};

#[test]
fn selected_skin_weights_match_overlapping_ranges_and_stable_ties() {
    let mut influences: Vec<_> = (0..160)
        .map(|index| SkinInfluence {
            bone: index % 7,
            first_vertex: (index * 13) % 61,
            vertex_count: index % 4 + 1,
            weight: if index % 11 == 0 {
                u32::MAX
            } else {
                (index % 3 + 1) as u32
            },
        })
        .collect();
    for reverse in [false, true] {
        if reverse {
            influences.reverse();
        }
        for maximum in [1, 2, 3, 7] {
            let mut weights = SkinningWeights::new(&influences, maximum).unwrap();
            for vertex in 0..65 {
                let mut totals = BTreeMap::<usize, u64>::new();
                for influence in &influences {
                    if (influence.first_vertex..influence.first_vertex + influence.vertex_count)
                        .contains(&vertex)
                    {
                        *totals.entry(influence.bone).or_default() += u64::from(influence.weight);
                    }
                }
                let mut ranked: Vec<_> = totals.into_iter().collect();
                ranked.sort_unstable_by_key(|&(bone, weight)| (Reverse(weight), bone));
                ranked.truncate(maximum);
                let total: u64 = ranked.iter().map(|(_, weight)| weight).sum();
                let mut expected: Vec<_> = ranked
                    .into_iter()
                    .map(|(bone, weight)| (bone, weight as f32 / total as f32))
                    .collect();
                expected.sort_unstable_by_key(|&(bone, _)| bone);
                assert_eq!(
                    weights.at(vertex),
                    expected,
                    "{maximum}, {vertex}, {reverse}"
                );
            }
            assert_eq!(weights.next_change, influences.len() * 2);
        }
    }
}

#[test]
fn deformation_and_weight_queries_use_the_same_strongest_bones() {
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(
            Some(VertexArrayState::new(1, 3, VertexComponent::Short).unwrap()),
            1.0,
            [0.0; 3],
        )
        .unwrap();
    let transforms = [
        Mat4::translation(10.0, 0.0, 0.0).unwrap(),
        Mat4::translation(20.0, 0.0, 0.0).unwrap(),
        Mat4::translation(40.0, 0.0, 0.0).unwrap(),
    ];
    // More ranges than four times the vertex count are legal; repeated ranges
    // of the same bone contribute to its combined weight before selection.
    let influences: Vec<_> = [0, 0, 0, 1, 2, 2]
        .into_iter()
        .map(|bone| SkinInfluence {
            bone,
            first_vertex: 0,
            vertex_count: 1,
            weight: 1,
        })
        .collect();
    for (maximum, expected) in [(1, 10.0), (2, 22.0), (3, 130.0 / 6.0)] {
        let output = buffer
            .transformed_skinned_vertices(&transforms, &influences, maximum, Mat4::IDENTITY)
            .unwrap();
        assert!((output[0].position.x - expected).abs() < 1.0e-5);
        let mut queried = 0.0;
        for (bone, transform) in transforms.iter().enumerate() {
            let pairs: Vec<_> = SkinInfluence::bone_vertex_weights(&influences, bone, maximum)
                .unwrap()
                .collect();
            if let Some(&(vertex, weight)) = pairs.first() {
                assert_eq!(vertex, 0);
                assert_eq!(pairs.len(), 1);
                queried += weight * transform.as_array()[12];
            }
        }
        assert!((queried - expected).abs() < 1.0e-5);
    }
}

#[test]
fn discarded_skin_bones_do_not_require_normal_matrix_inversion() {
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(
            Some(VertexArrayState::new(1, 3, VertexComponent::Short).unwrap()),
            1.0,
            [0.0; 3],
        )
        .unwrap();
    let mut normals = VertexArrayState::new(1, 3, VertexComponent::Short).unwrap();
    normals.set_shorts(0, 1, &[0, 0, i16::MAX]).unwrap();
    buffer.set_normals(Some(normals)).unwrap();
    let bones = [Mat4::scale(0.0, 0.0, 0.0).unwrap(), Mat4::IDENTITY];
    let influences = [1, 2]
        .into_iter()
        .enumerate()
        .map(|(bone, weight)| SkinInfluence {
            bone,
            first_vertex: 0,
            vertex_count: 1,
            weight,
        })
        .collect::<Vec<_>>();
    let vertices = buffer
        .transformed_lit_skinned_vertices(
            &bones,
            &influences,
            1,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Vec3::new(0.0, 0.0, 1.0),
            MaterialState {
                diffuse: u32::MAX,
                ..Default::default()
            },
            false,
            &[LightSource::Directional {
                direction: Vec3::new(0.0, 0.0, 1.0),
                color: 0x00ff_ffff,
                intensity: 1.0,
            }],
            1.0,
        )
        .unwrap();
    assert_eq!(vertices[0].color, 0xffff_ffff);
}

#[test]
fn bone_vertex_weights_merge_overlaps_and_skip_gaps() {
    let influences = [(0, 1, 3, 1), (0, 2, 2, 3), (1, 0, 3, 4), (0, 6, 1, 7)].map(
        |(bone, first_vertex, vertex_count, weight)| SkinInfluence {
            bone,
            first_vertex,
            vertex_count,
            weight,
        },
    );
    assert_eq!(
        SkinInfluence::bone_vertex_weights(&influences, 0, 3)
            .unwrap()
            .collect::<Vec<_>>(),
        [(1, 0.2), (2, 0.5), (3, 1.0), (6, 1.0)]
    );
    assert_eq!(
        SkinInfluence::bone_vertex_weights(&[], 0, 3)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        SkinInfluence::bone_vertex_weights(&influences, 2, 3)
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn bone_vertex_weights_bound_ranges_and_accumulate_large_weights() {
    let influences = (0..4096)
        .map(|index| SkinInfluence {
            bone: index % 2,
            first_vertex: 0,
            vertex_count: usize::from(u16::MAX),
            weight: i32::MAX as u32,
        })
        .collect::<Vec<_>>();
    let pairs = SkinInfluence::bone_vertex_weights(&influences, 0, 3)
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(pairs.len(), usize::from(u16::MAX));
    assert_eq!(pairs.first(), Some(&(0, 0.5)));
    assert_eq!(pairs.last(), Some(&(65_534, 0.5)));
    for first_vertex in [65_535, usize::MAX] {
        assert!(
            SkinInfluence::bone_vertex_weights(
                &[SkinInfluence {
                    first_vertex,
                    vertex_count: 1,
                    ..influences[0]
                }],
                0,
                3,
            )
            .is_err()
        );
    }
}

#[test]
fn weighted_skinning_deforms_vertices_deterministically() {
    let mut positions = VertexArrayState::new(1, 3, VertexComponent::Short).unwrap();
    positions.set_shorts(0, 1, &[0, 0, 0]).unwrap();
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(Some(positions), 1.0, [0.0; 3])
        .unwrap();
    let bones = [
        Mat4::translation(2.0, 0.0, 0.0).unwrap(),
        Mat4::translation(0.0, 2.0, 0.0).unwrap(),
    ];
    let influences = [
        SkinInfluence {
            bone: 0,
            first_vertex: 0,
            vertex_count: 1,
            weight: 1,
        },
        SkinInfluence {
            bone: 1,
            first_vertex: 0,
            vertex_count: 1,
            weight: 3,
        },
    ];
    let vertices = buffer
        .transformed_skinned_vertices(&bones, &influences, 3, Mat4::IDENTITY)
        .unwrap();
    assert_eq!(vertices[0].position, Vec4::new(0.5, 1.5, 0.0, 1.0));
}

#[test]
fn lit_skinning_uses_material_instead_of_unlit_default_color() {
    let mut positions = VertexArrayState::new(1, 3, VertexComponent::Short).unwrap();
    positions.set_shorts(0, 1, &[0, 0, 0]).unwrap();
    let mut normals = VertexArrayState::new(1, 3, VertexComponent::Short).unwrap();
    normals.set_shorts(0, 1, &[0, 0, i16::MAX]).unwrap();
    let mut buffer = VertexBufferState::default();
    buffer
        .set_positions(Some(positions), 1.0, [0.0; 3])
        .unwrap();
    buffer.set_normals(Some(normals)).unwrap();
    let bones = [Mat4::translation(2.0, 0.0, 0.0).unwrap()];
    let influences = [SkinInfluence {
        bone: 0,
        first_vertex: 0,
        vertex_count: 1,
        weight: 1,
    }];
    let material = MaterialState {
        emissive: 0x007f_7f7f,
        ..MaterialState::default()
    };

    let vertices = buffer
        .transformed_lit_skinned_vertices(
            &bones,
            &influences,
            3,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Vec3::new(0.0, 0.0, 1.0),
            material,
            false,
            &[],
            1.0,
        )
        .unwrap();

    assert_eq!(vertices[0].position, Vec4::new(2.0, 0.0, 0.0, 1.0));
    assert_eq!(vertices[0].color, 0xff7f_7f7f);

    let flattened = buffer
        .transformed_lit_skinned_vertices(
            &bones,
            &influences,
            3,
            Mat4::scale(1.0, 1.0, 0.0).unwrap(),
            Mat4::IDENTITY,
            Vec3::new(0.0, 0.0, 1.0),
            material,
            false,
            &[],
            1.0,
        )
        .unwrap();
    assert_eq!(flattened[0].color, 0xff7f_7f7f);
}

#[test]
fn skin_ranges_reject_missing_bones() {
    assert_eq!(
        SkinInfluence::validate_all(
            &[SkinInfluence {
                bone: 1,
                first_vertex: 0,
                vertex_count: 1,
                weight: 1,
            }],
            1,
            1,
        )
        .unwrap_err()
        .code(),
        "invalid-object-graph"
    );
}
