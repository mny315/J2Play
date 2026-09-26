use super::*;

#[test]
fn timer_task_cancel_reports_whether_it_prevents_a_future_execution() {
    let mut program = Program::new();
    for entry in runtime_bootstrap::production_bootstrap_inventory() {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    let mut class = test_class_definition(Some("java/util/TimerTask"));
    let result_field = "CancellingTask.result:Z";
    class.fields.push(Field {
        key: result_field.into(),
        declaring_class: "CancellingTask".into(),
        kind: ValueKind::Int,
        is_static: true,
        field_token: FieldToken::new(),
        instance_slot: None,
        initial: Value::Int(-1),
        constant_string: None,
    });
    program.classes.insert("CancellingTask".into(), class);
    let callback = runtime_method(
        "CancellingTask",
        "run",
        "()V",
        &[0x2a, 0xb6, 0, 1, 0xb3, 0, 7, 0xb1],
        1,
        1,
        vec![
            None,
            Some(Constant::Methodref {
                class_index: 2,
                name_and_type_index: 3,
            }),
            Some(Constant::Class { name_index: 4 }),
            Some(Constant::NameAndType {
                name_index: 5,
                descriptor_index: 6,
            }),
            Some(Constant::Utf8("java/util/TimerTask".into())),
            Some(Constant::Utf8("cancel".into())),
            Some(Constant::Utf8("()Z".into())),
            Some(Constant::Fieldref {
                class_index: 8,
                name_and_type_index: 9,
            }),
            Some(Constant::Class { name_index: 10 }),
            Some(Constant::NameAndType {
                name_index: 11,
                descriptor_index: 12,
            }),
            Some(Constant::Utf8("CancellingTask".into())),
            Some(Constant::Utf8("result".into())),
            Some(Constant::Utf8("Z".into())),
        ],
        false,
    );
    program.methods.insert(callback.key.clone(), callback);
    let cancel = &program.methods[&MethodKey {
        class: "java/util/TimerTask".into(),
        name: "cancel".into(),
        descriptor: "()Z".into(),
    }];
    for (period, expected) in [(-1, 0), (0, 0), (40, 1)] {
        let mut host = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut host);
        machine.initialize_class("CancellingTask", 1).unwrap();
        let timer = machine
            .allocate_native_instance("java/util/Timer", &[])
            .unwrap();
        let task = machine
            .allocate_native_instance("CancellingTask", &[Value::Reference(Some(timer))])
            .unwrap();
        machine
            .heap
            .managed
            .set_field(task, "java/util/TimerTask.scheduled:Z", HeapValue::Int(1))
            .unwrap();
        machine.scheduler.scheduled_tasks.push(ScheduledJavaTask {
            timer,
            task,
            deadline: 0,
            period,
            fixed_rate: false,
        });

        machine.run_due_timer_tasks(1, &[], &[]).unwrap();

        assert_eq!(
            machine.classes.static_fields.get(result_field),
            Some(&Value::Int(expected)),
            "period={period}"
        );
        assert!(matches!(
            machine
                .call(cancel, [Value::Reference(Some(task))], 1)
                .unwrap(),
            CallOutcome::Return(Some(Value::Int(0)))
        ));
        machine
            .classes
            .static_fields
            .insert(result_field.into(), Value::Int(-1));
        machine.scheduler.virtual_wall_millis = 100;
        machine.run_due_timer_tasks(1, &[], &[]).unwrap();
        assert_eq!(
            machine.classes.static_fields.get(result_field),
            Some(&Value::Int(-1))
        );
        assert!(machine.scheduler.scheduled_tasks.is_empty());
    }
}
