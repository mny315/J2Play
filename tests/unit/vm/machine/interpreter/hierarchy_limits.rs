use super::*;

fn linked_class(name: &str, dependencies: &[&str], interface: bool) -> ClassFile {
    let mut constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8(name.into())),
    ];
    let mut indices = Vec::new();
    for dependency in dependencies {
        let index = u16::try_from(constants.len()).unwrap();
        indices.push(index);
        constants.push(Some(Constant::Class {
            name_index: index + 1,
        }));
        constants.push(Some(Constant::Utf8((*dependency).into())));
    }
    ClassFile {
        minor_version: 0,
        major_version: 50,
        constant_pool: constants,
        access_flags: if interface { 0x0601 } else { 0x0021 },
        this_class: 1,
        super_class: if interface {
            0
        } else {
            indices.first().copied().unwrap_or(0)
        },
        interfaces: if interface { indices } else { vec![] },
        fields: vec![],
        methods: vec![],
        attributes: vec![],
    }
}

#[test]
fn linking_rejects_inheritance_cycles_and_rolls_back() {
    for interface in [false, true] {
        let mut program = Program::new();
        let limits = Limits::default();
        program
            .add_class(&linked_class("A", &["B"], interface), &limits)
            .unwrap();
        program
            .add_class(&linked_class("B", &["C"], interface), &limits)
            .unwrap();
        let before = (program.runtime_bytes, program.constant_pool_count);
        let error = program
            .add_class(&linked_class("C", &["A"], interface), &limits)
            .unwrap_err();
        assert_eq!(error.code(), "class-circularity");
        assert!(!program.contains_class("C"));
        assert_eq!((program.runtime_bytes, program.constant_pool_count), before);
        program
            .add_class(&linked_class("C", &[], interface), &limits)
            .unwrap();
        assert!(program.is_assignable_to("A", "C"));
        assert_eq!(
            program
                .add_class(&linked_class("Self", &["Self"], interface), &limits)
                .unwrap_err()
                .code(),
            "class-circularity"
        );
    }
}

#[test]
fn diamond_interface_hierarchy_is_not_a_cycle() {
    let mut program = Program::new();
    let limits = Limits::default();
    for (name, parents) in [
        ("Root", vec![]),
        ("Left", vec!["Root"]),
        ("Right", vec!["Root"]),
        ("Child", vec!["Left", "Right"]),
    ] {
        program
            .add_class(&linked_class(name, &parents, true), &limits)
            .unwrap();
    }
    assert!(program.is_assignable_to("Child", "Root"));
}

#[test]
fn initializer_parent_traversal_obeys_depth_limit_without_clinit() {
    let mut program = Program::new();
    for index in 0..8 {
        program
            .add_class(
                &linked_class(&format!("C{index}"), &[&format!("C{}", index + 1)], false),
                &Limits::default(),
            )
            .unwrap();
    }
    program
        .add_class(&linked_class("C8", &[], false), &Limits::default())
        .unwrap();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        max_frames: 4,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    assert_eq!(
        machine.initialize_class("C0", 1).unwrap_err().code(),
        "stack-overflow"
    );
    assert!(machine.classes.initializing.is_empty());
    assert!(!machine.classes.initialized.contains("C0"));
}

fn linear_hierarchy(count: usize) -> Program {
    let mut program = Program::new();
    for index in 0..count {
        let parent = format!("C{}", index.saturating_sub(1));
        let parents = if index == 0 {
            vec![]
        } else {
            vec![parent.as_str()]
        };
        let mut class = linked_class(&format!("C{index}"), &parents, false);
        let name_index = u16::try_from(class.constant_pool.len()).unwrap();
        class.constant_pool.extend([
            Some(Constant::Utf8("value".into())),
            Some(Constant::Utf8("I".into())),
        ]);
        class.fields.push(classfile::Member {
            access_flags: 1,
            name_index,
            descriptor_index: name_index + 1,
            attributes: vec![],
        });
        program.add_class(&class, &Limits::default()).unwrap();
    }
    let method = runtime_method(
        "C0",
        "answer",
        "()I",
        &[0x04, 0xac],
        1,
        1,
        vec![None],
        false,
    );
    program.methods.insert(method.key.clone(), method);
    program
}

#[test]
fn superclass_lookup_keeps_field_order_and_nearest_method() {
    let mut program = linear_hierarchy(64);
    let override_method = runtime_method(
        "C31",
        "answer",
        "()I",
        &[0x05, 0xac],
        1,
        1,
        vec![None],
        false,
    );
    program
        .methods
        .insert(override_method.key.clone(), override_method);
    let mut context = DefaultNativeContext;
    let machine = program.machine(Limits::default(), false, &mut context);
    for index in [0, 1, 30, 31, 32, 63] {
        let class = format!("C{index}");
        let method = machine.resolve_virtual(&class, "answer", "()I").unwrap();
        assert_eq!(method.class, if index < 31 { "C0" } else { "C31" });
        let fields = machine.instance_fields(&class).unwrap();
        assert_eq!(fields.len(), index + 1);
        for (owner, field) in fields.iter().enumerate() {
            assert_eq!(field.key.as_ref(), format!("C{owner}.value:I"));
        }
        assert_eq!(
            machine
                .resolve_virtual(&class, "absent", "()V")
                .unwrap_err()
                .code(),
            "abstract-method"
        );
    }
}

#[test]
fn superclass_lookup_reports_missing_classes_and_still_bounds_invalid_cycles() {
    let mut program = linear_hierarchy(8);
    for (parent, fields_error, method_error) in [
        ("Missing", "class-not-found", "abstract-method"),
        ("C7", "class-circularity", "class-circularity"),
    ] {
        // Production linking rejects the cycle; retain a defensive runtime bound.
        program.classes.get_mut("C0").unwrap().super_name = Some(parent.into());
        let mut context = DefaultNativeContext;
        let machine = program.machine(Limits::default(), false, &mut context);
        assert_eq!(
            machine.instance_fields("C7").unwrap_err().code(),
            fields_error
        );
        assert_eq!(
            machine
                .resolve_virtual("C7", "absent", "()V")
                .unwrap_err()
                .code(),
            method_error
        );
    }
}

#[test]
fn type_assignability_matches_transitive_reachability_with_shared_and_cyclic_parents() {
    const COUNT: usize = 32;
    let names = (0..COUNT)
        .map(|index| format!("C{index}"))
        .collect::<Vec<_>>();
    for branches in [false, true] {
        for cyclic in [false, true] {
            let mut program = linear_hierarchy(COUNT);
            let mut reachable = [[false; COUNT]; COUNT];
            for index in 0..COUNT {
                reachable[index][index] = true;
                if index > 0 {
                    reachable[index][index - 1] = true;
                }
                if branches && index.is_multiple_of(3) && index > 0 {
                    for parent in [index / 2, (index - 1) / 2] {
                        program
                            .classes
                            .get_mut(&names[index])
                            .unwrap()
                            .interfaces
                            .push(names[parent].clone());
                        reachable[index][parent] = true;
                    }
                }
            }
            if cyclic {
                // Linking rejects cycles, but runtime queries remain bounded
                // even if a synthetic program bypasses the linker.
                program.classes.get_mut("C0").unwrap().super_name = Some(names[COUNT - 1].clone());
                reachable[0][COUNT - 1] = true;
            }
            // Independent all-pairs closure, without following the runtime walk.
            for intermediate in 0..COUNT {
                for source in 0..COUNT {
                    for target in 0..COUNT {
                        reachable[source][target] |=
                            reachable[source][intermediate] && reachable[intermediate][target];
                    }
                }
            }
            for (source, name) in names.iter().enumerate() {
                for (target, other) in names.iter().enumerate() {
                    assert_eq!(
                        program.is_assignable_to(name, other),
                        reachable[source][target],
                        "{name} -> {other}, branches={branches}, cyclic={cyclic}"
                    );
                }
                assert!(!program.is_assignable_to(name, "Missing"));
                assert!(!program.is_assignable_to("Missing", name));
            }
            assert!(program.is_assignable_to("Missing", "Missing"));
        }
    }
    let mut program = linear_hierarchy(8);
    program.classes.get_mut("C0").unwrap().super_name = Some("External".into());
    assert!(program.is_assignable_to("C7", "External"));
    assert!(!program.is_assignable_to("C7", "Missing"));
}

#[test]
#[ignore = "manual release throughput measurement"]
fn type_assignability_throughput() {
    use std::{hint::black_box, time::Instant};
    for count in [1, 8, 64] {
        for branches in [false, true] {
            let mut program = linear_hierarchy(count);
            if branches {
                program
                    .classes
                    .get_mut(&format!("C{}", count - 1))
                    .unwrap()
                    .interfaces
                    .push("Interface".into());
                program
                    .classes
                    .insert("Interface".into(), test_class_definition(None));
            }
            let class = format!("C{}", count - 1);
            for target in ["C0", "Missing"] {
                let started = Instant::now();
                let mut checksum = 0;
                for _ in 0..100_000 {
                    checksum += usize::from(black_box(
                        program.is_assignable_to(black_box(&class), black_box(target)),
                    ));
                }
                eprintln!(
                    "classes={count} branches={branches} target={target}: {:?}; checksum={checksum}",
                    started.elapsed()
                );
            }
        }
    }
}

#[test]
#[ignore = "manual release throughput measurement"]
fn superclass_lookup_throughput() {
    use std::{hint::black_box, time::Instant};
    for count in [1, 8, 64] {
        let program = linear_hierarchy(count);
        let mut context = DefaultNativeContext;
        let machine = program.machine(Limits::default(), false, &mut context);
        let class = format!("C{}", count - 1);
        for fields in [false, true] {
            let started = Instant::now();
            let mut checksum = 0;
            for _ in 0..100_000 {
                checksum += if fields {
                    black_box(machine.instance_fields(black_box(&class)).unwrap()).len()
                } else {
                    black_box(
                        machine
                            .resolve_virtual(black_box(&class), "answer", "()I")
                            .unwrap(),
                    )
                    .class
                    .len()
                };
            }
            eprintln!(
                "classes={count} fields={fields}: {:?}; checksum={checksum}",
                started.elapsed()
            );
        }
    }
}
