use super::*;

#[test]
#[ignore = "manual heap metadata allocation throughput measurement"]
fn heap_metadata_allocation_throughput() {
    for name_bytes in [32, 4096] {
        for field_count in [0, 8] {
            for arrays in [false, true] {
                if arrays && field_count != 0 {
                    continue;
                }
                let class = format!("Fixture{}", "X".repeat(name_bytes));
                let mut definition = test_class_definition(None);
                definition.fields = (0..field_count)
                    .map(|index| Field {
                        key: format!("{class}.value{index}:I").into(),
                        declaring_class: class.clone(),
                        kind: ValueKind::Int,
                        is_static: false,
                        field_token: FieldToken::new(),
                        instance_slot: None,
                        initial: Value::Int(0),
                        constant_string: None,
                    })
                    .collect();
                let mut program = Program::new();
                program.classes.insert(class.clone(), definition);
                let mut host = DefaultNativeContext;
                let mut machine = program.machine(
                    Limits {
                        max_heap_bytes: 4096,
                        ..Limits::default()
                    },
                    false,
                    &mut host,
                );
                let mut checksum = 0_u64;
                let started = std::time::Instant::now();
                for _ in 0..16_384 {
                    let object = if arrays {
                        machine.allocate_array(
                            ArrayKind::Reference(class.as_str().into()),
                            0,
                            &[],
                            &[],
                        )
                    } else {
                        machine.allocate_native_instance(&class, &[])
                    }
                    .unwrap();
                    checksum = checksum.wrapping_mul(31).wrapping_add(object.to_raw());
                    std::hint::black_box(machine.heap.managed.get(object).unwrap());
                }
                eprintln!(
                    "heap-metadata name_bytes={name_bytes} fields={field_count} arrays={arrays} ns={} checksum={checksum:016x}",
                    started.elapsed().as_nanos()
                );
            }
        }
    }
}

#[test]
#[ignore = "release throughput fixture"]
fn micro3d_value_allocation_throughput() {
    let mut program = Program::new();
    for (class, names) in [
        ("com/mascotcapsule/micro3d/v3/Vector3D", vec!["x", "y", "z"]),
        (
            "com/mascotcapsule/micro3d/v3/AffineTrans",
            vec![
                "m00", "m01", "m02", "m03", "m10", "m11", "m12", "m13", "m20", "m21", "m22", "m23",
            ],
        ),
    ] {
        let mut definition = test_class_definition(None);
        definition.fields = names
            .into_iter()
            .map(|name| Field {
                key: format!("{class}.{name}:I").into(),
                declaring_class: class.into(),
                kind: ValueKind::Int,
                is_static: false,
                field_token: FieldToken::new(),
                instance_slot: None,
                initial: Value::Int(0),
                constant_string: None,
            })
            .collect();
        program.classes.insert(class.into(), definition);
    }
    for heap_bytes in [1024, 1024 * 1024] {
        for affine in [false, true] {
            let mut host = DefaultNativeContext;
            let mut machine = program.machine(
                Limits {
                    max_heap_bytes: heap_bytes,
                    ..Limits::default()
                },
                false,
                &mut host,
            );
            let mut checksum = 0_u64;
            let mut last = None;
            let start = std::time::Instant::now();
            for index in 0..32_768_i32 {
                let object = if affine {
                    machine.micro3d_allocate_affine(
                        micro3d::AffineTrans::new(std::array::from_fn(|slot| index ^ slot as i32)),
                        &[],
                    )
                } else {
                    machine
                        .micro3d_allocate_vector(micro3d::Vector3D::new(index, -index, !index), &[])
                }
                .unwrap();
                checksum = checksum.wrapping_mul(31).wrapping_add(object.to_raw());
                last = Some(object);
            }
            let elapsed = start.elapsed();
            let last = last.unwrap();
            if affine {
                assert_eq!(
                    machine.micro3d_affine(last).unwrap().values,
                    std::array::from_fn(|slot| 32_767 ^ slot as i32)
                );
            } else {
                assert_eq!(
                    machine.micro3d_vector(last).unwrap(),
                    micro3d::Vector3D::new(32_767, -32_767, !32_767)
                );
            }
            println!(
                "value-allocation heap={heap_bytes} affine={affine} ns={} checksum={checksum:016x}",
                elapsed.as_nanos()
            );
        }
    }
}

struct ArrayCancellationContext(std::rc::Rc<std::cell::Cell<usize>>);

impl HostServices for ArrayCancellationContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
    fn execution_cancelled(&self) -> bool {
        let Some(remaining) = self.0.get().checked_sub(1) else {
            return true;
        };
        self.0.set(remaining);
        false
    }
}

#[test]
fn multidimensional_array_cancellation_releases_partial_graph_roots() {
    for allowed in [0, 1, 2, 7, 63] {
        let program = Program::new();
        let remaining = std::rc::Rc::new(std::cell::Cell::new(allowed));
        let mut host = ArrayCancellationContext(std::rc::Rc::clone(&remaining));
        let mut machine = program.machine(Limits::default(), false, &mut host);
        let caller_root = machine
            .heap
            .managed
            .allocate_object("Caller", HashMap::new())
            .unwrap();
        machine.heap.temporary_roots.push(caller_root);
        assert_eq!(
            machine
                .allocate_multi_array("[[[I", &[8, 8, 16], &[], &[])
                .unwrap_err()
                .code(),
            "execution-cancelled"
        );
        assert!(machine.heap.managed.len() <= allowed + 1);
        assert_eq!(machine.heap.temporary_roots, [caller_root]);
        machine.collect_heap(machine.roots(&[], &[]));
        assert_eq!(machine.heap.managed.len(), 1);
        assert!(machine.heap.managed.get(caller_root).is_ok());

        remaining.set(usize::MAX);
        let array = machine
            .allocate_multi_array("[[I", &[2, 3], &[], &[])
            .unwrap();
        assert_eq!(machine.heap.managed.array_length(array).unwrap(), 2);
        for index in 0..2 {
            let HeapValue::Reference(Some(child)) =
                machine.heap.managed.array_get(array, index).unwrap()
            else {
                panic!("completed array must contain its child arrays");
            };
            assert_eq!(machine.heap.managed.array_length(child).unwrap(), 3);
            assert_eq!(
                machine.heap.managed.array_get(child, 2).unwrap(),
                HeapValue::Int(0)
            );
        }
        assert_eq!(machine.heap.temporary_roots, [caller_root]);
    }
}

#[test]
fn linked_instance_allocation_keeps_inherited_slots_and_caller_roots_through_gc() {
    let mut program = Program::new();
    for (class, parent, fields) in [
        (
            "Parent",
            None,
            vec![
                ("same:I", Value::Int(11), false),
                ("static:I", Value::Int(99), true),
            ],
        ),
        (
            "Child",
            Some("Parent"),
            vec![
                ("same:I", Value::Int(22), false),
                ("wide:J", Value::Long(33), false),
            ],
        ),
    ] {
        let mut definition = test_class_definition(parent);
        definition.fields = fields
            .into_iter()
            .map(|(name, initial, is_static)| Field {
                key: format!("{class}.{name}").into(),
                declaring_class: class.into(),
                kind: initial.kind(),
                is_static,
                field_token: FieldToken::new(),
                instance_slot: None,
                initial,
                constant_string: None,
            })
            .collect();
        program.classes.insert(class.into(), definition);
    }
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 128,
            ..Limits::default()
        },
        false,
        &mut host,
    );
    let retained = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 8)
        .unwrap();
    let garbage = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 72)
        .unwrap();
    assert_eq!(machine.heap.managed.bytes(), 128);

    let object = machine
        .allocate_native_instance("Child", &[Value::Reference(Some(retained))])
        .unwrap();
    assert!(machine.heap.managed.get(retained).is_ok());
    assert!(machine.heap.managed.get(garbage).is_err());
    for (slot, (class, index)) in [("Parent", 0), ("Child", 0), ("Child", 1)]
        .into_iter()
        .enumerate()
    {
        let field = &program.classes[class].fields[index];
        assert_eq!(
            machine
                .heap
                .managed
                .field_at(object, slot, &field.field_token, &field.key)
                .unwrap(),
            field.initial,
        );
    }
    assert!(
        machine
            .heap
            .managed
            .field(object, "Parent.static:I")
            .is_err()
    );
}

#[test]
fn char_array_copy_borrows_the_live_string_after_allocation_collection() {
    let program = Program::new();
    let mut host = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 128,
            ..Limits::default()
        },
        false,
        &mut host,
    );
    let string = machine
        .allocate_object("java/lang/String", HashMap::new(), &[], &[])
        .unwrap();
    let units = vec![0, 0xd800, 0xdc00, 0xdc00, 0xffff];
    machine
        .store_string_units(string, units.clone(), 1, &[], &[])
        .unwrap();
    let garbage = machine
        .heap
        .managed
        .allocate_array(
            ArrayKind::Byte,
            i32::try_from(128 - machine.heap.managed.bytes() - 24).unwrap(),
        )
        .unwrap();
    assert_eq!(machine.heap.managed.bytes(), 128);
    let array = machine
        .string_to_char_array(string, &[Value::Reference(Some(string))])
        .unwrap();
    assert!(machine.heap.managed.get(garbage).is_err());
    assert_eq!(machine.heap.string_values[&string], units);
    for (index, unit) in units.iter().enumerate() {
        assert_eq!(
            machine.heap.managed.array_get(array, index as i32).unwrap(),
            HeapValue::Int(i32::from(*unit))
        );
    }
    machine
        .heap
        .managed
        .array_set(array, 0, HeapValue::Int(42))
        .unwrap();
    assert_eq!(machine.heap.string_values[&string], units);
    let empty = machine.intern_string("", &[], &[]).unwrap();
    let empty_array = machine
        .string_to_char_array(empty, &[Value::Reference(Some(empty))])
        .unwrap();
    assert_eq!(machine.heap.managed.array_length(empty_array).unwrap(), 0);
}

#[test]
fn object_reference_fields_survive_collection_before_the_object_exists() {
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
    let leaf = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 8)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(leaf, 0, HeapValue::Int(42))
        .unwrap();
    let child = machine
        .heap
        .managed
        .allocate_object(
            "Child",
            HashMap::from([(
                "Child.bytes:[B".to_owned(),
                HeapValue::Reference(Some(leaf)),
            )]),
        )
        .unwrap();
    let garbage = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 28)
        .unwrap();
    assert_eq!(machine.heap.managed.bytes(), 96);

    // These references are held only in the prospective object's fields,
    // before it has a managed handle or an interpreter frame can root it.
    let holder = machine
        .allocate_object(
            "Holder",
            HashMap::from([(
                "Holder.child:LChild;".to_owned(),
                HeapValue::Reference(Some(child)),
            )]),
            &[],
            &[],
        )
        .unwrap();
    assert_eq!(
        machine
            .heap
            .managed
            .field(holder, "Holder.child:LChild;")
            .unwrap(),
        HeapValue::Reference(Some(child))
    );
    assert_eq!(
        machine.heap.managed.field(child, "Child.bytes:[B").unwrap(),
        HeapValue::Reference(Some(leaf))
    );
    assert_eq!(
        machine.heap.managed.array_get(leaf, 0).unwrap(),
        HeapValue::Int(42)
    );
    assert!(machine.heap.managed.get(garbage).is_err());
    machine.collect_heap(vec![holder]);
    assert_eq!(machine.heap.managed.len(), 3);
    machine.collect_heap(vec![]);
    assert!(machine.heap.managed.is_empty());
}

#[test]
fn string_buffer_append_keeps_utf16_source_alive_across_growth_collection() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            max_heap_bytes: 512,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let backing = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Char, 1)
        .unwrap();
    machine
        .heap
        .managed
        .array_set(backing, 0, HeapValue::Int(65))
        .unwrap();
    let buffer = machine
        .heap
        .managed
        .allocate_object(
            "java/lang/StringBuffer",
            HashMap::from([
                (
                    "java/lang/StringBuffer.value:[C".to_owned(),
                    HeapValue::Reference(Some(backing)),
                ),
                (
                    "java/lang/StringBuffer.count:I".to_owned(),
                    HeapValue::Int(1),
                ),
            ]),
        )
        .unwrap();
    let source = machine
        .allocate_object("java/lang/String", HashMap::new(), &[], &[])
        .unwrap();
    let units = vec![0, 0xd800, 0xdc00, 0xdc00, 0xffff];
    machine
        .store_string_units(source, units.clone(), 1, &[], &[])
        .unwrap();
    let garbage = machine
        .heap
        .managed
        .allocate_array(
            ArrayKind::Byte,
            i32::try_from(512 - machine.heap.managed.bytes() - 24).unwrap(),
        )
        .unwrap();
    assert_eq!(machine.heap.managed.bytes(), 512);
    assert_eq!(
        machine
            .string_buffer_append_string(&[
                Value::Reference(Some(buffer)),
                Value::Reference(Some(source)),
            ])
            .unwrap(),
        buffer
    );
    assert!(machine.heap.managed.get(garbage).is_err());
    assert_eq!(machine.heap.string_values.get(&source), Some(&units));
    machine
        .string_buffer_append_string(&[Value::Reference(Some(buffer)), Value::Reference(None)])
        .unwrap();
    let output = machine
        .string_buffer_to_string(buffer, &[Value::Reference(Some(buffer))])
        .unwrap();
    let mut expected = vec![65];
    expected.extend(units);
    expected.extend("null".encode_utf16());
    assert_eq!(machine.heap.string_values.get(&output), Some(&expected));
}

#[test]
fn side_payloads_share_the_heap_limit_and_are_released_by_gc() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        max_heap_bytes: 59,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    let string = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();
    machine
        .store_string_units(string, vec![u16::from(b'x'); 10], 1, &[], &[])
        .unwrap();
    let image = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 0)
        .unwrap();
    machine
        .store_immutable_image_pixels(
            image,
            Arc::new(ImmutableImagePixels::from_argb(vec![0; 3])),
            &[],
            &[],
        )
        .unwrap();
    assert_eq!(machine.heap.managed.bytes(), 59);

    machine.collect_heap(Vec::new());

    assert_eq!(machine.heap.managed.bytes(), 0);
    assert!(machine.heap.string_values.is_empty());
    assert!(machine.heap.immutable_image_pixels.is_empty());
}

#[test]
fn oversized_string_side_payload_fails_before_map_insertion() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let limits = Limits {
        max_heap_bytes: 32,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    let string = machine
        .heap
        .managed
        .allocate_object("java/lang/String", HashMap::new())
        .unwrap();

    let error = machine
        .store_string_units(string, vec![0; 13], 1, &[], &[])
        .unwrap_err();

    assert_eq!(error.code(), MANAGED_HEAP_LIMIT_CODE);
    assert!(!machine.heap.string_values.contains_key(&string));
    assert_eq!(machine.heap.managed.bytes(), 8);
}

#[test]
fn allocation_failure_is_catchable_in_the_allocating_frame() {
    let constants = vec![
        None,
        Some(Constant::Class { name_index: 2 }),
        Some(Constant::Utf8("java/lang/OutOfMemoryError".into())),
    ];
    let mut main = runtime_method(
        "T",
        "main",
        "()I",
        &[
            0x11, 0x27, 0x10, 0xbc, 8, 0x57, 0x03, 0xac, 0x57, 0x04, 0xac,
        ],
        1,
        0,
        constants,
        true,
    );
    Arc::make_mut(&mut main.exception_table).push(ExceptionHandler {
        start_pc: 0,
        end_pc: 6,
        handler_pc: 8,
        catch_type: 1,
    });
    let mut program = program_with_exception("java/lang/OutOfMemoryError");
    program.methods.insert(main.key.clone(), main);
    let limits = Limits {
        max_heap_bytes: 1_024,
        ..Limits::default()
    };
    let mut context = ManagedHeapNoticeContext::default();

    assert_eq!(
        program
            .execute_with_context("T", "main", "()I", limits, false, &mut context)
            .unwrap()
            .value,
        Some(Value::Int(1))
    );
    assert!(context.notices.is_empty());
}

#[test]
fn uncaught_heap_limit_keeps_its_origin_through_java_throw() {
    let mut program = caught_managed_heap_retry_program();
    let main = program
        .methods
        .get_mut(&MethodKey {
            class: "T".into(),
            name: "main".into(),
            descriptor: "()I".into(),
        })
        .unwrap();
    Arc::make_mut(&mut main.exception_table).clear();
    let error = program
        .execute(
            "T",
            "main",
            "()I",
            Limits {
                max_heap_bytes: 1_024,
                ..Limits::default()
            },
            false,
        )
        .unwrap_err();
    assert_eq!(error.code(), MANAGED_HEAP_LIMIT_CODE);
    assert!(crate::is_managed_heap_limit_error(&error));
}

#[test]
fn repeated_caught_managed_heap_limit_notifies_without_changing_guest_semantics() {
    let program = caught_managed_heap_retry_program();
    let mut context = ManagedHeapNoticeContext::default();
    let limits = Limits {
        max_instructions: 10_000,
        max_heap_bytes: 1_024,
        ..Limits::default()
    };

    let error = program
        .execute_with_context("T", "main", "()I", limits, false, &mut context)
        .unwrap_err();

    assert_eq!(error.code(), "instruction-limit");
    assert_eq!(context.notices.len(), 1);
    let notice = context.notices[0];
    assert_eq!(notice.caught_count, MANAGED_HEAP_NOTICE_CATCHES);
    assert_eq!(notice.heap_limit_bytes, 1_024);
    assert!(notice.heap_bytes <= notice.heap_limit_bytes);
}

#[test]
fn frontend_can_stop_a_caught_heap_retry_for_profile_selection() {
    let program = caught_managed_heap_retry_program();
    let mut context = ManagedHeapNoticeContext {
        decision: ManagedHeapLimitDecision::RequestProfileChange,
        ..ManagedHeapNoticeContext::default()
    };
    let limits = Limits {
        max_instructions: 10_000,
        max_heap_bytes: 1_024,
        ..Limits::default()
    };

    let error = program
        .execute_with_context("T", "main", "()I", limits, false, &mut context)
        .unwrap_err();

    assert_eq!(error.code(), "managed-heap-profile-change-requested");
    assert!(error.message().contains(MANAGED_HEAP_LIMIT_MESSAGE));
    assert_eq!(context.notices.len(), 1);
    assert_eq!(context.notices[0].caught_count, MANAGED_HEAP_NOTICE_CATCHES);
}

#[test]
pub(crate) fn weak_reference_does_not_keep_its_referent_alive() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let referent = machine
        .heap
        .managed
        .allocate_object("Referent", HashMap::new())
        .unwrap();
    let weak = machine
        .heap
        .managed
        .allocate_object("java/lang/ref/WeakReference", HashMap::new())
        .unwrap();
    machine.heap.weak_references.insert(weak, Some(referent));

    machine.collect_heap(vec![weak]);

    assert!(machine.heap.managed.get(weak).is_ok());
    assert!(machine.heap.managed.get(referent).is_err());
    assert_eq!(machine.heap.weak_references.get(&weak), Some(&None));
}
