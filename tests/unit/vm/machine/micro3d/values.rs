use super::*;
use crate::machine::{DefaultNativeContext, Limits, Program};
use std::collections::HashMap;

#[test]
fn disposed_native_history_collects_unreachable_wrappers_and_retries_once() {
    for retain_disposed in [false, true] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        machine.micro3d.runtime =
            micro3d::Runtime::new(1, 3 * size_of::<micro3d::ObjectKind>(), 1, 1).unwrap();
        // A bootstrapped VM has allocated ordinary Java objects before the
        // first native wrapper; raw zero is reserved by the native arena.
        machine
            .heap
            .managed
            .allocate_object("java/lang/Object", HashMap::new())
            .unwrap();
        let mut disposed = Vec::new();
        for _ in 0..3 {
            let wrapper = machine
                .heap
                .managed
                .allocate_object("com/mascotcapsule/micro3d/v3/Graphics3D", HashMap::new())
                .unwrap();
            machine
                .micro3d
                .runtime
                .create(wrapper.to_raw(), micro3d::ObjectKind::Graphics)
                .unwrap();
            machine.micro3d.runtime.dispose(wrapper.to_raw()).unwrap();
            disposed.push(wrapper);
        }
        let current = machine
            .heap
            .managed
            .allocate_object("com/mascotcapsule/micro3d/v3/Graphics3D", HashMap::new())
            .unwrap();
        let mut roots = vec![Value::Reference(Some(current))];
        if retain_disposed {
            roots.extend(
                disposed
                    .iter()
                    .map(|handle| Value::Reference(Some(*handle))),
            );
        }
        let mut attempts = 0;
        let result = machine.micro3d_allocate_native(&roots, |runtime| {
            attempts += 1;
            runtime.create(current.to_raw(), micro3d::ObjectKind::Graphics)
        });
        assert_eq!(attempts, 2);
        assert!(machine.heap.managed.get(current).is_ok());
        for wrapper in disposed {
            assert_eq!(machine.heap.managed.get(wrapper).is_ok(), retain_disposed);
        }
        if retain_disposed {
            assert_eq!(result.unwrap_err().code(), "resource-limit");
            assert_eq!(machine.micro3d.runtime.metrics().live_objects, 0);
        } else {
            result.unwrap();
            assert_eq!(machine.micro3d.runtime.metrics().live_objects, 1);
            assert!(machine.micro3d.runtime.kind(current.to_raw()).is_ok());
        }
    }
}
