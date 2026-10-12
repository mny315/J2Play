use super::m3g_support::object;
use super::*;

const CLASS: &str = "javax/microedition/m3g/Transform";
const VERTEX_TRANSFORM: &str = "(Ljavax/microedition/m3g/VertexArray;[FZ)V";

#[test]
fn m3g_orientation_readback_preserves_small_rotations() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (group, _) = object(
        &mut machine,
        "javax/microedition/m3g/Group",
        m3g::ObjectKind::Group {
            node: m3g::NodeState::default(),
            children: Vec::new(),
        },
    );
    let output = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Float, 5)
        .unwrap();
    for angle in [0.01_f32, -0.01, 1.0e-10, 1.0e-30] {
        machine.m3g_write_float_array(output, &[-77.0; 5]).unwrap();
        machine
            .invoke_m3g_scene_graph_native(
                "javax/microedition/m3g/Transformable",
                "setOrientation",
                "(FFFF)V",
                &[
                    Value::Reference(Some(group)),
                    Value::Float(angle),
                    Value::Float(0.0),
                    Value::Float(0.0),
                    Value::Float(1.0),
                ],
            )
            .unwrap();
        machine
            .invoke_m3g_scene_graph_native(
                "javax/microedition/m3g/Transformable",
                "getOrientation",
                "([F)V",
                &[
                    Value::Reference(Some(group)),
                    Value::Reference(Some(output)),
                ],
            )
            .unwrap();
        let actual = machine.m3g_float_array(output, 0).unwrap();
        assert!(
            (actual[0] / angle.abs() - 1.0).abs() < 1.0e-5,
            "angle={angle}, actual={actual:?}"
        );
        assert_eq!(&actual[1..], &[0.0, 0.0, angle.signum(), -77.0]);
    }
}

fn transform(machine: &mut Machine<'_, '_>) -> Handle {
    object(
        machine,
        CLASS,
        m3g::ObjectKind::Transform(
            m3g::Mat4::from_row_major([
                2.0, 0.0, 0.0, 7.0, 0.0, 3.0, 0.0, 11.0, 0.0, 0.0, -4.0, 13.0, 1.0, 2.0, 3.0, 5.0,
            ])
            .unwrap(),
        ),
    )
    .0
}

fn vertex_array(
    machine: &mut Machine<'_, '_>,
    components: usize,
    component_type: m3g::VertexComponent,
    values: &[i16],
) -> Handle {
    let count = values.len() / components;
    let mut array = m3g::VertexArrayState::new(count, components, component_type).unwrap();
    match component_type {
        m3g::VertexComponent::Byte => array
            .set_bytes(
                0,
                count,
                &values.iter().map(|value| *value as i8).collect::<Vec<_>>(),
            )
            .unwrap(),
        m3g::VertexComponent::Short => array.set_shorts(0, count, values).unwrap(),
    }
    object(
        machine,
        "javax/microedition/m3g/VertexArray",
        m3g::ObjectKind::VertexArray(array),
    )
    .0
}

#[test]
fn m3g_vertex_transform_returns_four_components_and_uses_the_requested_input_w() {
    // M3G 1.1 Transform.transform(VertexArray, float[], boolean): W belongs to
    // the input vector; output vectors always contain all four components.
    for component_type in [m3g::VertexComponent::Byte, m3g::VertexComponent::Short] {
        for components in [2, 3] {
            let program = Program::new();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let transform = transform(&mut machine);
            let limit = if component_type == m3g::VertexComponent::Byte {
                127
            } else {
                32767
            };
            let values = [-limit - 1, limit, -17, 42, -31, 63];
            let input = vertex_array(
                &mut machine,
                components,
                component_type,
                &values[..2 * components],
            );
            let output = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Float, 11)
                .unwrap();
            for w in [false, true] {
                machine.m3g_write_float_array(output, &[-77.0; 11]).unwrap();
                machine
                    .invoke_m3g_scene_graph_native(
                        CLASS,
                        "transform",
                        VERTEX_TRANSFORM,
                        &[
                            Value::Reference(Some(transform)),
                            Value::Reference(Some(input)),
                            Value::Reference(Some(output)),
                            Value::Int(i32::from(w)),
                        ],
                    )
                    .unwrap();
                let actual = machine.m3g_float_array(output, 0).unwrap();
                for (source, actual) in values[..2 * components]
                    .chunks_exact(components)
                    .zip(actual.chunks_exact(4))
                {
                    let x = f32::from(source[0]);
                    let y = f32::from(source[1]);
                    let z = f32::from(source.get(2).copied().unwrap_or(0));
                    let w = f32::from(w);
                    assert_eq!(
                        actual,
                        [
                            2.0 * x + 7.0 * w,
                            3.0 * y + 11.0 * w,
                            -4.0 * z + 13.0 * w,
                            x + 2.0 * y + 3.0 * z + 5.0 * w
                        ]
                    );
                }
                assert_eq!(&actual[8..], &[-77.0; 3]);
            }
        }
    }
}

#[test]
fn m3g_vertex_transform_validates_shapes_before_writing() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let transform = transform(&mut machine);
    for components in [2, 3, 4] {
        let input = vertex_array(
            &mut machine,
            components,
            m3g::VertexComponent::Short,
            &vec![1; components * 2],
        );
        for length in [0, 6, 7, 8] {
            if components != 4 && length == 8 {
                continue;
            }
            let output = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Float, length)
                .unwrap();
            machine
                .m3g_write_float_array(output, &vec![-77.0; length as usize])
                .unwrap();
            for w in [0, 1] {
                let error = machine
                    .invoke_m3g_scene_graph_native(
                        CLASS,
                        "transform",
                        VERTEX_TRANSFORM,
                        &[
                            Value::Reference(Some(transform)),
                            Value::Reference(Some(input)),
                            Value::Reference(Some(output)),
                            Value::Int(w),
                        ],
                    )
                    .err()
                    .expect("invalid vertex transfer must fail");
                assert_eq!(error.code(), "illegal-argument-exception");
                assert_eq!(
                    machine.m3g_float_array(output, 0).unwrap(),
                    vec![-77.0; length as usize]
                );
            }
        }
    }
}

#[test]
fn m3g_float_transform_updates_vectors_and_rejects_invalid_arrays_atomically() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let transform = transform(&mut machine);
    let values = [1.0, 2.0, 3.0, 1.0, 4.0, 5.0, 6.0, 0.0];
    for (length, malformed) in [(8, false), (7, false), (8, true), (0, false)] {
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Float, length)
            .unwrap();
        machine
            .m3g_write_float_array(array, &values[..length as usize])
            .unwrap();
        if malformed {
            machine.m3g_float_array_elements_mut(array, 0).unwrap()[7] = HeapValue::Int(42);
        }
        let original = machine.m3g_float_array_elements(array, 0).unwrap().to_vec();
        let result = machine.invoke_m3g_scene_graph_native(
            CLASS,
            "transform",
            "([F)V",
            &[
                Value::Reference(Some(transform)),
                Value::Reference(Some(array)),
            ],
        );
        if length == 7 || malformed {
            assert!(result.is_err());
            assert_eq!(
                machine.m3g_float_array_elements(array, 0).unwrap(),
                original
            );
        } else {
            result.unwrap();
            let expected = if length == 0 {
                &[][..]
            } else {
                &[9.0, 17.0, 1.0, 19.0, 8.0, 15.0, -24.0, 32.0][..]
            };
            assert_eq!(machine.m3g_float_array(array, 0).unwrap(), expected);
        }
    }
}

#[test]
#[ignore = "manual VM float transform throughput measurement"]
fn m3g_float_transform_throughput() {
    for count in [0, 1, 16, 1024, 16384] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        // Four applications restore the vectors, keeping every timed iteration finite.
        let transform = object(
            &mut machine,
            CLASS,
            m3g::ObjectKind::Transform(
                m3g::Mat4::from_row_major([
                    0.0, -1.0, 0.0, 0.5, 1.0, 0.0, 0.0, -0.25, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                    1.0,
                ])
                .unwrap(),
            ),
        )
        .0;
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Float, count * 4)
            .unwrap();
        let values: Vec<_> = (0..count * 4)
            .map(|index| ((index * 47) % 997) as f32)
            .collect();
        machine.m3g_write_float_array(array, &values).unwrap();
        let args = [
            Value::Reference(Some(transform)),
            Value::Reference(Some(array)),
        ];
        let started = std::time::Instant::now();
        for _ in 0..512 {
            machine
                .invoke_m3g_scene_graph_native(
                    CLASS,
                    "transform",
                    "([F)V",
                    std::hint::black_box(&args),
                )
                .unwrap();
        }
        let elapsed = started.elapsed();
        let actual = machine.m3g_float_array(array, 0).unwrap();
        assert_eq!(actual, values);
        let checksum = actual.iter().fold(0_u64, |sum, value| {
            sum.wrapping_mul(31)
                .wrapping_add(u64::from(value.to_bits()))
        });
        eprintln!("float vertices={count}: elapsed={elapsed:?} checksum={checksum:016x}");
    }
}

#[test]
#[ignore = "manual VM vertex transform throughput measurement"]
fn m3g_vertex_transform_throughput() {
    for count in [16, 1024, 16384] {
        for components in [2, 3] {
            let program = Program::new();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let transform = transform(&mut machine);
            let values: Vec<_> = (0..count * components)
                .map(|index| (index * 197) as i16)
                .collect();
            let input = vertex_array(
                &mut machine,
                components,
                m3g::VertexComponent::Short,
                &values,
            );
            let output = machine
                .heap
                .managed
                .allocate_array(ArrayKind::Float, (count * 4) as i32)
                .unwrap();
            let args = [
                Value::Reference(Some(transform)),
                Value::Reference(Some(input)),
                Value::Reference(Some(output)),
                Value::Int(1),
            ];
            let started = std::time::Instant::now();
            for _ in 0..128 {
                machine
                    .invoke_m3g_scene_graph_native(
                        CLASS,
                        "transform",
                        VERTEX_TRANSFORM,
                        std::hint::black_box(&args),
                    )
                    .unwrap();
            }
            let elapsed = started.elapsed();
            let checksum =
                machine
                    .m3g_float_array(output, 0)
                    .unwrap()
                    .iter()
                    .fold(0_u64, |sum, value| {
                        sum.wrapping_mul(31)
                            .wrapping_add(u64::from(value.to_bits()))
                    });
            eprintln!(
                "vertices={count} components={components}: elapsed={elapsed:?} checksum={checksum:016x}"
            );
        }
    }
}
