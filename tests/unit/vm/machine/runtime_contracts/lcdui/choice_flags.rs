use super::*;

const CLASSES: [(&str, [i32; 3]); 2] = [
    ("javax/microedition/lcdui/List", [1, 2, 3]),
    ("javax/microedition/lcdui/ChoiceGroup", [1, 2, 4]),
];

fn choice(machine: &mut Machine<'_, '_>, class: &str, kind: i32, count: i32) -> Handle {
    let receiver = machine.allocate_native_instance(class, &[]).unwrap();
    match invoke(
        machine,
        class,
        receiver,
        "<init>",
        "(Ljava/lang/String;I)V",
        &[Value::Reference(None), Value::Int(kind)],
    ) {
        CallOutcome::Return(None) => {}
        CallOutcome::Throw(exception) => panic!(
            "{class} constructor threw {}: {:?}",
            machine.object_class(exception).unwrap(),
            machine.throwable_diagnostic_message(exception)
        ),
        _ => panic!("{class} constructor did not complete"),
    }
    let text = machine.allocate_dynamic_string("entry", &[], &[]).unwrap();
    for index in 0..count {
        assert!(matches!(
            invoke(machine, class, receiver, "append", "(Ljava/lang/String;Ljavax/microedition/lcdui/Image;)I", &[Value::Reference(Some(text)), Value::Reference(None)]),
            CallOutcome::Return(Some(Value::Int(value))) if value == index
        ));
    }
    receiver
}

fn flags(machine: &mut Machine<'_, '_>, values: &[bool]) -> Handle {
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Boolean, i32::try_from(values.len()).unwrap())
        .unwrap();
    for (index, value) in values.iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(
                array,
                i32::try_from(index).unwrap(),
                Value::Int(i32::from(*value)),
            )
            .unwrap();
    }
    array
}

fn read_flags(machine: &Machine<'_, '_>, array: Handle) -> Vec<bool> {
    (0..machine.heap.managed.array_length(array).unwrap())
        .map(|index| matches!(machine.heap.managed.array_get(array, i32::try_from(index).unwrap()).unwrap(), Value::Int(value) if value != 0))
        .collect()
}

fn assert_selection(
    machine: &mut Machine<'_, '_>,
    class: &str,
    receiver: Handle,
    expected: &[bool],
) {
    for (index, expected) in expected.iter().enumerate() {
        assert!(
            matches!(
                invoke(machine, class, receiver, "isSelected", "(I)Z", &[Value::Int(i32::try_from(index).unwrap())]),
                CallOutcome::Return(Some(Value::Int(value))) if (value != 0) == *expected
            ),
            "{class} entry {index}"
        );
    }
}

#[test]
fn choice_flags_reject_short_and_null_arrays_before_changing_state() {
    let program = program();
    for (class, kinds) in CLASSES {
        for kind in kinds {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let receiver = choice(&mut machine, class, kind, 3);
            assert!(matches!(
                invoke(
                    &mut machine,
                    class,
                    receiver,
                    "setSelectedIndex",
                    "(IZ)V",
                    &[Value::Int(2), Value::Int(1)]
                ),
                CallOutcome::Return(None)
            ));
            for length in [None, Some(0), Some(1), Some(2)] {
                let array = length.map(|length| flags(&mut machine, &vec![true; length]));
                for (name, descriptor) in
                    [("getSelectedFlags", "([Z)I"), ("setSelectedFlags", "([Z)V")]
                {
                    let outcome = invoke(
                        &mut machine,
                        class,
                        receiver,
                        name,
                        descriptor,
                        &[Value::Reference(array)],
                    );
                    let CallOutcome::Throw(exception) = outcome else {
                        panic!("{class}.{name} accepted {length:?} flags for three entries");
                    };
                    assert_eq!(
                        machine.object_class(exception).unwrap(),
                        if length.is_none() {
                            "java/lang/NullPointerException"
                        } else {
                            "java/lang/IllegalArgumentException"
                        }
                    );
                    if let Some(array) = array {
                        assert_eq!(read_flags(&machine, array), vec![true; length.unwrap()]);
                    }
                    assert_selection(&mut machine, class, receiver, &[false, false, true]);
                }
            }
        }
    }
}

#[test]
fn choice_flags_apply_single_and_multiple_selection_and_clear_extra_output() {
    let program = program();
    for (class, kinds) in CLASSES {
        for kind in kinds {
            for count in [0, 1, 3] {
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let receiver = choice(&mut machine, class, kind, count);
                for extra in [0, 2] {
                    for pattern in [0_u8, 7, 6, 4, 0] {
                        let input: Vec<_> = (0..count + extra)
                            .map(|index| index >= count || pattern & (1 << index) != 0)
                            .collect();
                        let array = flags(&mut machine, &input);
                        assert!(matches!(
                            invoke(
                                &mut machine,
                                class,
                                receiver,
                                "setSelectedFlags",
                                "([Z)V",
                                &[Value::Reference(Some(array))]
                            ),
                            CallOutcome::Return(None)
                        ));
                        assert_eq!(read_flags(&machine, array), input);
                        let mut expected = input[..usize::try_from(count).unwrap()].to_vec();
                        if kind != 2 && count > 0 {
                            let selected = expected.iter().position(|value| *value).unwrap_or(0);
                            expected.fill(false);
                            expected[selected] = true;
                        }
                        assert_selection(&mut machine, class, receiver, &expected);
                        let output = flags(&mut machine, &vec![true; input.len()]);
                        let selected =
                            i32::try_from(expected.iter().filter(|value| **value).count()).unwrap();
                        assert!(
                            matches!(invoke(&mut machine, class, receiver, "getSelectedFlags", "([Z)I", &[Value::Reference(Some(output))]), CallOutcome::Return(Some(Value::Int(value))) if value == selected)
                        );
                        expected.resize(input.len(), false);
                        assert_eq!(read_flags(&machine, output), expected);
                    }
                }
            }
        }
    }
}

#[test]
fn choice_flags_keep_arrays_and_entries_alive_across_yields() {
    let program = program();
    for (class, kinds) in CLASSES {
        for kind in kinds {
            for quantum in [1, 4, 64] {
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                let receiver = choice(&mut machine, class, kind, 3);
                let worker = machine
                    .heap
                    .managed
                    .allocate_object("java/lang/Thread", HashMap::new())
                    .unwrap();
                machine.scheduler.current_thread = worker.to_raw();
                machine
                    .scheduler
                    .thread_states
                    .insert(worker, ThreadState::Running);
                for input in [[false, true, true, true], [false; 4]] {
                    let array = flags(&mut machine, &input);
                    for (name, descriptor) in
                        [("setSelectedFlags", "([Z)V"), ("getSelectedFlags", "([Z)I")]
                    {
                        let outcome = invoke_collecting(
                            &mut machine,
                            class,
                            receiver,
                            name,
                            descriptor,
                            &[Value::Reference(Some(array))],
                            quantum,
                        );
                        if name == "setSelectedFlags" {
                            assert!(matches!(outcome, CallOutcome::Return(None)));
                            assert_eq!(read_flags(&machine, array), input);
                        } else {
                            let expected = if kind == 2 {
                                [false, input[1], input[2], false]
                            } else {
                                [!input[1], input[1], false, false]
                            };
                            let count =
                                i32::try_from(expected.iter().filter(|flag| **flag).count())
                                    .unwrap();
                            assert!(
                                matches!(outcome, CallOutcome::Return(Some(Value::Int(value))) if value == count)
                            );
                            assert_eq!(read_flags(&machine, array), expected);
                        }
                    }
                }
            }
        }
    }
}
