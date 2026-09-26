use super::*;

fn class_with_methods(descriptor: &str, flags: &[u16]) -> Vec<u8> {
    let mut pool = Vec::new();
    for (text, class_index) in [
        ("test/Parameters", Some(1_u16)),
        ("java/lang/Object", Some(3)),
        ("method", None),
        (descriptor, None),
    ] {
        pool.push(1);
        pool.extend_from_slice(&u16::try_from(text.len()).unwrap().to_be_bytes());
        pool.extend_from_slice(text.as_bytes());
        if let Some(index) = class_index {
            pool.push(7);
            pool.extend_from_slice(&index.to_be_bytes());
        }
    }
    let mut bytes = class_with_pool(7, &pool);
    for value in [0x0021, 2, 4, 0, 0, u16::try_from(flags.len()).unwrap()] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    for &access_flags in flags {
        for value in [access_flags, 5, 6, 0] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes
}

#[test]
fn parameter_slots_are_limited_for_static_and_instance_methods() {
    for (component, width) in [
        ("I", 1),
        ("J", 2),
        ("D", 2),
        ("[J", 1),
        ("[[D", 1),
        ("Ljava/lang/Object;", 1),
    ] {
        for count in [127, 128, 254, 255, 256] {
            // Return values do not consume parameter slots.
            let descriptor = format!("({})D", component.repeat(count));
            for (flags, receiver) in [(0x0109, 0), (0x0101, 1), (0x0401, 1)] {
                let result = parse(&class_with_methods(&descriptor, &[flags]));
                let valid = count * width + receiver <= 255;
                assert_eq!(
                    result.is_ok(),
                    valid,
                    "{component} * {count}, flags={flags:x}"
                );
                if let Err(error) = result {
                    assert_eq!(error.code(), "invalid-descriptor");
                }
            }
        }
    }
    for count in [254, 255, 256, 60_000] {
        let descriptor = format!("({})V", "I".repeat(count));
        let mut budget = 2 * 1024 * 1024;
        let result = parse_with_budget(&class_with_methods(&descriptor, &[0x0109]), &mut budget);
        assert_eq!(result.is_ok(), count <= 255);
        if result.is_err() {
            assert_eq!(budget, 2 * 1024 * 1024);
        }
    }
}

#[test]
fn cached_parameter_slots_keep_the_instance_receiver_limit() {
    let descriptor = format!("({})V", "J".repeat(127));
    assert!(parse(&class_with_methods(&descriptor, &[0x0109, 0x0101])).is_ok());
    let descriptor = format!("({}I)V", "J".repeat(127));
    assert!(parse(&class_with_methods(&descriptor, &[0x0109])).is_ok());
    let result = parse(&class_with_methods(&descriptor, &[0x0109, 0x0101]));
    assert!(matches!(result, Err(error) if error.code() == "invalid-descriptor"));
}
