use super::*;
use crate::{FileHeader, ParsedObject};

fn file_with_object(object_type: ObjectType, data: Vec<u8>) -> M3gFile {
    M3gFile {
        header: FileHeader {
            major_version: 1,
            minor_version: 0,
            has_external_references: false,
            total_file_size: 0,
            approximate_content_size: 0,
            authoring_field: String::new(),
        },
        sections: Vec::new(),
        objects: vec![
            ParsedObject {
                index: 1,
                object_type: ObjectType::Header,
                data: std::sync::Arc::default(),
                section_offset: 12,
            },
            ParsedObject {
                index: 2,
                object_type,
                data: data.into(),
                section_offset: 64,
            },
        ],
    }
}

#[test]
fn invalid_animation_load_rolls_back_created_objects_and_preserves_external_sequences() {
    for incompatible_target in [false, true] {
        let mut runtime = Runtime::default();
        let sequence = runtime
            .create(
                Some(11),
                ObjectKind::KeyframeSequence(
                    crate::KeyframeSequenceState::new(
                        1,
                        if incompatible_target { 4 } else { 1 },
                        crate::Interpolation::Step,
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
        let before = runtime.counters();
        let mut file = file_with_object(ObjectType::ExternalReference, b"sequence.m3g\0".to_vec());
        let mut controller = vec![0; 12];
        controller.extend_from_slice(&1.0_f32.to_le_bytes());
        controller.extend_from_slice(&1.0_f32.to_le_bytes());
        controller.extend_from_slice(&[0; 16]);
        file.objects.push(ParsedObject {
            index: 3,
            object_type: ObjectType::AnimationController,
            data: controller.into(),
            section_offset: 64,
        });
        let mut track = vec![0; 12];
        for field in [2_u32, 3, if incompatible_target { 268 } else { 275 }] {
            track.extend_from_slice(&field.to_le_bytes());
        }
        file.objects.push(ParsedObject {
            index: 4,
            object_type: ObjectType::AnimationTrack,
            data: track.into(),
            section_offset: 64,
        });
        if incompatible_target {
            let mut background = Vec::new();
            // One attached track followed by no user parameters, color or image.
            for field in [0_u32, 1, 4, 0, 0, 0] {
                background.extend_from_slice(&field.to_le_bytes());
            }
            background.extend_from_slice(&[32, 32]);
            background.extend_from_slice(&[0; 16]);
            background.extend_from_slice(&[1, 1]);
            file.objects.push(ParsedObject {
                index: 5,
                object_type: ObjectType::Background,
                data: background.into(),
                section_offset: 64,
            });
        }
        let mut external = vec![None; file.objects.len()];
        external[1] = Some(sequence);
        let guests: Vec<_> = (0..file.objects.len())
            .map(|index| Some(100 + index as u64))
            .collect();
        let error = instantiate_file(&file, &mut runtime, &guests, &external).unwrap_err();
        assert_eq!(
            error.code(),
            if incompatible_target {
                "invalid-animation-target"
            } else {
                "invalid-animation-components"
            }
        );
        assert_eq!(
            (runtime.counters().0, runtime.counters().2),
            (before.0, before.2)
        );
        assert_eq!(runtime.resolve_guest(11).unwrap(), sequence);
        for guest in guests.into_iter().flatten() {
            assert!(runtime.resolve_guest(guest).is_err());
        }
    }
}

#[test]
fn validated_group_instantiates_with_guest_binding_and_state() {
    let mut data = Vec::new();
    data.extend_from_slice(&42_u32.to_le_bytes());
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&2_u32.to_le_bytes());
    data.extend_from_slice(&u32::MAX.to_le_bytes());
    data.extend_from_slice(&3_u32.to_le_bytes());
    data.extend_from_slice(&[1, 2, 3]);
    data.extend_from_slice(&7_u32.to_le_bytes());
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.push(9);
    data.extend_from_slice(&[0, 0]);
    data.extend_from_slice(&[1, 1, 255]);
    data.extend_from_slice(&u32::MAX.to_le_bytes());
    data.push(0);
    data.extend_from_slice(&0_u32.to_le_bytes());
    let file = file_with_object(ObjectType::Group, data);
    let mut runtime = Runtime::default();
    let instantiated =
        instantiate_file(&file, &mut runtime, &[None, Some(77)], &[None, None]).unwrap();
    let group = instantiated.handles[1].unwrap();
    assert_eq!(runtime.resolve_guest(77).unwrap(), group);
    assert_eq!(runtime.user_id(group).unwrap(), 42);
    assert!(matches!(
        runtime.kind(group).unwrap(),
        ObjectKind::Group { .. }
    ));
    assert_eq!(
        runtime.node_state(group).unwrap(),
        (true, true, u32::MAX, 1.0)
    );
    assert_eq!(
        instantiated.user_parameters[1],
        [(u32::MAX, vec![1, 2, 3]), (7, vec![9])]
    );
}

#[test]
fn serialized_image_copies_required_indices_from_a_larger_array() {
    let mut data = vec![0; 12]; // Object3D user ID, tracks and parameters.
    data.push(100); // RGBA
    data.push(0); // immutable
    data.extend_from_slice(&2_u32.to_le_bytes());
    data.extend_from_slice(&2_u32.to_le_bytes());
    let mut palette = vec![0; 256 * 4];
    palette[..8].copy_from_slice(&[255, 0, 0, 255, 0, 255, 0, 128]);
    data.extend_from_slice(&(palette.len() as u32).to_le_bytes());
    data.extend_from_slice(&palette);
    let indices = [0, 1, 0, 1, 9, 9, 9, 9];
    data.extend_from_slice(&(indices.len() as u32).to_le_bytes());
    data.extend_from_slice(&indices);
    let file = file_with_object(ObjectType::Image2D, data);
    let mut runtime = Runtime::default();

    let instantiated =
        instantiate_file(&file, &mut runtime, &[None, Some(77)], &[None, None]).unwrap();

    let ObjectKind::Image2D(image) = runtime.kind(instantiated.handles[1].unwrap()).unwrap() else {
        panic!("serialized object must remain an Image2D");
    };
    assert_eq!(
        image.pixels(),
        [0xffff_0000, 0x8000_ff00, 0xffff_0000, 0x8000_ff00]
    );
}

#[test]
fn invalid_object_tables_are_rejected_without_changing_the_runtime() {
    let mut data = vec![0; 12];
    data.extend_from_slice(&[0, 0, 1, 1, 255]);
    data.extend_from_slice(&u32::MAX.to_le_bytes());
    data.extend_from_slice(&[0; 5]);
    let valid = file_with_object(ObjectType::Group, data);
    for case in 0..9 {
        let mut file = valid.clone();
        match case {
            0..=3 => file.objects[1].index = [0, 1, 3, u32::MAX][case],
            4 => file.objects[0].index = 0,
            5 => file.objects[0].object_type = ObjectType::Group,
            6 => file.objects[1].object_type = ObjectType::Header,
            7 => file.objects.clear(),
            _ => file.objects.push(file.objects[1].clone()),
        }
        let mut runtime = Runtime::default();
        let existing = runtime.create(Some(11), ObjectKind::Object).unwrap();
        let before = runtime.counters();
        let guests = vec![Some(77); file.objects.len()];
        let external = vec![None; file.objects.len()];
        let error = instantiate_file(&file, &mut runtime, &guests, &external).unwrap_err();
        assert_eq!(error.code(), "instantiate-shape", "case {case}");
        assert_eq!(runtime.counters(), before);
        assert_eq!(runtime.resolve_guest(11).unwrap(), existing);
        assert!(runtime.resolve_guest(77).is_err());
    }
}
