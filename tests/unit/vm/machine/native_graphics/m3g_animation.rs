use super::*;

#[test]
fn animation_track_constructor_rejects_incompatible_keyframes_before_binding_guest_state() {
    let constructor = runtime_method(
        "javax/microedition/m3g/AnimationTrack",
        "<init>",
        "(Ljavax/microedition/m3g/KeyframeSequence;I)V",
        &[0xb1],
        0,
        3,
        Vec::new(),
        false,
    );
    for (property, components, valid) in [
        (270, 1, true),
        (270, 2, false),
        (270, 3, true),
        (259, 2, true),
        (259, 3, false),
        (259, 4, true),
        (266, 5, true),
        (255, 1, false),
    ] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let (sequence, _) = m3g_support::object(
            &mut machine,
            "javax/microedition/m3g/KeyframeSequence",
            m3g::ObjectKind::KeyframeSequence(
                m3g::KeyframeSequenceState::new(1, components, m3g::Interpolation::Step).unwrap(),
            ),
        );
        let guest = machine
            .heap
            .managed
            .allocate_object("javax/microedition/m3g/AnimationTrack", HashMap::new())
            .unwrap();
        let before = machine.m3g.runtime.counters();
        let result = machine.invoke_m3g_native(
            &constructor,
            &[
                Value::Reference(Some(guest)),
                Value::Reference(Some(sequence)),
                Value::Int(property),
            ],
            1,
        );
        if valid {
            assert!(matches!(result.unwrap(), CallOutcome::Return(None)));
            assert!(machine.m3g.runtime.resolve_guest(guest.to_raw()).is_ok());
        } else {
            assert_eq!(
                java_error_class(
                    &result
                        .err()
                        .expect("incompatible keyframes must fail construction")
                ),
                Some("java/lang/IllegalArgumentException")
            );
            assert!(machine.m3g.runtime.resolve_guest(guest.to_raw()).is_err());
            assert_eq!(machine.m3g.runtime.counters(), before);
        }
    }
}

#[test]
fn adding_an_incompatible_track_reports_a_java_argument_error_immediately() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (node, native_node) = m3g_support::object(
        &mut machine,
        "javax/microedition/m3g/Group",
        m3g::ObjectKind::Group {
            node: m3g::NodeState::default(),
            children: Vec::new(),
        },
    );
    let (_, sequence) = m3g_support::object(
        &mut machine,
        "javax/microedition/m3g/KeyframeSequence",
        m3g::ObjectKind::KeyframeSequence(
            m3g::KeyframeSequenceState::new(1, 3, m3g::Interpolation::Step).unwrap(),
        ),
    );
    let (track, _) = m3g_support::object(
        &mut machine,
        "javax/microedition/m3g/AnimationTrack",
        m3g::ObjectKind::AnimationTrack {
            sequence,
            controller: None,
            property: 258,
        },
    );
    let add = runtime_method(
        "javax/microedition/m3g/Object3D",
        "addAnimationTrack",
        "(Ljavax/microedition/m3g/AnimationTrack;)V",
        &[0xb1],
        0,
        2,
        Vec::new(),
        false,
    );
    let before = machine.m3g.runtime.counters();
    let error = machine
        .invoke_m3g_native(
            &add,
            &[Value::Reference(Some(node)), Value::Reference(Some(track))],
            1,
        )
        .err()
        .expect("incompatible track must fail attachment");
    assert_eq!(
        java_error_class(&error),
        Some("java/lang/IllegalArgumentException")
    );
    assert!(
        machine
            .m3g
            .runtime
            .animation_tracks(native_node)
            .unwrap()
            .is_empty()
    );
    assert_eq!(machine.m3g.runtime.counters(), before);
}
