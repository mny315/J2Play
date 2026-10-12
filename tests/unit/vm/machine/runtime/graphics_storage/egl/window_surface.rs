use super::*;

fn bootstrap_call(
    machine: &mut Machine<'_, '_>,
    class: &str,
    name: &str,
    descriptor: &str,
    arguments: &[Value],
) -> CallOutcome {
    let method = machine.program.methods[&MethodKey {
        class: class.into(),
        name: name.into(),
        descriptor: descriptor.into(),
    }]
        .clone();
    machine.call(&method, arguments, 1).unwrap()
}

fn canvas(machine: &mut Machine<'_, '_>) -> Handle {
    let canvas = machine
        .allocate_native_instance("javax/microedition/lcdui/Canvas", &[])
        .unwrap();
    let outcome = bootstrap_call(
        machine,
        "javax/microedition/lcdui/Canvas",
        "<init>",
        "()V",
        &[Value::Reference(Some(canvas))],
    );
    if let CallOutcome::Throw(exception) = outcome {
        panic!(
            "Canvas constructor threw {}: {:?}",
            machine.object_class(exception).unwrap(),
            machine.throwable_diagnostic_message(exception)
        );
    }
    assert!(matches!(outcome, CallOutcome::Return(None)));
    canvas
}

#[test]
fn egl_windows_bind_canvas_graphics_and_follow_framebuffer_replacement() {
    let program = program_with_lcdui_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            lcd_width: 4,
            lcd_height: 4,
            lcd_fullscreen_available: true,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let base = egl_arguments(&mut machine);
    let canvas = canvas(&mut machine);
    let config = machine
        .allocate_native_instance("javax/microedition/khronos/egl/EGLConfig", &[])
        .unwrap();
    let context = machine
        .allocate_native_instance("javax/microedition/khronos/egl/EGLContext", &[])
        .unwrap();
    for field in ["paintGraphics", "gameGraphics"] {
        let graphics = machine
            .graphics_reference_field(
                canvas,
                &format!(
                    "javax/microedition/lcdui/Canvas.{field}:Ljavax/microedition/lcdui/Graphics;"
                ),
            )
            .unwrap();
        let mut arguments = base.clone();
        arguments.extend([
            Value::Reference(Some(config)),
            Value::Reference(Some(graphics)),
            Value::Reference(None),
        ]);
        let CallOutcome::Return(Some(Value::Reference(Some(surface)))) =
            egl_call(&mut machine, "eglCreateWindowSurface", &arguments).unwrap()
        else {
            panic!("window surface creation failed");
        };
        arguments.truncate(2);
        arguments.extend([
            Value::Reference(Some(surface)),
            Value::Reference(Some(surface)),
            Value::Reference(Some(context)),
        ]);
        assert!(matches!(
            egl_call(&mut machine, "eglMakeCurrent", &arguments).unwrap(),
            CallOutcome::Return(Some(Value::Int(1)))
        ));
        assert_eq!(machine.jsr239.target, Some(canvas));
        machine
            .canvas_set_full_screen_mode(&[Value::Reference(Some(canvas)), Value::Int(1)])
            .unwrap();
        let framebuffer = machine
            .graphics_reference_field(
                canvas,
                "javax/microedition/lcdui/Canvas.framebuffer:Ljavax/microedition/lcdui/Image;",
            )
            .unwrap();
        assert_eq!(
            machine
                .graphics_reference_field(
                    graphics,
                    "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;"
                )
                .unwrap(),
            framebuffer
        );
        assert_eq!(
            machine
                .canvas_dimension(&[Value::Reference(machine.jsr239.target)], true)
                .unwrap(),
            4
        );
    }
}

#[test]
fn egl_windows_reject_canvas_objects_and_offscreen_graphics() {
    let program = program_with_lcdui_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let mut arguments = egl_arguments(&mut machine);
    let canvas = canvas(&mut machine);
    let config = machine
        .allocate_native_instance("javax/microedition/khronos/egl/EGLConfig", &[])
        .unwrap();
    let image = machine.allocate_image(2, 2, vec![0; 4], true, &[]).unwrap();
    let CallOutcome::Return(Some(Value::Reference(Some(graphics)))) = bootstrap_call(
        &mut machine,
        "javax/microedition/lcdui/Image",
        "getGraphics",
        "()Ljavax/microedition/lcdui/Graphics;",
        &[Value::Reference(Some(image))],
    ) else {
        panic!("offscreen Graphics creation failed");
    };
    arguments.extend([
        Value::Reference(Some(config)),
        Value::Reference(None),
        Value::Reference(None),
    ]);
    for target in [None, Some(canvas), Some(graphics), Some(image)] {
        arguments[3] = Value::Reference(target);
        let outcome = egl_call(&mut machine, "eglCreateWindowSurface", &arguments).unwrap();
        assert!(
            matches!(outcome, CallOutcome::Throw(exception) if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException")
        );
    }
}

#[test]
fn canvas_graphics_keep_their_window_alive_without_leaking_a_reference_cycle() {
    let program = program_with_lcdui_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let canvas = canvas(&mut machine);
    let CallOutcome::Return(Some(Value::Reference(Some(graphics)))) = bootstrap_call(
        &mut machine,
        "javax/microedition/lcdui/Canvas",
        "__getGameGraphics",
        "()Ljavax/microedition/lcdui/Graphics;",
        &[Value::Reference(Some(canvas))],
    ) else {
        panic!("GameCanvas Graphics acquisition failed");
    };
    machine.collect_heap(vec![graphics]);
    assert!(machine.heap.managed.get(canvas).is_ok());
    machine.collect_heap(Vec::new());
    assert!(machine.heap.managed.get(canvas).is_err());
    assert!(machine.heap.managed.get(graphics).is_err());
}
