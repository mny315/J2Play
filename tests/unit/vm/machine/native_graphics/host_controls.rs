use super::*;

#[derive(Default)]
pub(crate) struct RecordingVibrationContext {
    accepted: bool,
    requests: Vec<VibrationRequest>,
}

impl HostServices for RecordingVibrationContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn request_vibration(&mut self, request: VibrationRequest) -> bool {
        self.requests.push(request);
        self.accepted
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

#[derive(Default)]
pub(crate) struct RecordingFramePacingContext {
    frame_requests: u32,
}

impl HostServices for RecordingFramePacingContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn pace_lcdui_frame_request(&mut self) -> Result<(), EmuError> {
        self.frame_requests += 1;
        Ok(())
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

#[test]
pub(crate) fn canvas_repaint_paces_only_new_pending_work_on_the_applied_current_canvas() {
    let program = Program::new();
    let mut context = RecordingFramePacingContext::default();
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let framebuffer = machine
            .heap
            .managed
            .allocate_object(
                "javax/microedition/lcdui/Image",
                HashMap::from([
                    (
                        "javax/microedition/lcdui/Image.width:I".into(),
                        HeapValue::Int(8),
                    ),
                    (
                        "javax/microedition/lcdui/Image.height:I".into(),
                        HeapValue::Int(6),
                    ),
                ]),
            )
            .unwrap();
        let canvas = machine
            .heap.managed
            .allocate_object(
                "javax/microedition/lcdui/Canvas",
                HashMap::from([
                    (
                        "javax/microedition/lcdui/Canvas.framebuffer:Ljavax/microedition/lcdui/Image;"
                            .into(),
                        HeapValue::Reference(Some(framebuffer)),
                    ),
                    (
                        "javax/microedition/lcdui/Canvas.pending:Z".into(),
                        HeapValue::Int(0),
                    ),
                    (
                        "javax/microedition/lcdui/Canvas.damageX:I".into(),
                        HeapValue::Int(0),
                    ),
                    (
                        "javax/microedition/lcdui/Canvas.damageY:I".into(),
                        HeapValue::Int(0),
                    ),
                    (
                        "javax/microedition/lcdui/Canvas.damageW:I".into(),
                        HeapValue::Int(0),
                    ),
                    (
                        "javax/microedition/lcdui/Canvas.damageH:I".into(),
                        HeapValue::Int(0),
                    ),
                ]),
            )
            .unwrap();
        let display = machine
            .heap.managed
            .allocate_object(
                "javax/microedition/lcdui/Display",
                HashMap::from([
                    (
                        "javax/microedition/lcdui/Display.current:Ljavax/microedition/lcdui/Displayable;"
                            .into(),
                        HeapValue::Reference(Some(canvas)),
                    ),
                    (
                        "javax/microedition/lcdui/Display.appliedCurrent:Ljavax/microedition/lcdui/Displayable;"
                            .into(),
                        HeapValue::Reference(None),
                    ),
                ]),
            )
            .unwrap();
        machine.classes.static_fields.insert(
            "javax/microedition/lcdui/Display.INSTANCE:Ljavax/microedition/lcdui/Display;".into(),
            Value::Reference(Some(display)),
        );

        // setCurrent() publishes the requested target before its deferred
        // display turn applies it. Repaint remains pending but must not
        // consume a real-time frame interval while the Canvas is hidden.
        machine
            .canvas_repaint(&[Value::Reference(Some(canvas))])
            .unwrap();
        assert_eq!(
            machine
                .graphics_int_field(canvas, "javax/microedition/lcdui/Canvas.pending:Z")
                .unwrap(),
            1
        );
        machine
            .heap.managed
            .set_field(
                display,
                "javax/microedition/lcdui/Display.appliedCurrent:Ljavax/microedition/lcdui/Displayable;",
                HeapValue::Reference(Some(canvas)),
            )
            .unwrap();

        // Model the display turn consuming the hidden request. The next
        // visible request creates work and receives one pacing credit.
        machine
            .heap
            .managed
            .set_field(
                canvas,
                "javax/microedition/lcdui/Canvas.pending:Z",
                HeapValue::Int(0),
            )
            .unwrap();

        machine
            .canvas_repaint(&[Value::Reference(Some(canvas))])
            .unwrap();
        machine
            .canvas_repaint(&[
                Value::Reference(Some(canvas)),
                Value::Int(1),
                Value::Int(1),
                Value::Int(2),
                Value::Int(2),
            ])
            .unwrap();
    }

    assert_eq!(context.frame_requests, 1);
}

#[test]
pub(crate) fn nokia_device_control_retains_valid_light_levels() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let method = runtime_method(
        "com/nokia/mid/ui/DeviceControl",
        "setLights",
        "(II)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        true,
    );

    let outcome = machine
        .invoke_vm_native(&method, &[Value::Int(1), Value::Int(37)], 1)
        .unwrap();

    assert!(matches!(outcome, Some(CallOutcome::Return(None))));
    assert_eq!(machine.device.light_levels, [100, 37]);
}

#[test]
pub(crate) fn siemens_light_controls_the_display_backlight_state() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);

    for (name, expected_level) in [("setLightOff", 0), ("setLightOn", 100)] {
        let method = runtime_method(
            "com/siemens/mp/game/Light",
            name,
            "()V",
            &[0xb1],
            0,
            0,
            Vec::new(),
            true,
        );
        let outcome = machine.invoke_vm_native(&method, &[], 1).unwrap();

        assert!(matches!(outcome, Some(CallOutcome::Return(None))));
        assert_eq!(machine.device.light_levels, [expected_level, 100]);
    }
}

#[test]
pub(crate) fn vibration_apis_share_the_frontend_neutral_host_boundary() {
    let program = Program::new();
    let mut context = RecordingVibrationContext {
        accepted: true,
        ..RecordingVibrationContext::default()
    };
    {
        let mut machine = program.machine(Limits::default(), false, &mut context);
        for (name, descriptor, args, expected) in [
            (
                "startVibrator",
                "()V",
                Vec::new(),
                VibrationRequest::Continuous { level: None },
            ),
            (
                "triggerVibrator",
                "(I)V",
                vec![Value::Int(375)],
                VibrationRequest::Timed {
                    duration_millis: 375,
                    level: None,
                },
            ),
            ("stopVibrator", "()V", Vec::new(), VibrationRequest::Stop),
        ] {
            let method = runtime_method(
                "com/siemens/mp/game/Vibrator",
                name,
                descriptor,
                &[0xb1],
                0,
                args.len(),
                Vec::new(),
                true,
            );
            assert!(matches!(
                machine.invoke_vm_native(&method, &args, 1).unwrap(),
                Some(CallOutcome::Return(None))
            ));
            assert_eq!(machine.device.last_vibration_request, expected);
        }

        let display_vibrate = runtime_method(
            "javax/microedition/lcdui/Display",
            "vibrate",
            "(I)Z",
            &[0xac],
            1,
            2,
            Vec::new(),
            true,
        );
        assert!(matches!(
            machine
                .invoke_vm_native(
                    &display_vibrate,
                    &[Value::Reference(None), Value::Int(250)],
                    1,
                )
                .unwrap(),
            Some(CallOutcome::Return(Some(Value::Int(1))))
        ));
        assert_eq!(
            machine.device.last_vibration_request,
            VibrationRequest::Timed {
                duration_millis: 250,
                level: None,
            }
        );

        let nokia_start = runtime_method(
            "com/nokia/mid/ui/DeviceControl",
            "startVibra",
            "(IJ)V",
            &[0xb1],
            0,
            3,
            Vec::new(),
            true,
        );
        assert!(matches!(
            machine
                .invoke_vm_native(&nokia_start, &[Value::Int(42), Value::Long(900)], 1)
                .unwrap(),
            Some(CallOutcome::Return(None))
        ));
        assert_eq!(
            machine.device.last_vibration_request,
            VibrationRequest::Timed {
                duration_millis: 900,
                level: Some(42),
            }
        );

        let nokia_stop = runtime_method(
            "com/nokia/mid/ui/DeviceControl",
            "stopVibra",
            "()V",
            &[0xb1],
            0,
            0,
            Vec::new(),
            true,
        );
        assert!(matches!(
            machine.invoke_vm_native(&nokia_stop, &[], 1).unwrap(),
            Some(CallOutcome::Return(None))
        ));
        assert_eq!(
            machine.device.last_vibration_request,
            VibrationRequest::Stop
        );
    }

    assert_eq!(
        context.requests,
        vec![
            VibrationRequest::Continuous { level: None },
            VibrationRequest::Timed {
                duration_millis: 375,
                level: None,
            },
            VibrationRequest::Stop,
            VibrationRequest::Timed {
                duration_millis: 250,
                level: None,
            },
            VibrationRequest::Timed {
                duration_millis: 900,
                level: Some(42),
            },
            VibrationRequest::Stop,
        ]
    );
}

#[test]
pub(crate) fn display_vibrate_reports_unavailable_host_and_rejects_negative_duration() {
    let program = program_with_exception("java/lang/IllegalArgumentException");
    let mut context = RecordingVibrationContext::default();
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let method = runtime_method(
        "javax/microedition/lcdui/Display",
        "vibrate",
        "(I)Z",
        &[0xac],
        1,
        2,
        Vec::new(),
        true,
    );

    assert!(matches!(
        machine
            .invoke_vm_native(&method, &[Value::Reference(None), Value::Int(100)], 1,)
            .unwrap(),
        Some(CallOutcome::Return(Some(Value::Int(0))))
    ));
    let Some(CallOutcome::Throw(exception)) = machine
        .invoke_vm_native(&method, &[Value::Reference(None), Value::Int(-1)], 1)
        .unwrap()
    else {
        panic!("negative Display.vibrate duration must throw");
    };
    assert_eq!(
        machine.object_class(exception).unwrap(),
        "java/lang/IllegalArgumentException"
    );
}
