use super::*;

#[test]
fn m3g_child_wrapper_keeps_its_parent_wrapper_alive_across_gc() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let _reserved_zero = machine
        .heap
        .managed
        .allocate_object("sentinel", HashMap::new())
        .unwrap();
    let parent = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Group", HashMap::new())
        .unwrap();
    let child = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Group", HashMap::new())
        .unwrap();
    let parent_native = machine
        .m3g
        .runtime
        .create(
            Some(parent.to_raw()),
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    let child_native = machine
        .m3g
        .runtime
        .create(
            Some(child.to_raw()),
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    machine
        .m3g
        .runtime
        .add_child(parent_native, child_native)
        .unwrap();

    machine.collect_heap(vec![child]);

    assert!(machine.heap.managed.get(parent).is_ok());
    let get_parent = runtime_method(
        "javax/microedition/m3g/Node",
        "getParent",
        "()Ljavax/microedition/m3g/Node;",
        &[0xb0],
        1,
        1,
        Vec::new(),
        false,
    );
    assert!(matches!(
        machine
            .invoke_m3g_native(&get_parent, &[Value::Reference(Some(child))], 1)
            .unwrap(),
        CallOutcome::Return(Some(Value::Reference(Some(actual)))) if actual == parent
    ));
}

#[test]
fn m3g_properties_allocation_failure_releases_only_its_temporary_roots() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 96,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let outer = machine
        .heap
        .managed
        .allocate_object("test/Outer", HashMap::new())
        .unwrap();
    let baseline = machine.heap.managed.bytes();
    machine.heap.temporary_roots.push(outer);
    for _ in 0..3 {
        let error = machine.m3g_properties_table(&[]).unwrap_err();
        assert_eq!(error.code(), MANAGED_HEAP_LIMIT_CODE);
        assert_eq!(machine.heap.temporary_roots, [outer]);
        let roots = machine.roots(&[], &[]);
        machine.collect_heap(roots);
        assert_eq!(machine.heap.managed.bytes(), baseline);
        assert!(machine.heap.managed.get(outer).is_ok());
    }
    machine.heap.temporary_roots.pop();
    let roots = machine.roots(&[], &[]);
    machine.collect_heap(roots);
    assert_eq!(machine.heap.managed.bytes(), 0);
}

#[test]
fn micro3d_native_pressure_collects_unreachable_guest_wrappers() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        micro3d_objects: 1,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    let constructor = runtime_method(
        "com/mascotcapsule/micro3d/v3/Effect3D",
        "<init>",
        "()V",
        &[0xb1],
        0,
        1,
        Vec::new(),
        false,
    );

    let _reserved_zero = machine
        .heap
        .managed
        .allocate_object("sentinel", HashMap::new())
        .unwrap();
    let first = machine
        .heap
        .managed
        .allocate_object("com/mascotcapsule/micro3d/v3/Effect3D", HashMap::new())
        .unwrap();
    machine
        .invoke_micro3d_native(&constructor, &[Value::Reference(Some(first))])
        .unwrap();
    let second = machine
        .heap
        .managed
        .allocate_object("com/mascotcapsule/micro3d/v3/Effect3D", HashMap::new())
        .unwrap();

    let outcome = machine
        .invoke_micro3d_native(&constructor, &[Value::Reference(Some(second))])
        .unwrap();

    assert!(matches!(outcome, CallOutcome::Return(None)));
    assert!(machine.heap.managed.get(first).is_err());
    assert!(machine.heap.managed.get(second).is_ok());
    assert!(machine.micro3d.runtime.kind(first.to_raw()).is_err());
    assert!(matches!(
        machine.micro3d.runtime.kind(second.to_raw()),
        Ok(micro3d::ObjectKind::Effect(_))
    ));
    assert_eq!(machine.micro3d.runtime.metrics().live_objects, 1);
}

#[test]
fn m3g_native_pressure_collects_unreachable_guest_wrappers() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        m3g_arena: m3g::ArenaLimits {
            objects: 1,
            ..m3g::ArenaLimits::default()
        },
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
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

    let _reserved_zero = machine
        .heap
        .managed
        .allocate_object("sentinel", HashMap::new())
        .unwrap();
    let first = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Transform", HashMap::new())
        .unwrap();
    machine
        .invoke_m3g_native(&constructor, &[Value::Reference(Some(first))], 1)
        .unwrap();
    let second = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Transform", HashMap::new())
        .unwrap();

    let outcome = machine
        .invoke_m3g_native(&constructor, &[Value::Reference(Some(second))], 1)
        .unwrap();

    assert!(matches!(outcome, CallOutcome::Return(None)));
    assert!(machine.heap.managed.get(first).is_err());
    assert!(machine.heap.managed.get(second).is_ok());
    assert!(machine.m3g.runtime.resolve_guest(first.to_raw()).is_err());
    assert!(matches!(
        machine
            .m3g
            .runtime
            .kind(machine.m3g.runtime.resolve_guest(second.to_raw()).unwrap()),
        Ok(m3g::ObjectKind::Transform(_))
    ));
    assert_eq!(machine.m3g.runtime.counters().0, 1);
}

#[test]
fn m3g_child_growth_retries_after_collecting_an_unreachable_wrapper() {
    let mut probe = m3g::Runtime::default();
    let probe_group = probe
        .create(
            None,
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    let probe_child = probe
        .create(None, m3g::ObjectKind::Node(m3g::NodeState::default()))
        .unwrap();
    let rooted_bytes = probe.counters().2;
    probe.create(None, m3g::ObjectKind::Object).unwrap();
    let full_bytes = probe.counters().2;
    probe.add_child(probe_group, probe_child).unwrap();
    assert!(probe.counters().2 > full_bytes);
    assert!(probe.counters().2 - full_bytes <= full_bytes - rooted_bytes);

    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        m3g_arena: m3g::ArenaLimits {
            objects: 3,
            bytes: full_bytes,
        },
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
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

    let _reserved_zero = machine
        .heap
        .managed
        .allocate_object("sentinel", HashMap::new())
        .unwrap();
    let group = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Group", HashMap::new())
        .unwrap();
    let child = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Node", HashMap::new())
        .unwrap();
    let dead = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Object3D", HashMap::new())
        .unwrap();
    let group_native = machine
        .m3g
        .runtime
        .create(
            Some(group.to_raw()),
            m3g::ObjectKind::Group {
                node: m3g::NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap();
    let child_native = machine
        .m3g
        .runtime
        .create(
            Some(child.to_raw()),
            m3g::ObjectKind::Node(m3g::NodeState::default()),
        )
        .unwrap();
    let dead_native = machine
        .m3g
        .runtime
        .create(Some(dead.to_raw()), m3g::ObjectKind::Object)
        .unwrap();
    assert_eq!(machine.m3g.runtime.counters().2, full_bytes);

    let outcome = machine
        .invoke_m3g_native(
            &add_child,
            &[Value::Reference(Some(group)), Value::Reference(Some(child))],
            1,
        )
        .unwrap();

    assert!(matches!(outcome, CallOutcome::Return(None)));
    assert!(machine.heap.managed.get(group).is_ok());
    assert!(machine.heap.managed.get(child).is_ok());
    assert!(machine.heap.managed.get(dead).is_err());
    assert!(machine.m3g.runtime.kind(dead_native).is_err());
    assert_eq!(
        machine.m3g.runtime.children(group_native).unwrap(),
        [child_native]
    );
    assert_eq!(
        machine.m3g.runtime.parent(child_native).unwrap(),
        Some(group_native)
    );
}

#[test]
pub(crate) fn gc_expands_native_scene_edges_from_transitively_reachable_java_objects() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let child_guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/VertexArray", HashMap::new())
        .unwrap();
    let root_guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/VertexBuffer", HashMap::new())
        .unwrap();
    let holder = machine
        .heap
        .managed
        .allocate_object(
            "Holder",
            HashMap::from([(
                "Holder.value:Ljava/lang/Object;".to_owned(),
                HeapValue::Reference(Some(root_guest)),
            )]),
        )
        .unwrap();

    let mut positions = m3g::VertexArrayState::new(3, 3, m3g::VertexComponent::Short).unwrap();
    positions
        .set_shorts(0, 3, &[0, 0, 0, 1, 0, 0, 0, 1, 0])
        .unwrap();
    let child_native = machine
        .m3g
        .runtime
        .create(
            Some(child_guest.to_raw()),
            m3g::ObjectKind::VertexArray(positions.clone()),
        )
        .unwrap();
    let mut state = m3g::VertexBufferState::default();
    state.set_positions(Some(positions), 1.0, [0.0; 3]).unwrap();
    machine
        .m3g
        .runtime
        .create(
            Some(root_guest.to_raw()),
            m3g::ObjectKind::VertexBuffer {
                state,
                arrays: [Some(child_native), None, None, None, None],
            },
        )
        .unwrap();

    let roots = machine.roots(&[Some(Value::Reference(Some(holder)))], &[]);
    machine.collect_heap(roots);

    assert!(machine.heap.managed.get(root_guest).is_ok());
    assert!(machine.heap.managed.get(child_guest).is_ok());
    assert!(matches!(
        machine.m3g.runtime.kind(child_native).unwrap(),
        m3g::ObjectKind::VertexArray(_)
    ));
}

#[test]
pub(crate) fn gc_ignores_stale_native_user_object_handles() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let stale = machine
        .heap
        .managed
        .allocate_object("Discarded", HashMap::new())
        .unwrap();
    machine.heap.managed.collect([]);
    assert!(machine.heap.managed.get(stale).is_err());

    let root_guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Object3D", HashMap::new())
        .unwrap();
    let holder = machine
        .heap
        .managed
        .allocate_object(
            "Holder",
            HashMap::from([(
                "Holder.value:Ljava/lang/Object;".to_owned(),
                HeapValue::Reference(Some(root_guest)),
            )]),
        )
        .unwrap();
    let native = machine
        .m3g
        .runtime
        .create(Some(root_guest.to_raw()), m3g::ObjectKind::Object)
        .unwrap();
    machine
        .m3g
        .runtime
        .set_user_object(native, Some(stale.to_raw()))
        .unwrap();

    let roots = machine.roots(&[Some(Value::Reference(Some(holder)))], &[]);
    machine.collect_heap(roots);

    assert!(machine.heap.managed.get(root_guest).is_ok());
    assert_eq!(
        machine.m3g.runtime.user_object(native).unwrap(),
        Some(stale.to_raw())
    );
}
