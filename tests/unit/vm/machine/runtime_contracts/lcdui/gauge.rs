use super::*;

const GAUGE: &str = "javax/microedition/lcdui/Gauge";

fn gauge(machine: &mut Machine<'_, '_>, interactive: bool, maximum: i32, value: i32) -> Handle {
    let receiver = machine.allocate_native_instance(GAUGE, &[]).unwrap();
    assert!(matches!(
        invoke(
            machine,
            GAUGE,
            receiver,
            "<init>",
            "(Ljava/lang/String;ZII)V",
            &[
                Value::Reference(None),
                Value::Int(i32::from(interactive)),
                Value::Int(maximum),
                Value::Int(value),
            ]
        ),
        CallOutcome::Return(None)
    ));
    receiver
}

fn value(machine: &mut Machine<'_, '_>, gauge: Handle, name: &str) -> i32 {
    let CallOutcome::Return(Some(Value::Int(value))) =
        invoke(machine, GAUGE, gauge, name, "()I", &[])
    else {
        panic!("Gauge.{name} did not return an integer");
    };
    value
}

fn set(machine: &mut Machine<'_, '_>, gauge: Handle, name: &str, value: i32) {
    assert!(matches!(
        invoke(machine, GAUGE, gauge, name, "(I)V", &[Value::Int(value)]),
        CallOutcome::Return(None)
    ));
}

#[test]
fn gauge_user_input_saturates_at_integer_and_range_boundaries() {
    let program = program();
    for interactive in [false, true] {
        for maximum in [1, 10, i32::MAX] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let gauge = gauge(&mut machine, interactive, maximum, 0);
            for initial in [0, maximum - 1, maximum] {
                for key in [-3, 52, -4, 54, -1, 50, -5, 53] {
                    set(&mut machine, gauge, "setValue", initial);
                    set(&mut machine, gauge, "__key", key);
                    let expected = match (interactive, key) {
                        (true, -3 | 52) => (initial - 1).max(0),
                        (true, -4 | 54) => initial.saturating_add(1).min(maximum),
                        _ => initial,
                    };
                    assert_eq!(
                        value(&mut machine, gauge, "getValue"),
                        expected,
                        "interactive={interactive}, max={maximum}, initial={initial}, key={key}"
                    );
                }
            }
        }
    }
}

#[test]
fn gauge_range_changes_reset_indefinite_state_and_preserve_definite_values() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let gauge = gauge(&mut machine, false, 10, 7);
    for (maximum, expected) in [(20, 7), (5, 5), (-1, 0), (-1, 0)] {
        set(&mut machine, gauge, "setMaxValue", maximum);
        assert_eq!(value(&mut machine, gauge, "getMaxValue"), maximum);
        assert_eq!(value(&mut machine, gauge, "getValue"), expected);
    }
    for state in 0..=3 {
        set(&mut machine, gauge, "setValue", state);
        set(&mut machine, gauge, "setMaxValue", -1);
        assert_eq!(value(&mut machine, gauge, "getValue"), state);
        set(&mut machine, gauge, "setMaxValue", 10);
        assert_eq!(value(&mut machine, gauge, "getValue"), 0);
        set(&mut machine, gauge, "setMaxValue", -1);
    }
    for invalid in [i32::MIN, -2, 0] {
        let CallOutcome::Throw(exception) = invoke(
            &mut machine,
            GAUGE,
            gauge,
            "setMaxValue",
            "(I)V",
            &[Value::Int(invalid)],
        ) else {
            panic!("invalid range was accepted");
        };
        assert_eq!(
            machine.object_class(exception).unwrap(),
            "java/lang/IllegalArgumentException"
        );
        assert_eq!(value(&mut machine, gauge, "getMaxValue"), -1);
        assert_eq!(value(&mut machine, gauge, "getValue"), 0);
    }
}

#[test]
fn gauge_range_and_key_updates_survive_collection_at_each_yield() {
    let program = program();
    for interactive in [false, true] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let gauge = gauge(&mut machine, interactive, i32::MAX, i32::MAX);
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
        for (name, argument, expected_max, expected_value) in [
            ("__key", -4, i32::MAX, i32::MAX),
            ("setMaxValue", 5, 5, 5),
            (
                "setMaxValue",
                -1,
                if interactive { 5 } else { -1 },
                if interactive { 5 } else { 0 },
            ),
            (
                "setMaxValue",
                0,
                if interactive { 5 } else { -1 },
                if interactive { 5 } else { 0 },
            ),
            ("setMaxValue", 10, 10, if interactive { 5 } else { 0 }),
        ] {
            let outcome = invoke_collecting(
                &mut machine,
                GAUGE,
                gauge,
                name,
                "(I)V",
                &[Value::Int(argument)],
                1,
            );
            if name == "setMaxValue" && (argument == 0 || (interactive && argument == -1)) {
                let CallOutcome::Throw(exception) = outcome else {
                    panic!("invalid maximum accepted");
                };
                assert_eq!(
                    machine.object_class(exception).unwrap(),
                    "java/lang/IllegalArgumentException"
                );
            } else {
                assert!(matches!(outcome, CallOutcome::Return(None)));
            }
            machine.scheduler.quantum_remaining = u64::MAX;
            assert_eq!(value(&mut machine, gauge, "getMaxValue"), expected_max);
            assert_eq!(value(&mut machine, gauge, "getValue"), expected_value);
        }
    }
}

#[test]
fn gauge_paint_scales_large_values_without_integer_overflow() {
    let program = program();
    for maximum in [1, 100, i32::MAX] {
        for current in [0, maximum / 2, maximum - 1, maximum] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let gauge = gauge(&mut machine, false, maximum, current);
            let image_class = "javax/microedition/lcdui/Image";
            let method = &program.methods[&MethodKey {
                class: image_class.into(),
                name: "createImage".into(),
                descriptor: "(II)Ljavax/microedition/lcdui/Image;".into(),
            }];
            let CallOutcome::Return(Some(Value::Reference(Some(image)))) = machine
                .call(method, [Value::Int(64), Value::Int(12)], 1)
                .unwrap()
            else {
                panic!("image creation failed");
            };
            let CallOutcome::Return(Some(Value::Reference(Some(graphics)))) = invoke(
                &mut machine,
                image_class,
                image,
                "getGraphics",
                "()Ljavax/microedition/lcdui/Graphics;",
                &[],
            ) else {
                panic!("image graphics unavailable");
            };
            assert!(matches!(
                invoke(
                    &mut machine,
                    GAUGE,
                    gauge,
                    "__paint",
                    "(Ljavax/microedition/lcdui/Graphics;IIIZ)V",
                    &[
                        Value::Reference(Some(graphics)),
                        Value::Int(0),
                        Value::Int(0),
                        Value::Int(64),
                        Value::Int(0),
                    ]
                ),
                CallOutcome::Return(None)
            ));
            let pixels = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Int, 64)
                .unwrap();
            assert!(matches!(
                invoke(
                    &mut machine,
                    image_class,
                    image,
                    "getRGB",
                    "([IIIIIII)V",
                    &[
                        Value::Reference(Some(pixels)),
                        Value::Int(0),
                        Value::Int(64),
                        Value::Int(0),
                        Value::Int(5),
                        Value::Int(64),
                        Value::Int(1),
                    ]
                ),
                CallOutcome::Return(None)
            ));
            let expected = i64::from(current) * 62 / i64::from(maximum);
            for x in 1..63 {
                let pixel = machine.heap.managed.array_get(pixels, x).unwrap();
                assert_eq!(
                    pixel != Value::Int(-1),
                    i64::from(x) <= expected,
                    "max={maximum}, current={current}, x={x}, pixel={pixel:?}"
                );
            }
        }
    }
}
