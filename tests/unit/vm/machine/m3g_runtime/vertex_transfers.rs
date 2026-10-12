use super::*;

const CLASS: &str = "javax/microedition/m3g/VertexArray";

fn vertex_array(
    machine: &mut Machine<'_, '_>,
    count: usize,
    component_type: m3g::VertexComponent,
) -> (Handle, m3g::Handle) {
    let guest = machine
        .heap
        .managed
        .allocate_object(CLASS, HashMap::new())
        .unwrap();
    let native = machine
        .m3g
        .runtime
        .create(
            Some(guest.to_raw()),
            m3g::ObjectKind::VertexArray(
                m3g::VertexArrayState::new(count, 3, component_type).unwrap(),
            ),
        )
        .unwrap();
    (guest, native)
}

#[test]
fn vertex_updates_preserve_other_ranges_and_reject_malformed_inputs_atomically() {
    for (component_type, kind, descriptor) in [
        (m3g::VertexComponent::Byte, ArrayKind::Byte, "(II[B)V"),
        (m3g::VertexComponent::Short, ArrayKind::Short, "(II[S)V"),
    ] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let (guest, native) = vertex_array(&mut machine, 4, component_type);
        let values = machine.heap.managed.allocate_array(kind, 7).unwrap();
        let input = [i32::MIN, -32769, -129, 128, 32768, i32::MAX];
        for (index, value) in input.into_iter().enumerate() {
            machine
                .heap
                .managed
                .array_set(values, index as i32, HeapValue::Int(value))
                .unwrap();
        }
        // Only the consumed prefix must contain integer tags.
        let Allocation::Array { elements, .. } = machine.heap.managed.get_mut(values).unwrap()
        else {
            panic!("expected array");
        };
        elements[6] = HeapValue::Float(1.0);
        let args = [
            Value::Reference(Some(guest)),
            Value::Int(1),
            Value::Int(2),
            Value::Reference(Some(values)),
        ];
        machine
            .invoke_m3g_object_native(CLASS, "set", descriptor, &args)
            .unwrap();
        let m3g::ObjectKind::VertexArray(state) = machine.m3g.runtime.kind(native).unwrap() else {
            panic!("expected vertex array");
        };
        let snapshot = state.clone();
        let actual = snapshot.components(0, 4, component_type).unwrap();
        assert_eq!(&actual[..3], &[0; 3]);
        assert_eq!(&actual[9..], &[0; 3]);
        for (&actual, expected) in actual[3..9].iter().zip(input) {
            assert_eq!(
                actual,
                if component_type == m3g::VertexComponent::Byte {
                    i16::from(expected as i8)
                } else {
                    expected as i16
                }
            );
        }
        machine
            .heap
            .managed
            .array_set(values, 0, HeapValue::Int(7))
            .unwrap();
        let Allocation::Array { elements, .. } = machine.heap.managed.get_mut(values).unwrap()
        else {
            panic!("expected array");
        };
        elements[5] = HeapValue::Float(1.0);
        assert!(
            machine
                .invoke_m3g_object_native(CLASS, "set", descriptor, &args)
                .is_err()
        );
        let m3g::ObjectKind::VertexArray(state) = machine.m3g.runtime.kind(native).unwrap() else {
            panic!("expected vertex array");
        };
        assert_eq!(state, &snapshot);
        machine
            .heap
            .managed
            .array_set(values, 5, HeapValue::Int(-7))
            .unwrap();
        machine
            .invoke_m3g_object_native(CLASS, "set", descriptor, &args)
            .unwrap();
        let m3g::ObjectKind::VertexArray(state) = machine.m3g.runtime.kind(native).unwrap() else {
            panic!("expected vertex array");
        };
        assert_eq!(state.component(1, 0).unwrap(), 7);
        assert_eq!(snapshot.component(1, 0).unwrap(), 0);
    }
}

#[test]
#[ignore = "manual release throughput comparison"]
fn vertex_update_throughput() {
    use std::{hint::black_box, time::Instant};

    for (component_type, kind, descriptor) in [
        (m3g::VertexComponent::Byte, ArrayKind::Byte, "(II[B)V"),
        (m3g::VertexComponent::Short, ArrayKind::Short, "(II[S)V"),
    ] {
        for count in [0, 1, 16, 1024, 16384] {
            let program = Program::new();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let (guest, native) = vertex_array(&mut machine, count.max(1), component_type);
            let values = machine
                .heap
                .managed
                .allocate_array(kind.clone(), (count * 3) as i32)
                .unwrap();
            for index in 0..count * 3 {
                machine
                    .heap
                    .managed
                    .array_set(
                        values,
                        index as i32,
                        HeapValue::Int(index as i32 * 177 - 37),
                    )
                    .unwrap();
            }
            let args = [
                Value::Reference(Some(guest)),
                Value::Int(0),
                Value::Int(count as i32),
                Value::Reference(Some(values)),
            ];
            let start = Instant::now();
            for _ in 0..4096 {
                black_box(
                    machine
                        .invoke_m3g_object_native(CLASS, "set", descriptor, black_box(&args))
                        .unwrap(),
                );
            }
            let elapsed = start.elapsed();
            let m3g::ObjectKind::VertexArray(state) = machine.m3g.runtime.kind(native).unwrap()
            else {
                panic!("expected vertex array");
            };
            let checksum = state
                .components(0, count, component_type)
                .unwrap()
                .iter()
                .fold(0_i64, |sum, &value| {
                    sum.wrapping_mul(31).wrapping_add(i64::from(value))
                });
            eprintln!(
                "vertex-update-{component_type:?}-{count}: elapsed={elapsed:?} checksum={checksum}"
            );
        }
    }
}
