use super::*;

const BUFFER: &str = "java/lang/StringBuffer";

#[test]
#[ignore = "manual release throughput measurement for StringBuffer appends"]
fn string_buffer_append_throughput() {
    use std::{hint::black_box, time::Duration, time::Instant};
    const APPENDS: usize = 128;
    let program = program_with_core_natives();
    for (kind, units) in [
        ("char", vec![0xd800]),
        ("empty", vec![]),
        ("short", vec![65]),
        ("medium", (0..16).collect()),
        ("long", (0..256).collect()),
    ] {
        for growth in [false, true] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let source = machine
                .allocate_object("java/lang/String", HashMap::new(), &[], &[])
                .unwrap();
            machine
                .store_string_units(source, units.clone(), 1, &[], &[])
                .unwrap();
            let buffer = machine.allocate_native_instance(BUFFER, &[]).unwrap();
            let append = &program.methods[&MethodKey {
                class: BUFFER.into(),
                name: "append".into(),
                descriptor: if kind == "char" {
                    "(C)Ljava/lang/StringBuffer;"
                } else {
                    "(Ljava/lang/String;)Ljava/lang/StringBuffer;"
                }
                .into(),
            }];
            let args = [
                Value::Reference(Some(buffer)),
                if kind == "char" {
                    Value::Int(i32::from(units[0]))
                } else {
                    Value::Reference(Some(source))
                },
            ];
            let mut elapsed = Duration::ZERO;
            let mut checksum = 0_u64;
            for _ in 0..64 {
                let capacity = if growth { 16 } else { units.len() * APPENDS };
                let backing = machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Char, capacity as i32)
                    .unwrap();
                for (field, value) in [
                    ("java/lang/StringBuffer.count:I", HeapValue::Int(0)),
                    (
                        "java/lang/StringBuffer.value:[C",
                        HeapValue::Reference(Some(backing)),
                    ),
                ] {
                    machine
                        .heap
                        .managed
                        .set_field(buffer, field, value)
                        .unwrap();
                }
                let start = Instant::now();
                for _ in 0..APPENDS {
                    let outcome = machine.call(append, black_box(args), 1).unwrap();
                    assert!(matches!(
                        outcome,
                        CallOutcome::Return(Some(Value::Reference(Some(result)))) if result == buffer
                    ));
                }
                elapsed += start.elapsed();
                let count = machine
                    .graphics_int_field(buffer, "java/lang/StringBuffer.count:I")
                    .unwrap();
                assert_eq!(count as usize, units.len() * APPENDS);
                let backing = machine
                    .graphics_reference_field(buffer, "java/lang/StringBuffer.value:[C")
                    .unwrap();
                let Allocation::Array { elements, .. } = machine.heap.managed.get(backing).unwrap()
                else {
                    panic!("StringBuffer backing array");
                };
                for (value, expected) in elements[..count as usize].iter().zip(units.iter().cycle())
                {
                    assert_eq!(*value, HeapValue::Int(i32::from(*expected)));
                    checksum = checksum.wrapping_add(u64::from(*expected));
                }
                machine.collect_heap(vec![buffer, source]);
            }
            eprintln!(
                "string-buffer kind={kind} growth={growth} elapsed_ns={} checksum={checksum}",
                elapsed.as_nanos()
            );
        }
    }
}
