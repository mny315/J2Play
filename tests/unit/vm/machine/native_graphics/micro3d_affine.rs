use super::*;

const CLASS: &str = "com/mascotcapsule/micro3d/v3/AffineTrans";

pub(super) fn affine_object(machine: &mut Machine<'_, '_>) -> Handle {
    let fields = (0..3)
        .flat_map(|row| {
            (0..4).map(move |column| (format!("{CLASS}.m{row}{column}:I"), HeapValue::Int(0)))
        })
        .collect();
    machine.heap.managed.allocate_object(CLASS, fields).unwrap()
}

fn affine_method(name: &str, descriptor: &str) -> Method {
    runtime_method(CLASS, name, descriptor, &[0xb1], 0, 13, Vec::new(), false)
}

#[test]
fn math_fields_keep_base_class_ownership_and_observe_direct_guest_updates() {
    let mut program = program_with_exception("ChildAffine");
    program.classes.get_mut("ChildAffine").unwrap().super_name = Some(CLASS.into());
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let fields = (0..3)
        .flat_map(|row| {
            (0..4).map(move |column| (format!("{CLASS}.m{row}{column}:I"), HeapValue::Int(0)))
        })
        .chain([(String::from("ChildAffine.m00:I"), HeapValue::Int(-99))])
        .collect();
    let child = machine
        .heap
        .managed
        .allocate_object("ChildAffine", fields)
        .unwrap();
    machine
        .micro3d_set_affine(child, micro3d::AffineTrans::IDENTITY)
        .unwrap();
    assert!(matches!(
        machine
            .heap
            .managed
            .field(child, "ChildAffine.m00:I")
            .unwrap(),
        HeapValue::Int(-99)
    ));
    machine
        .heap
        .managed
        .set_field(child, &format!("{CLASS}.m00:I"), HeapValue::Int(8192))
        .unwrap();
    assert_eq!(machine.micro3d_affine(child).unwrap().values[0], 8192);
    assert!(machine.micro3d_vector(child).is_err());
    let vector = machine
        .heap
        .managed
        .allocate_object(
            "com/mascotcapsule/micro3d/v3/Vector3D",
            ["x", "y", "z"]
                .map(|name| {
                    (
                        format!("com/mascotcapsule/micro3d/v3/Vector3D.{name}:I"),
                        HeapValue::Int(0),
                    )
                })
                .into(),
        )
        .unwrap();
    let value = micro3d::Vector3D::new(i32::MIN, 42, i32::MAX);
    machine.micro3d_set_vector(vector, value).unwrap();
    assert_eq!(machine.micro3d_vector(vector).unwrap(), value);
    assert!(machine.micro3d_affine(vector).is_err());
}

#[test]
fn affine_array_ranges_preserve_unused_elements_and_validate_before_mutation() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let affine = affine_object(&mut machine);
    let source = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 64)
        .unwrap();
    machine
        .graphics_int_array_mut(source)
        .unwrap()
        .fill(HeapValue::Float(f32::NAN));
    let expected = micro3d::AffineTrans::new(std::array::from_fn(|index| index as i32 * 17 - 33));
    for (slot, value) in machine.graphics_int_array_mut(source).unwrap()[7..19]
        .iter_mut()
        .zip(expected.values)
    {
        *slot = HeapValue::Int(value);
    }
    for name in ["<init>", "set"] {
        let method = affine_method(name, "([II)V");
        let mut args = [
            Value::Reference(Some(affine)),
            Value::Reference(Some(source)),
            Value::Int(7),
        ];
        machine.invoke_micro3d_native(&method, &args).unwrap();
        assert_eq!(machine.micro3d_affine(affine).unwrap(), expected);
        for offset in [-1, 63, i32::MAX, 6] {
            args[2] = Value::Int(offset);
            assert!(machine.invoke_micro3d_native(&method, &args).is_err());
            assert_eq!(machine.micro3d_affine(affine).unwrap(), expected);
        }
    }
    let destination = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 32)
        .unwrap();
    machine
        .graphics_int_array_mut(destination)
        .unwrap()
        .fill(HeapValue::Int(-77));
    let method = affine_method("get", "([II)V");
    let mut args = [
        Value::Reference(Some(affine)),
        Value::Reference(Some(destination)),
        Value::Int(11),
    ];
    machine.invoke_micro3d_native(&method, &args).unwrap();
    let result = machine.graphics_int_array_snapshot(destination).unwrap();
    assert_eq!(&result[..11], &[-77; 11]);
    assert_eq!(&result[11..23], &expected.values);
    assert_eq!(&result[23..], &[-77; 9]);
    for offset in [-1, 21, i32::MAX] {
        args[2] = Value::Int(offset);
        assert!(machine.invoke_micro3d_native(&method, &args).is_err());
        assert_eq!(
            machine.graphics_int_array_snapshot(destination).unwrap(),
            result
        );
    }
}

#[test]
fn affine_rows_consume_three_by_four_values_and_share_constructor_and_setter_rules() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let affine = affine_object(&mut machine);
    let rows = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Reference("[I".into()), 4)
        .unwrap();
    let expected = micro3d::AffineTrans::new(std::array::from_fn(|index| index as i32 * 13));
    for row in 0..3 {
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Int, 6)
            .unwrap();
        machine
            .m3g_write_int_array(array, &expected.values[row * 4..row * 4 + 4])
            .unwrap();
        machine.graphics_int_array_mut(array).unwrap()[4..].fill(HeapValue::Float(f32::NAN));
        machine
            .heap
            .managed
            .array_set(rows, row as i32, HeapValue::Reference(Some(array)))
            .unwrap();
    }
    let args = [Value::Reference(Some(affine)), Value::Reference(Some(rows))];
    for name in ["<init>", "set"] {
        machine
            .invoke_micro3d_native(&affine_method(name, "([[I)V"), &args)
            .unwrap();
        assert_eq!(machine.micro3d_affine(affine).unwrap(), expected);
    }
    machine
        .heap
        .managed
        .array_set(rows, 2, HeapValue::Reference(None))
        .unwrap();
    for name in ["<init>", "set"] {
        assert_eq!(
            machine
                .invoke_micro3d_native(&affine_method(name, "([[I)V"), &args)
                .err()
                .unwrap()
                .code(),
            "null-pointer-exception"
        );
        assert_eq!(machine.micro3d_affine(affine).unwrap(), expected);
    }
}

#[test]
#[ignore = "manual heap-backed affine field access throughput measurement"]
fn micro3d_affine_fields_throughput() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let affine = affine_object(&mut machine);
    let started = std::time::Instant::now();
    for index in 0..16_384 {
        let mut value = micro3d::AffineTrans::IDENTITY;
        value.values[3] = index;
        machine
            .micro3d_set_affine(affine, std::hint::black_box(value))
            .unwrap();
        assert_eq!(machine.micro3d_affine(affine).unwrap(), value);
    }
    eprintln!("16384 affine write/read pairs: {:?}", started.elapsed());
}
