use super::m3g_support::object;
use super::*;

fn projections() -> [(i32, m3g::CameraProjection); 5] {
    [
        (48, m3g::CameraProjection::default()),
        (
            49,
            m3g::CameraProjection::Parallel {
                height: 4.0,
                aspect_ratio: 2.0,
                near: 3.0,
                far: -2.0,
            },
        ),
        (
            50,
            m3g::CameraProjection::Perspective {
                field_of_view: 60.0,
                aspect_ratio: 2.0,
                near: 10.0,
                far: 1.0,
            },
        ),
        (
            49,
            m3g::CameraProjection::Parallel {
                height: 4.0,
                aspect_ratio: 2.0,
                near: 0.0,
                far: 0.0,
            },
        ),
        (
            50,
            m3g::CameraProjection::Perspective {
                field_of_view: 60.0,
                aspect_ratio: 2.0,
                near: 1.0,
                far: 1.0,
            },
        ),
    ]
}

fn camera(machine: &mut Machine<'_, '_>, projection: m3g::CameraProjection) -> Handle {
    object(
        machine,
        "javax/microedition/m3g/Camera",
        m3g::ObjectKind::Camera {
            node: m3g::NodeState::default(),
            projection,
        },
    )
    .0
}

fn call(
    machine: &mut Machine<'_, '_>,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> Result<CallOutcome, EmuError> {
    let method = runtime_method(
        "javax/microedition/m3g/Camera",
        name,
        descriptor,
        &[0xb1],
        1,
        args.len(),
        Vec::new(),
        false,
    );
    machine.invoke_m3g_native(&method, args, 1)
}

fn get_projection(
    machine: &mut Machine<'_, '_>,
    camera: Handle,
    output: Option<Handle>,
    array: bool,
) -> Result<CallOutcome, EmuError> {
    call(
        machine,
        "getProjection",
        if array {
            "([F)I"
        } else {
            "(Ljavax/microedition/m3g/Transform;)I"
        },
        &[Value::Reference(Some(camera)), Value::Reference(output)],
    )
}

#[test]
fn camera_projection_queries_accept_null_outputs_including_empty_clip_volumes() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for (kind, projection) in projections() {
        let camera = camera(&mut machine, projection);
        for array in [false, true] {
            assert!(
                matches!(get_projection(&mut machine, camera, None, array).unwrap(), CallOutcome::Return(Some(Value::Int(value))) if value == kind)
            );
        }
    }
}

#[test]
fn camera_parameter_queries_preserve_generic_arrays_and_parametric_suffixes() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    for (kind, projection) in projections() {
        let camera = camera(&mut machine, projection);
        for length in 0..=6 {
            let array = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Float, length)
                .unwrap();
            for index in 0..length {
                machine
                    .heap
                    .managed
                    .array_set(array, index, HeapValue::Float(77.0))
                    .unwrap();
            }
            let result = get_projection(&mut machine, camera, Some(array), true);
            let mut expected = vec![HeapValue::Float(77.0); length as usize];
            if length < 4 {
                assert!(
                    matches!(result.unwrap(), CallOutcome::Throw(exception) if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException")
                );
            } else {
                assert!(
                    matches!(result.unwrap(), CallOutcome::Return(Some(Value::Int(value))) if value == kind)
                );
                let values = match projection {
                    m3g::CameraProjection::Generic(_) => None,
                    m3g::CameraProjection::Parallel {
                        height,
                        aspect_ratio,
                        near,
                        far,
                    } => Some([height, aspect_ratio, near, far]),
                    m3g::CameraProjection::Perspective {
                        field_of_view,
                        aspect_ratio,
                        near,
                        far,
                    } => Some([field_of_view, aspect_ratio, near, far]),
                };
                if let Some(values) = values {
                    expected[..4].copy_from_slice(&values.map(HeapValue::Float));
                }
            }
            let actual = (0..length)
                .map(|index| machine.heap.managed.array_get(array, index).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn empty_camera_volume_rejects_matrix_query_without_changing_destination() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let initial = m3g::Mat4::translation(2.0, 3.0, 4.0).unwrap();
    let (output, native) = object(
        &mut machine,
        "javax/microedition/m3g/Transform",
        m3g::ObjectKind::Transform(initial),
    );
    for (_, projection) in projections().into_iter().skip(3) {
        let camera = camera(&mut machine, projection);
        let result = get_projection(&mut machine, camera, Some(output), false).unwrap();
        assert!(
            matches!(result, CallOutcome::Throw(exception) if machine.object_class(exception).unwrap() == "java/lang/ArithmeticException")
        );
        assert_eq!(
            machine.m3g.runtime.transform_value(native).unwrap(),
            initial
        );
    }
}

#[test]
fn camera_setters_preserve_state_after_invalid_parameters() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let camera = camera(&mut machine, m3g::CameraProjection::default());
    let native = machine.m3g_handle(camera).unwrap();
    for (name, values) in [
        ("setParallel", [4.0, 2.0, -2.0, -3.0]),
        ("setParallel", [4.0, 2.0, 0.0, 0.0]),
        ("setPerspective", [60.0, 2.0, 10.0, 1.0]),
        ("setPerspective", [60.0, 2.0, 1.0, 1.0]),
    ] {
        let mut args = vec![Value::Reference(Some(camera))];
        args.extend(values.map(Value::Float));
        assert!(matches!(
            call(&mut machine, name, "(FFFF)V", &args).unwrap(),
            CallOutcome::Return(None)
        ));
        let m3g::ObjectKind::Camera {
            projection: saved, ..
        } = machine.m3g.runtime.kind(native).unwrap()
        else {
            unreachable!()
        };
        let saved = *saved;
        for (index, invalid) in [(0, 0.0), (0, f32::NAN), (1, -1.0), (2, f32::INFINITY)] {
            let mut invalid_args = args.clone();
            invalid_args[index + 1] = Value::Float(invalid);
            let result = call(&mut machine, name, "(FFFF)V", &invalid_args).unwrap();
            assert!(
                matches!(result, CallOutcome::Throw(exception) if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException")
            );
            let m3g::ObjectKind::Camera { projection, .. } =
                machine.m3g.runtime.kind(native).unwrap()
            else {
                unreachable!()
            };
            assert_eq!(*projection, saved);
        }
    }
}

#[test]
fn camera_generic_matrix_is_copied_on_both_set_and_get() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let camera = camera(&mut machine, m3g::CameraProjection::default());
    let matrix = m3g::Mat4::translation(2.0, 3.0, 4.0).unwrap();
    let (source, source_native) = object(
        &mut machine,
        "javax/microedition/m3g/Transform",
        m3g::ObjectKind::Transform(matrix),
    );
    call(
        &mut machine,
        "setGeneric",
        "(Ljavax/microedition/m3g/Transform;)V",
        &[
            Value::Reference(Some(camera)),
            Value::Reference(Some(source)),
        ],
    )
    .unwrap();
    machine
        .m3g
        .runtime
        .set_transform_value(source_native, m3g::Mat4::IDENTITY)
        .unwrap();
    let (output, output_native) = object(
        &mut machine,
        "javax/microedition/m3g/Transform",
        m3g::ObjectKind::Transform(m3g::Mat4::IDENTITY),
    );
    for _ in 0..2 {
        assert!(matches!(
            get_projection(&mut machine, camera, Some(output), false).unwrap(),
            CallOutcome::Return(Some(Value::Int(48)))
        ));
        assert_eq!(
            machine.m3g.runtime.transform_value(output_native).unwrap(),
            matrix
        );
        machine
            .m3g
            .runtime
            .set_transform_value(output_native, m3g::Mat4::IDENTITY)
            .unwrap();
    }
}
