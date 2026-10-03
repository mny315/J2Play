use super::*;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::hint::black_box;
use std::io::{Cursor, Read};
use std::time::Instant;

fn extended_pool(tag: u8, count: u16) -> Vec<u8> {
    let bytes = minimal_class();
    let prefix = parse_prefix(&bytes).unwrap();
    let mut expanded = bytes[..prefix.next_offset].to_vec();
    expanded[8..10].copy_from_slice(&(8 + count).to_be_bytes());
    for index in 0..count {
        expanded.push(tag);
        if tag == 7 {
            expanded.extend_from_slice(&1_u16.to_be_bytes());
        } else {
            expanded.extend_from_slice(&u32::from(index).to_be_bytes());
        }
    }
    expanded.extend_from_slice(&bytes[prefix.next_offset..]);
    expanded
}

fn fixture_classes() -> Vec<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(include_bytes!(
        "../../fixtures/java-me/conformance.jar"
    )))
    .unwrap();
    let mut classes = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        if std::path::Path::new(file.name())
            .extension()
            .is_some_and(|extension| extension == "class")
        {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            classes.push(bytes);
        }
    }
    assert!(!classes.is_empty());
    classes
}

#[test]
#[ignore = "manual release benchmark for class parsing"]
fn parse_throughput() {
    for (label, inputs, iterations) in [
        ("minimal", vec![minimal_class()], 32_768),
        ("reference-pool", vec![extended_pool(7, 4096)], 256),
        ("integer-pool", vec![extended_pool(3, 4096)], 256),
        ("conformance-classes", fixture_classes(), 128),
    ] {
        let expected: Vec<_> = inputs.iter().map(|bytes| parse(bytes).unwrap()).collect();
        let mut signature = DefaultHasher::new();
        format!("{expected:?}").hash(&mut signature);
        let started = Instant::now();
        for _ in 0..iterations {
            for bytes in &inputs {
                black_box(parse(black_box(bytes)).unwrap());
            }
        }
        let elapsed = started.elapsed();
        for (bytes, expected) in inputs.iter().zip(&expected) {
            assert_eq!(parse(bytes).unwrap(), *expected);
        }
        eprintln!(
            "classfile-{label} elapsed={elapsed:?} classes={} signature={:016x}",
            inputs.len(),
            signature.finish(),
        );
    }
}
