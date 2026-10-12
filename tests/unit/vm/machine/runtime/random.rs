use super::*;

#[test]
fn random_default_constructors_do_not_repeat_when_wall_clock_is_unchanged() {
    let program = random_test_program(None, false);
    let mut context = PacingContext {
        now_millis: 1_234,
        ..PacingContext::default()
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let allocate_random = |machine: &mut Machine<'_, '_>| {
        machine
            .heap
            .managed
            .allocate_object(
                "java/util/Random",
                HashMap::from([("java/util/Random.seed:J".to_owned(), HeapValue::Long(0))]),
            )
            .unwrap()
    };
    let first = allocate_random(&mut machine);
    let second = allocate_random(&mut machine);
    for random in [first, second] {
        assert!(matches!(
            call_random(&mut machine, random, "<init>", "()V").unwrap(),
            CallOutcome::Return(None)
        ));
    }

    assert_ne!(
        next_random_int(&mut machine, first),
        next_random_int(&mut machine, second)
    );
}

fn random_test_program(next_code: Option<&[u8]>, override_seed: bool) -> Program {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(240, 320) {
        let name = entry.class.class_name(entry.class.this_class);
        if !matches!(name, Some("java/lang/Object" | "java/util/Random")) {
            continue;
        }
        program.add_class(&entry.class, &Limits::default()).unwrap();
        if name != Some("java/util/Random") || (next_code.is_none() && !override_seed) {
            continue;
        }
        let mut subclass = entry.class.clone();
        let name_index = subclass.constant_pool.len() as u16;
        subclass
            .constant_pool
            .push(Some(Constant::Utf8("test/CustomRandom".into())));
        let this_class = subclass.constant_pool.len() as u16;
        subclass
            .constant_pool
            .push(Some(Constant::Class { name_index }));
        subclass.super_class = subclass.this_class;
        subclass.this_class = this_class;
        let field_name = subclass.constant_pool.len() as u16;
        subclass
            .constant_pool
            .push(Some(Constant::Utf8("received".into())));
        let descriptor = subclass.constant_pool.len() as u16;
        subclass
            .constant_pool
            .push(Some(Constant::Utf8("J".into())));
        let field_type = subclass.constant_pool.len() as u16;
        subclass.constant_pool.push(Some(Constant::NameAndType {
            name_index: field_name,
            descriptor_index: descriptor,
        }));
        let field_ref = subclass.constant_pool.len() as u16;
        subclass.constant_pool.push(Some(Constant::Fieldref {
            class_index: this_class,
            name_and_type_index: field_type,
        }));
        subclass.fields = vec![classfile::Member {
            access_flags: 2,
            name_index: field_name,
            descriptor_index: descriptor,
            attributes: Vec::new(),
        }];
        subclass.methods.retain_mut(|member| {
            let name = entry.class.utf8(member.name_index);
            let bytes = if name == Some("next") {
                let Some(bytes) = next_code else { return false };
                bytes.to_vec()
            } else if name == Some("setSeed") && override_seed {
                let [high, low] = field_ref.to_be_bytes();
                vec![0x2a, 0x1f, 0xb5, high, low, 0xb1]
            } else {
                return false;
            };
            let Attribute::Code(code) = &mut member.attributes[0] else {
                panic!("missing method body")
            };
            code.code = bytes;
            true
        });
        program.add_class(&subclass, &Limits::default()).unwrap();
    }
    program
}

fn call_random(
    machine: &mut Machine<'_, '_>,
    random: Handle,
    name: &str,
    descriptor: &str,
) -> Result<CallOutcome, EmuError> {
    let method = machine
        .program
        .methods
        .get(&MethodKey {
            class: "java/util/Random".into(),
            name: name.into(),
            descriptor: descriptor.into(),
        })
        .unwrap()
        .clone();
    machine.call(&method, [Value::Reference(Some(random))], 1)
}

fn next_random_int(machine: &mut Machine<'_, '_>, random: Handle) -> i32 {
    let CallOutcome::Return(Some(Value::Int(value))) =
        call_random(machine, random, "nextInt", "()I").unwrap()
    else {
        panic!("missing random integer")
    };
    value
}

#[test]
fn random_floating_values_dispatch_to_the_overridden_bit_generator() {
    let program = random_test_program(Some(&[0x1b, 0xac]), false);
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let random = machine
        .heap
        .managed
        .allocate_object(
            "test/CustomRandom",
            HashMap::from([("java/util/Random.seed:J".into(), HeapValue::Long(255))]),
        )
        .unwrap();
    let CallOutcome::Return(Some(Value::Float(value))) =
        call_random(&mut machine, random, "nextFloat", "()F").unwrap()
    else {
        panic!("missing float result")
    };
    assert_eq!(value, 24.0 / 16_777_216.0);
    let CallOutcome::Return(Some(Value::Double(value))) =
        call_random(&mut machine, random, "nextDouble", "()D").unwrap()
    else {
        panic!("missing double result")
    };
    assert_eq!(
        value,
        ((26_u64 << 27) + 27) as f64 / 9_007_199_254_740_992.0
    );
    assert_eq!(
        machine
            .heap
            .managed
            .field(random, "java/util/Random.seed:J")
            .unwrap(),
        HeapValue::Long(255)
    );
}

#[test]
fn random_float_methods_preserve_the_instruction_limit_in_overrides() {
    let program = random_test_program(Some(&[0xa7, 0, 0]), false);
    for (name, descriptor) in [("nextFloat", "()F"), ("nextDouble", "()D")] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(
            Limits {
                max_instructions: 64,
                ..Limits::default()
            },
            false,
            &mut context,
        );
        let random = machine
            .heap
            .managed
            .allocate_object(
                "test/CustomRandom",
                HashMap::from([("java/util/Random.seed:J".into(), HeapValue::Long(255))]),
            )
            .unwrap();
        let error = call_random(&mut machine, random, name, descriptor)
            .err()
            .expect("infinite override must exhaust its instruction budget");
        assert_eq!(error.code(), "instruction-limit");
    }
}

#[test]
fn random_default_constructor_dispatches_to_the_overridden_seed_setter() {
    let program = random_test_program(None, true);
    for random_seed in [None, Some(7)] {
        let mut context = PacingContext {
            now_millis: 1_234,
            random_seed,
            ..PacingContext::default()
        };
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let random = machine
            .heap
            .managed
            .allocate_object(
                "test/CustomRandom",
                HashMap::from([
                    ("java/util/Random.seed:J".into(), HeapValue::Long(0)),
                    ("test/CustomRandom.received:J".into(), HeapValue::Long(0)),
                ]),
            )
            .unwrap();
        assert!(matches!(
            call_random(&mut machine, random, "<init>", "()V").unwrap(),
            CallOutcome::Return(None)
        ));
        let HeapValue::Long(received) = machine
            .heap
            .managed
            .field(random, "test/CustomRandom.received:J")
            .unwrap()
        else {
            panic!("missing constructor seed")
        };
        if let Some(seed) = random_seed {
            assert_eq!(received, seed ^ JAVA_RANDOM_MULTIPLIER);
        } else {
            assert_ne!(received, 0);
        }
        assert_eq!(
            machine
                .heap
                .managed
                .field(random, "java/util/Random.seed:J")
                .unwrap(),
            HeapValue::Long(0)
        );
    }
}

#[test]
fn random_default_constructor_honors_deterministic_replay_seed() {
    let program = random_test_program(None, false);
    let mut context = PacingContext {
        now_millis: 1_234,
        random_seed: Some(7),
        ..PacingContext::default()
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let random = machine
        .heap
        .managed
        .allocate_object(
            "java/util/Random",
            HashMap::from([("java/util/Random.seed:J".to_owned(), HeapValue::Long(0))]),
        )
        .unwrap();
    assert!(matches!(
        call_random(&mut machine, random, "<init>", "()V").unwrap(),
        CallOutcome::Return(None)
    ));
    assert_eq!(
        machine
            .heap
            .managed
            .field(random, "java/util/Random.seed:J")
            .unwrap(),
        HeapValue::Long(7)
    );
}

#[test]
fn random_lcg_matches_java_seed_zero_vector() {
    let program = random_test_program(None, false);
    let mut context = PacingContext {
        now_millis: 0,
        ..PacingContext::default()
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let random = machine
        .heap
        .managed
        .allocate_object(
            "java/util/Random",
            HashMap::from([(
                "java/util/Random.seed:J".to_owned(),
                HeapValue::Long(JAVA_RANDOM_MULTIPLIER),
            )]),
        )
        .unwrap();

    assert_eq!(next_random_int(&mut machine, random), -1_155_484_576);
    assert_eq!(next_random_int(&mut machine, random), -723_955_400);
}
