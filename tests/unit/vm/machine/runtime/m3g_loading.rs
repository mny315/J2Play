use super::*;
use std::cell::Cell;

struct ResourceContext {
    resources: HashMap<String, Vec<u8>>,
    reads: Cell<usize>,
    cancel_after: usize,
}

impl HostServices for ResourceContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, name: &str) -> Result<Option<Vec<u8>>, EmuError> {
        self.reads.set(self.reads.get() + 1);
        Ok(self.resources.get(name).cloned())
    }
    fn execution_cancelled(&self) -> bool {
        self.reads.get() >= self.cancel_after
    }
}

fn section(objects: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut payload = Vec::new();
    for (kind, data) in objects {
        payload.push(*kind);
        payload.extend_from_slice(&(data.len() as u32).to_le_bytes());
        payload.extend_from_slice(data);
    }
    let mut bytes = vec![0];
    bytes.extend_from_slice(&(13 + payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in &bytes {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    bytes.extend_from_slice(&((b << 16) | a).to_le_bytes());
    bytes
}

fn group_file(externals: &[&str]) -> Vec<u8> {
    let mut group = vec![0; 12]; // Object3D metadata
    group.extend_from_slice(&[0, 0, 1, 1, 255]); // Transformable and Node flags
    group.extend_from_slice(&u32::MAX.to_le_bytes()); // Scope
    group.push(0); // No alignment
    group.extend_from_slice(&u32::from(!externals.is_empty()).to_le_bytes());
    if !externals.is_empty() {
        group.extend_from_slice(&2_u32.to_le_bytes()); // First external Group
    }
    file(externals, &[(9, group)])
}

fn file(externals: &[&str], objects: &[(u8, Vec<u8>)]) -> Vec<u8> {
    const IDENTIFIER: &[u8] = b"\xabJSR184\xbb\r\n\x1a\n";
    let mut content = Vec::new();
    if !externals.is_empty() {
        content.extend_from_slice(&section(
            &externals
                .iter()
                .map(|uri| {
                    let mut bytes = uri.as_bytes().to_vec();
                    bytes.push(0);
                    (255, bytes)
                })
                .collect::<Vec<_>>(),
        ));
    }
    content.extend_from_slice(&section(objects));
    let mut header = vec![1, 0, u8::from(!externals.is_empty())];
    header.extend_from_slice(&[0; 8]);
    header.extend_from_slice(b"j2play\0");
    let length = IDENTIFIER.len() + section(&[(0, header.clone())]).len() + content.len();
    header[3..7].copy_from_slice(&(length as u32).to_le_bytes());
    let mut bytes = IDENTIFIER.to_vec();
    bytes.extend_from_slice(&section(&[(0, header)]));
    bytes.extend_from_slice(&content);
    m3g::M3gFile::parse(&bytes, m3g::LoaderLimits::default()).unwrap();
    bytes
}

fn chain(length: usize) -> (Vec<u8>, ResourceContext) {
    let mut resources = HashMap::new();
    for index in 1..length {
        let file = if index + 1 == length {
            group_file(&[])
        } else {
            group_file(&[&format!("{}.m3g", index + 1)])
        };
        resources.insert(format!("{index}.m3g"), file);
    }
    (
        group_file(&["1.m3g"]),
        ResourceContext {
            resources,
            reads: Cell::new(0),
            cancel_after: usize::MAX,
        },
    )
}

#[test]
fn m3g_external_load_obeys_depth_and_aggregate_object_budget() {
    let program = program_with_bootstrap();
    for depth_limited in [false, true] {
        let (bytes, mut context) = chain(16);
        let mut limits = Limits::default();
        if depth_limited {
            limits.m3g_graph_depth = 4;
        } else {
            limits.m3g_loader.objects = 8;
        }
        let mut machine = program.machine(limits, false, &mut context);
        let error = machine
            .m3g_load_bytes(&bytes, Some("0.m3g"), &[])
            .unwrap_err();
        assert_eq!(error.code(), "resource-limit");
        assert!(machine.heap.temporary_roots.is_empty());
        drop(machine);
        assert!(context.reads.get() <= 4);
    }
}

#[test]
fn m3g_external_load_observes_cancellation_before_the_next_resource() {
    let program = program_with_bootstrap();
    for cancel_after in [0, 1, 3] {
        let (bytes, mut context) = chain(16);
        context.cancel_after = cancel_after;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let error = machine.m3g_load_bytes(&bytes, None, &[]).unwrap_err();
        assert_eq!(error.code(), "execution-cancelled");
        assert!(machine.heap.temporary_roots.is_empty());
        drop(machine);
        assert_eq!(context.reads.get(), cancel_after);
    }
}

#[test]
fn m3g_loader_accepts_exact_graph_budgets_and_preserves_earlier_roots() {
    let program = program_with_bootstrap();
    let (bytes, context) = chain(2);
    let root = m3g::M3gFile::parse(&bytes, m3g::LoaderLimits::default()).unwrap();
    let child =
        m3g::M3gFile::parse(&context.resources["1.m3g"], m3g::LoaderLimits::default()).unwrap();
    let objects = root.objects.len() + child.objects.len();
    let sections = root.sections.len() + child.sections.len();
    let decompressed = root
        .sections
        .iter()
        .chain(&child.sections)
        .map(|section| section.uncompressed_length as usize)
        .sum::<usize>();
    for reduced in [None, Some(0), Some(1), Some(2)] {
        let (mut bytes, mut context) = chain(2);
        let mut limits = Limits {
            m3g_graph_depth: 2,
            ..Limits::default()
        };
        limits.m3g_loader.objects = objects - usize::from(reduced == Some(0));
        limits.m3g_loader.sections = sections - usize::from(reduced == Some(1));
        limits.m3g_loader.decompressed_bytes = decompressed - usize::from(reduced == Some(2));
        limits.m3g_loader.file_bytes = bytes.len();
        // Loader.load(byte[], offset) accepts bytes after the declared file.
        bytes.extend_from_slice(&[0; 1024]);
        let mut machine = program.machine(limits, false, &mut context);
        let sentinel = machine
            .heap
            .managed
            .allocate_object("Sentinel", HashMap::new())
            .unwrap();
        machine.heap.temporary_roots.push(sentinel);
        let result = machine.m3g_load_bytes(&bytes, Some("0.m3g"), &[]);
        assert_eq!(machine.heap.temporary_roots, [sentinel]);
        if reduced.is_some() {
            assert_eq!(result.unwrap_err().code(), "resource-limit");
            assert_eq!(machine.m3g.runtime.counters().0, 0);
            assert_eq!(machine.m3g.runtime.counters().2, 0);
        } else {
            let array = result.unwrap();
            let HeapValue::Reference(Some(guest)) =
                machine.heap.managed.array_get(array, 0).unwrap()
            else {
                panic!("expected the loaded Group");
            };
            let native = machine.m3g_handle(guest).unwrap();
            assert_eq!(machine.m3g.runtime.children(native).unwrap().len(), 1);
            assert_eq!(machine.m3g.runtime.counters().0, 2);
        }
    }
}

#[test]
fn deep_m3g_resource_chains_load_without_recursive_host_frames() {
    let program = program_with_bootstrap();
    let (bytes, mut context) = chain(256);
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let array = machine.m3g_load_bytes(&bytes, Some("0.m3g"), &[]).unwrap();
    assert!(machine.heap.temporary_roots.is_empty());
    assert_eq!(machine.m3g.runtime.counters().0, 256);
    machine.collect_heap(vec![array]);
    assert_eq!(machine.m3g.runtime.counters().0, 256);
    machine.collect_heap(Vec::new());
    assert_eq!(machine.m3g.runtime.counters().0, 0);
    drop(machine);
    assert_eq!(context.reads.get(), 255);
}

#[test]
fn m3g_reuses_external_resources_and_rolls_back_before_retry() {
    let program = program_with_bootstrap();
    for (second, expected) in [
        ("./1.m3g", None),
        ("missing.m3g", Some("external-reference-missing")),
        ("0.m3g", Some("external-reference-cycle")),
    ] {
        let (_, mut context) = chain(2);
        let bytes = group_file(&["1.m3g", second]);
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let result = machine.m3g_load_bytes(&bytes, Some("0.m3g"), &[]);
        assert!(machine.heap.temporary_roots.is_empty());
        if let Some(code) = expected {
            assert_eq!(result.unwrap_err().code(), code);
            assert_eq!(machine.m3g.runtime.counters().0, 0);
            assert_eq!(machine.m3g.runtime.counters().2, 0);
            assert_eq!(machine.m3g.metrics.loaded_files, 1); // The first child had been created.
            machine
                .m3g_load_bytes(&group_file(&["1.m3g"]), Some("0.m3g"), &[])
                .unwrap();
            assert_eq!(machine.m3g.runtime.counters().0, 2);
        } else {
            result.unwrap();
            assert_eq!(machine.m3g.runtime.counters().0, 2);
            assert_eq!(machine.m3g.metrics.loaded_files, 2);
            drop(machine);
            assert_eq!(context.reads.get(), 1);
        }
    }
}

#[test]
fn png_external_references_share_the_loader_budget_and_cache() {
    let program = program_with_bootstrap();
    let image = graphics::Framebuffer::new(2, 2).unwrap().to_png();
    // Background can retain an Image2D external reference.
    let mut background = vec![0; 12];
    background.extend_from_slice(&0xff00_0000_u32.to_le_bytes());
    background.extend_from_slice(&2_u32.to_le_bytes());
    background.extend_from_slice(&[32, 32]);
    background.extend_from_slice(&[0; 16]);
    background.extend_from_slice(&[1, 1]);
    let bytes = file(&["image.png", "./image.png"], &[(4, background)]);
    let parsed = m3g::M3gFile::parse(&bytes, m3g::LoaderLimits::default()).unwrap();
    let decompressed = parsed
        .sections
        .iter()
        .map(|section| section.uncompressed_length as usize)
        .sum::<usize>()
        + 16;
    for fits in [false, true] {
        let mut context = ResourceContext {
            resources: HashMap::from([("image.png".to_owned(), image.clone())]),
            reads: Cell::new(0),
            cancel_after: usize::MAX,
        };
        let mut limits = Limits::default();
        limits.m3g_loader.objects = parsed.objects.len() + 1;
        limits.m3g_loader.decompressed_bytes = decompressed - usize::from(!fits);
        let mut machine = program.machine(limits, false, &mut context);
        let result = machine.m3g_load_bytes(&bytes, None, &[]);
        if fits {
            let array = result.unwrap();
            machine.collect_heap(vec![array]);
            assert_eq!(machine.m3g.runtime.counters().0, 2);
        } else {
            assert_eq!(result.unwrap_err().code(), "resource-limit");
            assert_eq!(machine.m3g.runtime.counters().0, 0);
        }
        drop(machine);
        assert_eq!(context.reads.get(), 1);
    }
}

#[test]
fn both_loader_overloads_propagate_cancellation_to_the_worker() {
    let program = program_with_bootstrap();
    for descriptor in [
        "([BI)[Ljavax/microedition/m3g/Object3D;",
        "(Ljava/lang/String;)[Ljavax/microedition/m3g/Object3D;",
    ] {
        for cancel_after in [0, 2] {
            let (bytes, mut context) = chain(4);
            context.resources.insert("0.m3g".to_owned(), bytes.clone());
            context.cancel_after = cancel_after;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let arguments = if descriptor.starts_with("([B") {
                let array = machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Byte, bytes.len() as i32)
                    .unwrap();
                for (index, byte) in bytes.iter().enumerate() {
                    machine
                        .heap
                        .managed
                        .array_set(array, index as i32, HeapValue::Int(i32::from(*byte as i8)))
                        .unwrap();
                }
                vec![Value::Reference(Some(array)), Value::Int(0)]
            } else {
                let name = machine.intern_string("0.m3g", &[], &[]).unwrap();
                vec![Value::Reference(Some(name))]
            };
            let error = machine
                .invoke_m3g_object_native(
                    "javax/microedition/m3g/Loader",
                    "load",
                    descriptor,
                    &arguments,
                )
                .err()
                .expect("cancellation must escape Java exception handling");
            assert_eq!(error.code(), "execution-cancelled");
            assert!(machine.heap.temporary_roots.is_empty());
            drop(machine);
            assert_eq!(context.reads.get(), cancel_after);
        }
    }
}

#[test]
fn m3g_loader_applies_sprite_crop_limits_and_rolls_back_rejected_objects() {
    let program = program_with_bootstrap();
    let mut image = vec![0; 12];
    image.extend_from_slice(&[100, 1]); // Mutable RGBA.
    image.extend_from_slice(&8_u32.to_le_bytes());
    image.extend_from_slice(&4_u32.to_le_bytes());
    for width in [-8_i32, 0, 8, i32::MIN] {
        let mut sprite = vec![0; 12];
        sprite.extend_from_slice(&[0, 0, 1, 1, 255]);
        sprite.extend_from_slice(&u32::MAX.to_le_bytes());
        sprite.push(0);
        sprite.extend_from_slice(&2_u32.to_le_bytes());
        sprite.extend_from_slice(&0_u32.to_le_bytes());
        sprite.push(0);
        let crop = [i32::MIN, i32::MAX, width, 4];
        for value in crop {
            sprite.extend_from_slice(&value.to_le_bytes());
        }
        let bytes = file(&[], &[(10, image.clone()), (18, sprite)]);
        for maximum in [7, 8] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(
                Limits {
                    m3g_max_sprite_crop_dimension: maximum,
                    ..Limits::default()
                },
                false,
                &mut context,
            );
            let result = machine.m3g_load_bytes(&bytes, None, &[]);
            assert!(machine.heap.temporary_roots.is_empty());
            if width.unsigned_abs() > maximum {
                assert_eq!(result.unwrap_err().code(), "invalid-crop");
                assert_eq!(machine.m3g.runtime.counters().0, 0);
                assert_eq!(machine.m3g.runtime.counters().2, 0);
            } else {
                let array = result.unwrap();
                let HeapValue::Reference(Some(guest)) =
                    machine.heap.managed.array_get(array, 0).unwrap()
                else {
                    unreachable!()
                };
                let m3g::ObjectKind::Sprite3D(state) = machine
                    .m3g
                    .runtime
                    .kind(machine.m3g_handle(guest).unwrap())
                    .unwrap()
                else {
                    unreachable!()
                };
                assert_eq!(state.crop, crop);
            }
        }
    }
}

#[test]
pub(crate) fn m3g_loader_user_parameters_become_a_guest_hashtable() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let guest = machine
        .heap
        .managed
        .allocate_object("javax/microedition/m3g/Object3D", HashMap::new())
        .unwrap();
    let native = machine
        .m3g
        .runtime
        .create(Some(guest.to_raw()), m3g::ObjectKind::Object)
        .unwrap();

    machine
        .m3g_install_user_parameters(
            native,
            &[(u32::MAX, vec![0, 127, 128, 255]), (7, vec![9])],
            &[],
        )
        .unwrap();

    let table = Handle::from_raw(machine.m3g.runtime.user_object(native).unwrap().unwrap());
    assert_eq!(
        machine
            .graphics_int_field(table, "java/util/Hashtable.count:I")
            .unwrap(),
        2
    );
    let keys = machine
        .graphics_reference_field(table, "java/util/Hashtable.keys:[Ljava/lang/Object;")
        .unwrap();
    let values = machine
        .graphics_reference_field(table, "java/util/Hashtable.values:[Ljava/lang/Object;")
        .unwrap();
    machine.collect_heap(vec![guest]);
    assert!(machine.heap.managed.get(table).is_ok());
    for (index, expected_key, expected_bytes) in
        [(0, -1, &[0, 127, 128, 255][..]), (1, 7, &[9][..])]
    {
        let HeapValue::Reference(Some(key)) = machine.heap.managed.array_get(keys, index).unwrap()
        else {
            panic!("serialized parameter key must be an Integer");
        };
        assert_eq!(
            machine
                .graphics_int_field(key, "java/lang/Integer.value:I")
                .unwrap(),
            expected_key
        );
        let HeapValue::Reference(Some(value)) =
            machine.heap.managed.array_get(values, index).unwrap()
        else {
            panic!("serialized parameter value must be a byte array");
        };
        assert_eq!(machine.m3g_byte_array(value).unwrap(), expected_bytes);
    }
}

#[test]
fn loader_offsets_reject_negative_and_end_positions_before_decoding() {
    let program = program_with_bootstrap();
    for (length, offset, expected) in [
        (0, 0, "java/lang/IndexOutOfBoundsException"),
        (2, -1, "java/lang/IndexOutOfBoundsException"),
        (2, 2, "java/lang/IndexOutOfBoundsException"),
        (2, i32::MAX, "java/lang/IndexOutOfBoundsException"),
        (2, 0, "java/io/IOException"),
        (2, 1, "java/io/IOException"),
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Byte, length)
            .unwrap();
        let outcome = machine.invoke_m3g_object_native(
            "javax/microedition/m3g/Loader",
            "load",
            "([BI)[Ljavax/microedition/m3g/Object3D;",
            &[Value::Reference(Some(array)), Value::Int(offset)],
        );
        match outcome {
            Ok(CallOutcome::Throw(exception)) => {
                assert_eq!(machine.object_class(exception).unwrap(), expected)
            }
            Err(error) => assert_eq!(java_error_class(&error), Some(expected)),
            _ => panic!("invalid input must throw"),
        }
        assert_eq!(machine.m3g.runtime.counters().0, 0);
        assert!(machine.heap.temporary_roots.is_empty());
    }
}

#[test]
fn loader_byte_array_ignores_bytes_before_the_offset_and_after_the_file() {
    let program = program_with_bootstrap();
    let file = group_file(&[]);
    for prefix_length in [0, 65536] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let array = machine
            .heap
            .managed
            .allocate_array(ArrayKind::Byte, prefix_length + file.len() as i32 + 2048)
            .unwrap();
        for (index, byte) in file.iter().enumerate() {
            machine
                .heap
                .managed
                .array_set(
                    array,
                    prefix_length + index as i32,
                    HeapValue::Int(i32::from(*byte as i8)),
                )
                .unwrap();
        }
        let result = machine
            .invoke_m3g_object_native(
                "javax/microedition/m3g/Loader",
                "load",
                "([BI)[Ljavax/microedition/m3g/Object3D;",
                &[Value::Reference(Some(array)), Value::Int(prefix_length)],
            )
            .unwrap();
        let CallOutcome::Return(Some(Value::Reference(Some(roots)))) = result else {
            panic!("expected loaded roots");
        };
        assert_eq!(machine.heap.managed.array_length(roots).unwrap(), 1);
        let HeapValue::Reference(Some(guest)) = machine.heap.managed.array_get(roots, 0).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(
            machine.object_class(guest).unwrap(),
            "javax/microedition/m3g/Group"
        );
    }
}
