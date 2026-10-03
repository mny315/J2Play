use super::*;

#[test]
fn string_index_of_is_linear_and_preserves_utf16_semantics() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let value = machine
        .allocate_dynamic_string("aabaabaac", &[], &[])
        .unwrap();
    let needle = machine.allocate_dynamic_string("aabaac", &[], &[]).unwrap();
    let empty = machine.allocate_dynamic_string("", &[], &[]).unwrap();
    assert_eq!(machine.string_index_of(value, needle, 0).unwrap(), 3);
    assert_eq!(machine.string_index_of(value, needle, 4).unwrap(), -1);
    assert_eq!(machine.string_index_of(value, empty, -1).unwrap(), 0);
    assert_eq!(machine.string_index_of(value, empty, 99).unwrap(), 9);

    let surrogate_value = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .store_string_units(surrogate_value, vec![0xd800, 1, 0xd800, 2], 1, &[], &[])
        .unwrap();
    let surrogate_needle = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .store_string_units(surrogate_needle, vec![0xd800, 2], 1, &[], &[])
        .unwrap();
    assert_eq!(
        machine
            .string_index_of(surrogate_value, surrogate_needle, 0)
            .unwrap(),
        2
    );

    let mut periodic_value = vec![u16::from(b'a'); 100_000];
    periodic_value.push(u16::from(b'b'));
    let mut periodic_needle = vec![u16::from(b'a'); 50_000];
    periodic_needle.push(u16::from(b'c'));
    let long_value = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .store_string_units(long_value, periodic_value, 1, &[], &[])
        .unwrap();
    let long_needle = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .store_string_units(long_needle, periodic_needle, 1, &[], &[])
        .unwrap();
    assert_eq!(
        machine.string_index_of(long_value, long_needle, 0).unwrap(),
        -1
    );
}

#[test]
fn two_way_utf16_search_matches_exhaustive_small_inputs() {
    fn sequence(mut code: usize, length: usize, radix: usize) -> Vec<u16> {
        (0..length)
            .map(|_| {
                let unit = u16::try_from(code % radix).unwrap();
                code /= radix;
                unit
            })
            .collect()
    }

    for (radix, max_haystack, max_needle) in [(2usize, 8usize, 5usize), (3, 6, 4)] {
        for haystack_len in 0..=max_haystack {
            for haystack_code in 0..radix.pow(u32::try_from(haystack_len).unwrap()) {
                let haystack = sequence(haystack_code, haystack_len, radix);
                for needle_len in 0..=max_needle {
                    for needle_code in 0..radix.pow(u32::try_from(needle_len).unwrap()) {
                        let needle = sequence(needle_code, needle_len, radix);
                        let expected = if needle.is_empty() {
                            Some(0)
                        } else {
                            haystack
                                .windows(needle.len())
                                .position(|window| window == needle)
                        };
                        assert_eq!(
                            two_way_utf16_index(&haystack, &needle, || Ok(())).unwrap(),
                            expected,
                            "radix={radix} haystack={haystack:?} needle={needle:?}"
                        );
                    }
                }
            }
        }
    }
}
