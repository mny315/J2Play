//! EGL output buffers are validated before any results are published.

use super::*;

mod window_surface;

fn egl_arguments(machine: &mut Machine<'_, '_>) -> Vec<Value> {
    let egl = machine
        .allocate_native_instance("javax/microedition/khronos/egl/EGLImpl", &[])
        .unwrap();
    machine.jsr239.egl = Some(egl);
    let display = machine
        .allocate_native_instance("javax/microedition/khronos/egl/EGLDisplay", &[])
        .unwrap();
    vec![Value::Reference(Some(egl)), Value::Reference(Some(display))]
}

fn egl_call(
    machine: &mut Machine<'_, '_>,
    name: &str,
    arguments: &[Value],
) -> Result<CallOutcome, EmuError> {
    let method = machine
        .program
        .methods
        .values()
        .find(|method| {
            method.key.class == "javax/microedition/khronos/egl/EGLImpl" && method.key.name == name
        })
        .unwrap()
        .clone();
    machine.call(&method, arguments, 1)
}

fn int_array(machine: &mut Machine<'_, '_>, values: &[i32]) -> Handle {
    let array = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, i32::try_from(values.len()).unwrap())
        .unwrap();
    for (index, &value) in values.iter().enumerate() {
        machine
            .heap
            .managed
            .array_set(array, i32::try_from(index).unwrap(), HeapValue::Int(value))
            .unwrap();
    }
    array
}

#[test]
fn egl_initialize_accepts_optional_versions_and_preserves_short_arrays() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let mut arguments = egl_arguments(&mut machine);
    arguments.push(Value::Reference(None));
    assert!(matches!(
        egl_call(&mut machine, "eglInitialize", &arguments).unwrap(),
        CallOutcome::Return(Some(Value::Int(1)))
    ));
    for values in [&[][..], &[47][..], &[47, 53, 59][..]] {
        let versions = int_array(&mut machine, values);
        arguments[2] = Value::Reference(Some(versions));
        let outcome = egl_call(&mut machine, "eglInitialize", &arguments).unwrap();
        if values.len() < 2 {
            assert!(
                matches!(outcome, CallOutcome::Throw(exception) if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException")
            );
            assert_eq!(
                machine.graphics_int_array_snapshot(versions).unwrap(),
                values
            );
        } else {
            assert!(matches!(outcome, CallOutcome::Return(Some(Value::Int(1)))));
            assert_eq!(
                machine.graphics_int_array_snapshot(versions).unwrap(),
                [1, 1, 59]
            );
        }
    }
}

#[test]
fn egl_config_queries_validate_both_outputs_before_writing() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let base = egl_arguments(&mut machine);
    for name in ["eglGetConfigs", "eglChooseConfig"] {
        for (length, size, count_length) in [(0, 1, 1), (1, 2, 1), (1, 1, 0)] {
            let configs = machine
                .heap
                .managed
                .allocate_array(
                    ArrayKind::Reference("javax/microedition/khronos/egl/EGLConfig".into()),
                    length,
                )
                .unwrap();
            let count = int_array(&mut machine, &vec![47; count_length]);
            let mut arguments = base.clone();
            if name == "eglChooseConfig" {
                arguments.push(Value::Reference(None));
            }
            arguments.extend([
                Value::Reference(Some(configs)),
                Value::Int(size),
                Value::Reference(Some(count)),
            ]);
            let outcome = egl_call(&mut machine, name, &arguments).unwrap();
            assert!(
                matches!(outcome, CallOutcome::Throw(exception) if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException"),
                "{name}: length={length}, size={size}, count_length={count_length}"
            );
            assert_eq!(
                machine.graphics_int_array_snapshot(count).unwrap(),
                vec![47; count_length]
            );
            for index in 0..length {
                assert_eq!(
                    machine.heap.managed.array_get(configs, index).unwrap(),
                    HeapValue::Reference(None)
                );
            }
        }
    }
}

#[test]
fn egl_config_queries_distinguish_count_only_from_empty_destinations() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let base = egl_arguments(&mut machine);
    for name in ["eglGetConfigs", "eglChooseConfig"] {
        for (length, size, expected) in [(None, 0, 1), (Some(0), 0, 0), (Some(2), 2, 1)] {
            let configs = length.map(|length| {
                machine
                    .heap
                    .managed
                    .allocate_array(
                        ArrayKind::Reference("javax/microedition/khronos/egl/EGLConfig".into()),
                        length,
                    )
                    .unwrap()
            });
            let count = int_array(&mut machine, &[47, 53]);
            let mut arguments = base.clone();
            if name == "eglChooseConfig" {
                arguments.push(Value::Reference(None));
            }
            arguments.extend([
                Value::Reference(configs),
                Value::Int(size),
                Value::Reference(Some(count)),
            ]);
            assert!(matches!(
                egl_call(&mut machine, name, &arguments).unwrap(),
                CallOutcome::Return(Some(Value::Int(1)))
            ));
            assert_eq!(
                machine.graphics_int_array_snapshot(count).unwrap(),
                [expected, 53]
            );
            if let Some(configs) = configs.filter(|_| size > 0) {
                let HeapValue::Reference(Some(config)) =
                    machine.heap.managed.array_get(configs, 0).unwrap()
                else {
                    panic!("missing EGL config");
                };
                assert_eq!(
                    machine.object_class(config).unwrap(),
                    "javax/microedition/khronos/egl/EGLConfig"
                );
                assert_eq!(
                    machine.heap.managed.array_get(configs, 1).unwrap(),
                    HeapValue::Reference(None)
                );
            }
        }
    }
}

#[test]
fn egl_config_allocation_failure_preserves_output_arrays() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 1024,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let mut arguments = egl_arguments(&mut machine);
    let configs = machine
        .heap
        .managed
        .allocate_array(
            ArrayKind::Reference("javax/microedition/khronos/egl/EGLConfig".into()),
            1,
        )
        .unwrap();
    let count = int_array(&mut machine, &[47]);
    arguments.extend([
        Value::Reference(Some(configs)),
        Value::Int(1),
        Value::Reference(Some(count)),
    ]);
    let remaining = 1024 - machine.heap.managed.bytes() - 24;
    let filler = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, i32::try_from(remaining).unwrap())
        .unwrap();
    machine.heap.temporary_roots.push(filler);
    assert_eq!(
        machine
            .invoke_egl_native("eglGetConfigs", "", &arguments)
            .err()
            .unwrap()
            .code(),
        MANAGED_HEAP_LIMIT_CODE
    );
    assert_eq!(machine.graphics_int_array_snapshot(count).unwrap(), [47]);
    assert_eq!(
        machine.heap.managed.array_get(configs, 0).unwrap(),
        HeapValue::Reference(None)
    );
}
