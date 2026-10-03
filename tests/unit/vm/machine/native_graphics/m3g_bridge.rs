use super::*;

#[test]
fn m3g_bridge_executes_transform_and_graph_contracts() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let transform = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Transform", HashMap::new())
        .unwrap();
    let constructor = runtime_method(
        "javax/microedition/m3g/Transform",
        "<init>",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(&constructor, &[Value::Reference(Some(transform))], 1)
        .unwrap();
    let translate = runtime_method(
        "javax/microedition/m3g/Transform",
        "postTranslate",
        "(FFF)V",
        &[0xb1],
        0,
        4,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &translate,
            &[
                Value::Reference(Some(transform)),
                Value::Float(2.0),
                Value::Float(3.0),
                Value::Float(4.0),
            ],
            1,
        )
        .unwrap();
    let points = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 4)
        .unwrap();
    for (index, value) in [1.0, 1.0, 1.0, 1.0].into_iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(points, index as i32, HeapValue::Float(value))
            .unwrap();
    }
    let transform_array = runtime_method(
        "javax/microedition/m3g/Transform",
        "transform",
        "([F)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &transform_array,
            &[
                Value::Reference(Some(transform)),
                Value::Reference(Some(points)),
            ],
            1,
        )
        .unwrap();
    assert_eq!(
        (0..4)
            .map(|index| machine.heap.managed.array_get(points, index).unwrap())
            .collect::<Vec<_>>(),
        vec![
            HeapValue::Float(3.0),
            HeapValue::Float(4.0),
            HeapValue::Float(5.0),
            HeapValue::Float(1.0),
        ]
    );

    // The JSR-184 public float[16] contract is row-major even though the
    // renderer stores matrices column-major internally.
    let matrix = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 16)
        .unwrap();
    let row_major = [
        1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 6.0, 0.0, 0.0, 1.0, 7.0, 0.0, 0.0, 0.0, 1.0,
    ];
    for (index, value) in row_major.into_iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(matrix, index as i32, HeapValue::Float(value))
            .unwrap();
    }
    let set_matrix = runtime_method(
        "javax/microedition/m3g/Transform",
        "set",
        "([F)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &set_matrix,
            &[
                Value::Reference(Some(transform)),
                Value::Reference(Some(matrix)),
            ],
            1,
        )
        .unwrap();
    for (index, value) in [1.0, 1.0, 1.0, 1.0].into_iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(points, index as i32, HeapValue::Float(value))
            .unwrap();
    }
    machine
        .invoke_m3g_native(
            &transform_array,
            &[
                Value::Reference(Some(transform)),
                Value::Reference(Some(points)),
            ],
            1,
        )
        .unwrap();
    assert_eq!(
        (0..4)
            .map(|index| machine.heap.managed.array_get(points, index).unwrap())
            .collect::<Vec<_>>(),
        vec![
            HeapValue::Float(6.0),
            HeapValue::Float(7.0),
            HeapValue::Float(8.0),
            HeapValue::Float(1.0),
        ]
    );
    let get_matrix = runtime_method(
        "javax/microedition/m3g/Transform",
        "get",
        "([F)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    for index in 0..16 {
        machine
            .heap
            .managed
            .array_set(matrix, index, HeapValue::Float(f32::NAN))
            .unwrap();
    }
    machine
        .invoke_m3g_native(
            &get_matrix,
            &[
                Value::Reference(Some(transform)),
                Value::Reference(Some(matrix)),
            ],
            1,
        )
        .unwrap();
    assert_eq!(
        (0..16)
            .map(|index| machine.heap.managed.array_get(matrix, index).unwrap())
            .collect::<Vec<_>>(),
        row_major.map(HeapValue::Float)
    );

    let group = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Group", HashMap::new())
        .unwrap();
    let camera = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Camera", HashMap::new())
        .unwrap();
    let light = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Light", HashMap::new())
        .unwrap();
    for (class, object) in [
        ("javax/microedition/m3g/Group", group),
        ("javax/microedition/m3g/Camera", camera),
        ("javax/microedition/m3g/Light", light),
    ] {
        let constructor = runtime_method(class, "<init>", "()V", &[0xb1], 0, 1, Vec::new(), false);
        machine
            .invoke_m3g_native(&constructor, &[Value::Reference(Some(object))], 1)
            .unwrap();
    }
    let set_orientation = runtime_method(
        "javax/microedition/m3g/Transformable",
        "setOrientation",
        "(FFFF)V",
        &[0xb1],
        0,
        5,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &set_orientation,
            &[
                Value::Reference(Some(camera)),
                Value::Float(137.0),
                Value::Float(1.0),
                Value::Float(2.0),
                Value::Float(3.0),
            ],
            1,
        )
        .unwrap();
    let orientation = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 4)
        .unwrap();
    let get_orientation = runtime_method(
        "javax/microedition/m3g/Transformable",
        "getOrientation",
        "([F)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &get_orientation,
            &[
                Value::Reference(Some(camera)),
                Value::Reference(Some(orientation)),
            ],
            1,
        )
        .unwrap();
    let expected_axis = [1.0_f32, 2.0, 3.0].map(|value| value / 14.0_f32.sqrt());
    for (index, expected) in [137.0, expected_axis[0], expected_axis[1], expected_axis[2]]
        .into_iter()
        .enumerate()
    {
        let HeapValue::Float(actual) = machine
            .heap
            .managed
            .array_get(orientation, index as i32)
            .unwrap()
        else {
            panic!("orientation component is not a float");
        };
        assert!((actual - expected).abs() < 1.0e-4, "{actual} != {expected}");
    }
    let set_intensity = runtime_method(
        "javax/microedition/m3g/Light",
        "setIntensity",
        "(F)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &set_intensity,
            &[Value::Reference(Some(light)), Value::Float(-0.75)],
            1,
        )
        .unwrap();
    let m3g::ObjectKind::Light { light, .. } = machine
        .m3g
        .runtime
        .kind(machine.m3g_handle(light).unwrap())
        .unwrap()
    else {
        panic!("Light guest object has no native light state");
    };
    assert_eq!(light.intensity, -0.75);
    let add_child = runtime_method(
        "javax/microedition/m3g/Group",
        "addChild",
        "(Ljavax/microedition/m3g/Node;)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &add_child,
            &[
                Value::Reference(Some(group)),
                Value::Reference(Some(camera)),
            ],
            1,
        )
        .unwrap();
    let count = runtime_method(
        "javax/microedition/m3g/Group",
        "getChildCount",
        "()I",
        &[0xac],
        1,
        1,
        Vec::new(),
        false,
    );
    assert!(matches!(
        machine
            .invoke_m3g_native(&count, &[Value::Reference(Some(group))], 1)
            .unwrap(),
        CallOutcome::Return(Some(Value::Int(1)))
    ));
    let remove_child = runtime_method(
        "javax/microedition/m3g/Group",
        "removeChild",
        "(Ljavax/microedition/m3g/Node;)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &remove_child,
            &[Value::Reference(Some(group)), Value::Reference(None)],
            1,
        )
        .unwrap();
    let remove_track = runtime_method(
        "javax/microedition/m3g/Object3D",
        "removeAnimationTrack",
        "(Ljavax/microedition/m3g/AnimationTrack;)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    machine
        .invoke_m3g_native(
            &remove_track,
            &[Value::Reference(Some(group)), Value::Reference(None)],
            1,
        )
        .unwrap();
    assert!(matches!(
        machine
            .invoke_m3g_native(&count, &[Value::Reference(Some(group))], 1)
            .unwrap(),
        CallOutcome::Return(Some(Value::Int(1)))
    ));
}

#[test]
fn m3g_texture_compatibility_limit_is_separate_from_advertised_device_limit() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        m3g_max_texture_dimension: 256,
        m3g_compatibility_max_texture_dimension: 512,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    let image = machine
        .m3g
        .runtime
        .create(
            None,
            m3g::ObjectKind::Image2D(
                m3g::Image2DState::mutable(m3g::ImageFormat::Rgb, 512, 512).unwrap(),
            ),
        )
        .unwrap();

    machine.m3g_validate_texture_image(image).unwrap();
    assert_eq!(machine.limits.m3g_max_texture_dimension, 256);

    machine.limits.m3g_compatibility_max_texture_dimension = 256;
    let error = machine.m3g_validate_texture_image(image).unwrap_err();
    assert_eq!(error.code(), "illegal-argument-exception");
    assert!(error.message().contains("512x512"));
    assert!(error.message().contains("device property 256"));
}

#[test]
fn m3g_properties_table_follows_the_active_profile_limits() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        m3g_support_antialiasing: true,
        m3g_support_true_color: false,
        m3g_support_dithering: true,
        m3g_support_mipmapping: false,
        m3g_support_perspective_correction: false,
        m3g_support_local_camera_lighting: true,
        m3g_max_lights: 4,
        m3g_max_viewport_width: 511,
        m3g_max_viewport_height: 383,
        m3g_max_viewport_dimension: 512,
        m3g_max_texture_dimension: 127,
        m3g_max_sprite_crop_dimension: 255,
        m3g_max_transforms_per_vertex: 2,
        m3g_num_texture_units: 1,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    let method = runtime_method(
        "javax/microedition/m3g/Graphics3D",
        "getProperties",
        "()Ljava/util/Hashtable;",
        &[0xb0],
        1,
        0,
        Vec::new(),
        true,
    );
    let CallOutcome::Return(Some(Value::Reference(Some(table)))) =
        machine.invoke_m3g_native(&method, &[], 1).unwrap()
    else {
        panic!("Graphics3D.getProperties must return a Hashtable");
    };
    let keys = machine
        .graphics_reference_field(table, "java/util/Hashtable.keys:[Ljava/lang/Object;")
        .unwrap();
    let values = machine
        .graphics_reference_field(table, "java/util/Hashtable.values:[Ljava/lang/Object;")
        .unwrap();
    let count = machine
        .graphics_int_field(table, "java/util/Hashtable.count:I")
        .unwrap();
    let mut actual = HashMap::new();
    for index in 0..count {
        let HeapValue::Reference(Some(key)) = machine.heap.managed.array_get(keys, index).unwrap()
        else {
            panic!("M3G property key must be a String");
        };
        let name = String::from_utf16_lossy(machine.heap.string_values.get(&key).unwrap());
        let HeapValue::Reference(Some(value)) =
            machine.heap.managed.array_get(values, index).unwrap()
        else {
            panic!("M3G property value must be boxed");
        };
        let class = machine.object_class(value).unwrap().into_owned();
        let primitive = match class.as_str() {
            "java/lang/Boolean" => machine
                .graphics_int_field(value, "java/lang/Boolean.value:Z")
                .unwrap(),
            "java/lang/Integer" => machine
                .graphics_int_field(value, "java/lang/Integer.value:I")
                .unwrap(),
            _ => panic!("unexpected M3G property value class {class}"),
        };
        actual.insert(name, (class, primitive));
    }

    let boolean = |value| ("java/lang/Boolean".to_owned(), i32::from(value));
    let integer = |value| ("java/lang/Integer".to_owned(), value);
    assert_eq!(actual.len(), 14);
    assert_eq!(actual["supportAntialiasing"], boolean(true));
    assert_eq!(actual["supportTrueColor"], boolean(false));
    assert_eq!(actual["supportDithering"], boolean(true));
    assert_eq!(actual["supportMipmapping"], boolean(false));
    assert_eq!(actual["supportPerspectiveCorrection"], boolean(false));
    assert_eq!(actual["supportLocalCameraLighting"], boolean(true));
    assert_eq!(actual["maxLights"], integer(4));
    assert_eq!(actual["maxViewportWidth"], integer(511));
    assert_eq!(actual["maxViewportHeight"], integer(383));
    assert_eq!(actual["maxViewportDimension"], integer(512));
    assert_eq!(actual["maxTextureDimension"], integer(127));
    assert_eq!(actual["maxSpriteCropDimension"], integer(255));
    assert_eq!(actual["maxTransformsPerVertex"], integer(2));
    assert_eq!(actual["numTextureUnits"], integer(1));
}

#[test]
fn m3g_texture_units_follow_the_active_profile() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        m3g_num_texture_units: 1,
        ..Limits::default()
    };
    let machine = program.machine(limits, false, &mut context);
    assert_eq!(machine.m3g_texture_unit(0).unwrap(), 0);
    assert_eq!(
        machine.m3g_texture_unit(1).unwrap_err().code(),
        "index-out-of-bounds-exception"
    );
}
