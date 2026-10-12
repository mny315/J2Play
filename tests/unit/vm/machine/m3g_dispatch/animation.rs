use super::*;
use crate::machine::{
    ArrayKind, DefaultNativeContext, HashMap, HeapValue, Limits, Program, java_error_class,
};

#[test]
fn keyframe_timestamp_query_accepts_a_null_output_and_still_validates_index() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/KeyframeSequence", HashMap::new())
        .unwrap();
    let mut sequence = m3g::KeyframeSequenceState::new(1, 3, m3g::Interpolation::Linear).unwrap();
    sequence.set_keyframe(0, 41, &[1.0, 2.0, 3.0]).unwrap();
    machine
        .m3g
        .runtime
        .create(
            Some(guest.to_raw()),
            m3g::ObjectKind::KeyframeSequence(sequence),
        )
        .unwrap();
    let query = |machine: &mut Machine<'_, '_>, index, destination| {
        machine.invoke_m3g_animation_native(
            "javax/microedition/m3g/KeyframeSequence",
            "getKeyframe",
            "(I[F)I",
            &[
                Value::Reference(Some(guest)),
                Value::Int(index),
                Value::Reference(destination),
            ],
        )
    };
    assert!(matches!(
        query(&mut machine, 0, None).unwrap(),
        CallOutcome::Return(Some(Value::Int(41)))
    ));
    for index in [-1, 1, i32::MAX] {
        assert_eq!(
            java_error_class(
                &query(&mut machine, index, None)
                    .err()
                    .expect("invalid keyframe index")
            ),
            Some("java/lang/IndexOutOfBoundsException")
        );
    }
    let output = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 5)
        .unwrap();
    machine.m3g_write_float_array(output, &[7.0; 5]).unwrap();
    assert!(matches!(
        query(&mut machine, 0, Some(output)).unwrap(),
        CallOutcome::Return(Some(Value::Int(41)))
    ));
    assert_eq!(
        machine.m3g_float_array(output, 5).unwrap(),
        [1.0, 2.0, 3.0, 7.0, 7.0]
    );
    let short = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 1)
        .unwrap();
    machine.m3g_write_float_array(short, &[7.0]).unwrap();
    assert!(query(&mut machine, 0, Some(short)).is_err());
    assert_eq!(
        machine.heap.managed.array_get(short, 0).unwrap(),
        HeapValue::Float(7.0)
    );
}

#[test]
fn keyframe_setters_report_index_exceptions_without_changing_the_sequence() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/KeyframeSequence", HashMap::new())
        .unwrap();
    let mut sequence = m3g::KeyframeSequenceState::new(2, 1, m3g::Interpolation::Linear).unwrap();
    sequence.set_keyframe(0, 10, &[0.25]).unwrap();
    sequence.set_keyframe(1, 20, &[0.75]).unwrap();
    let native = machine
        .m3g
        .runtime
        .create(
            Some(guest.to_raw()),
            m3g::ObjectKind::KeyframeSequence(sequence.clone()),
        )
        .unwrap();
    let input = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 1)
        .unwrap();
    machine.m3g_write_float_array(input, &[1.0]).unwrap();
    for invalid in [-1, 2, i32::MAX] {
        for (method, descriptor, arguments) in [
            (
                "setKeyframe",
                "(II[F)V",
                vec![
                    Value::Int(invalid),
                    Value::Int(15),
                    Value::Reference(Some(input)),
                ],
            ),
            (
                "setValidRange",
                "(II)V",
                vec![Value::Int(invalid), Value::Int(0)],
            ),
            (
                "setValidRange",
                "(II)V",
                vec![Value::Int(0), Value::Int(invalid)],
            ),
        ] {
            let mut args = vec![Value::Reference(Some(guest))];
            args.extend(arguments);
            let error = machine
                .invoke_m3g_animation_native(
                    "javax/microedition/m3g/KeyframeSequence",
                    method,
                    descriptor,
                    &args,
                )
                .err()
                .expect("invalid keyframe index");
            assert_eq!(
                java_error_class(&error),
                Some("java/lang/IndexOutOfBoundsException")
            );
            let m3g::ObjectKind::KeyframeSequence(current) =
                machine.m3g.runtime.kind(native).unwrap()
            else {
                panic!("expected keyframes");
            };
            assert_eq!(current, &sequence);
        }
    }
}
