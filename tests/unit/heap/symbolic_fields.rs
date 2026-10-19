use super::*;

#[test]
fn symbolic_fields_preserve_exact_names_across_lengths_and_shared_ends() {
    let mut names = vec![String::new(), "é".into(), "甲乙.値:Ｉ".into()];
    for length in 1..=65 {
        let common = "a".repeat(length - 1);
        names.push(format!("x{common}"));
        names.push(format!("{common}y"));
        names.push(format!("é{common}"));
    }
    names.sort_unstable();
    names.dedup();
    let mut heap = Heap::new(64 * 1024);
    let object = heap
        .allocate_object(
            "Fields",
            names
                .iter()
                .enumerate()
                .map(|(index, name)| (name.clone(), HeapValue::Int(i32::try_from(index).unwrap())))
                .collect(),
        )
        .unwrap();
    let wrong_token = FieldToken::new();
    for (index, name) in names.iter().enumerate() {
        let original = HeapValue::Int(i32::try_from(index).unwrap());
        assert_eq!(heap.field(object, name), Ok(original));
        assert_eq!(
            heap.field_at(object, usize::MAX, &wrong_token, name),
            Ok(original)
        );
        let updated = HeapValue::Int(-(i32::try_from(index).unwrap()) - 1);
        heap.set_field(object, name, updated).unwrap();
        assert_eq!(heap.field(object, name), Ok(updated));
        heap.set_field_at(object, 0, &wrong_token, name, original)
            .unwrap();
    }
    for missing in ["z", "aaaaaaaz", "zaaaaaaa", "éaaaaaaaaz", "甲乙.値:Ｊ"] {
        assert_eq!(heap.field(object, missing), Err(HeapError::TypeMismatch));
        assert_eq!(
            heap.set_field(object, missing, HeapValue::Int(1)),
            Err(HeapError::TypeMismatch)
        );
    }
    for (index, name) in names.iter().enumerate() {
        assert_eq!(
            heap.field(object, name),
            Ok(HeapValue::Int(i32::try_from(index).unwrap()))
        );
    }
}

#[test]
#[ignore = "manual exact symbolic field access throughput measurement"]
#[allow(clippy::cast_sign_loss)] // This fixture stores only integers from 0 to 15.
fn symbolic_field_access_throughput() {
    use std::{hint::black_box, time::Instant};
    for length in [1, 7, 8, 9, 32, 64] {
        for different_end in [false, true] {
            let names: Vec<String> = (0..16)
                .map(|index| {
                    let distinct = char::from(b'A' + index);
                    let common = "a".repeat(length - 1);
                    if different_end {
                        format!("{common}{distinct}")
                    } else {
                        format!("{distinct}{common}")
                    }
                })
                .collect();
            let mut heap = Heap::new(4096);
            let mut fields: Vec<_> = names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    (
                        Arc::<str>::from(name.as_str()),
                        FieldToken::new(),
                        HeapValue::Int(i32::try_from(index).unwrap()),
                    )
                })
                .collect();
            let object = heap.allocate_object_linked("Fields", &mut fields).unwrap();
            let started = Instant::now();
            let mut checksum = 0_u64;
            for _ in 0..65_536 {
                for name in &names {
                    let HeapValue::Int(value) =
                        heap.field(black_box(object), black_box(name)).unwrap()
                    else {
                        panic!("integer field expected")
                    };
                    checksum += value as u64;
                }
            }
            let elapsed = started.elapsed();
            assert_eq!(checksum, 65_536 * 120);
            eprintln!(
                "symbolic-fields length={length} different_end={different_end} elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}
