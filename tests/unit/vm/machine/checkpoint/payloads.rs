use super::*;
use crate::machine::{Arc, ArrayKind, Handle, ImmutableImagePixels, Limits, Machine};

fn populate(machine: &mut Machine<'_, '_>) -> [Handle; 4] {
    let instance = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, 0)
        .unwrap();
    machine
        .store_immutable_image_pixels(
            pixels,
            Arc::new(ImmutableImagePixels::from_argb(vec![0x0012_3456; 32])),
            &[],
            &[],
        )
        .unwrap();
    let string = machine.allocate_dynamic_string("Text", &[], &[]).unwrap();
    let canonical = machine.intern_string("Canonical", &[], &[]).unwrap();
    [instance, pixels, string, canonical]
}

#[test]
fn checkpoint_rejects_invalid_native_payloads_before_replacing_the_vm() {
    let program = program();
    for invalid in [
        "palette index",
        "palette size",
        "pixel handle",
        "pixel kind",
        "pixel accounting",
        "pixel backing length",
        "string handle",
        "string kind",
        "string accounting",
        "interned contents",
        "interned accounting",
        "weak owner",
        "weak referent",
        "trace owner",
        "trace kind",
        "trace accounting",
        "OOM owner",
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let [instance, pixels, string, canonical] = populate(&mut machine);
        match invalid {
            "palette index" | "palette size" => {
                let (palette, indices) = if invalid == "palette index" {
                    (vec![0], vec![1])
                } else {
                    (vec![0; 257], vec![0])
                };
                machine
                    .store_immutable_image_pixels(
                        pixels,
                        Arc::new(ImmutableImagePixels::Indexed8 {
                            palette: palette.into(),
                            indices: indices.into(),
                        }),
                        &[],
                        &[],
                    )
                    .unwrap();
            }
            "pixel handle" | "pixel kind" => {
                let payload = machine.heap.immutable_image_pixels.remove(&pixels).unwrap();
                let invalid_handle = if invalid == "pixel handle" {
                    Handle::from_raw(u64::MAX)
                } else {
                    instance
                };
                machine
                    .heap
                    .immutable_image_pixels
                    .insert(invalid_handle, payload);
            }
            "pixel accounting" => machine.heap.managed.set_external_bytes(pixels, 0).unwrap(),
            "pixel backing length" => {
                let array = machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Int, 1)
                    .unwrap();
                machine
                    .store_immutable_image_pixels(
                        array,
                        Arc::clone(&machine.heap.immutable_image_pixels[&pixels]),
                        &[],
                        &[],
                    )
                    .unwrap();
            }
            "string handle" | "string kind" => {
                let invalid_handle = if invalid == "string handle" {
                    Handle::from_raw(u64::MAX)
                } else {
                    pixels
                };
                machine.heap.string_values.insert(invalid_handle, vec![65]);
            }
            "string accounting" => machine.heap.managed.set_external_bytes(string, 0).unwrap(),
            "interned contents" => {
                machine.heap.interned_strings.insert(vec![65], canonical);
            }
            "interned accounting" => machine
                .heap
                .managed
                .set_external_bytes(canonical, 18)
                .unwrap(),
            "weak owner" => {
                machine
                    .heap
                    .weak_references
                    .insert(Handle::from_raw(u64::MAX), None);
            }
            "weak referent" => {
                machine
                    .heap
                    .weak_references
                    .insert(instance, Some(Handle::from_raw(u64::MAX)));
            }
            "trace owner" => {
                machine
                    .heap
                    .throwable_traces
                    .insert(Handle::from_raw(u64::MAX), vec![]);
            }
            "trace kind" => {
                machine.heap.throwable_traces.insert(instance, vec![]);
            }
            "trace accounting" => {
                let throwable = machine
                    .heap
                    .managed
                    .allocate_object("java/lang/Throwable", HashMap::new())
                    .unwrap();
                let method = program.methods.values().next().unwrap();
                machine
                    .record_exception_frame(throwable, method, 1, &[], &[])
                    .unwrap();
                machine
                    .heap
                    .managed
                    .set_external_bytes(throwable, 0)
                    .unwrap();
            }
            "OOM owner" => {
                machine
                    .heap
                    .managed_heap_limit_throwables
                    .insert(Handle::from_raw(u64::MAX));
            }
            _ => unreachable!(),
        }
        let bytes = machine.encode_checkpoint(instance).unwrap();
        let mut context = DefaultNativeContext;
        let mut restored = program.machine(Limits::default(), false, &mut context);
        restored.device.random_seed_sequence = 73;
        assert_eq!(
            restored
                .restore_checkpoint("Checkpoint", &bytes)
                .expect_err(invalid)
                .code(),
            "checkpoint-heap-payload",
            "{invalid}"
        );
        assert_eq!(restored.device.random_seed_sequence, 73, "{invalid}");
        assert!(restored.heap.managed.is_empty(), "{invalid}");
    }
}

#[test]
fn checkpoint_restores_indexed_and_direct_pixels_and_both_string_storage_forms() {
    let program = program();
    for indexed in [false, true] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let [instance, pixels, string, canonical] = populate(&mut machine);
        machine.heap.weak_references.insert(instance, Some(string));
        machine.heap.weak_references.insert(string, None);
        let throwable = machine
            .heap
            .managed
            .allocate_object("java/lang/Throwable", HashMap::new())
            .unwrap();
        let method = program.methods.values().next().unwrap();
        machine
            .record_exception_frame(throwable, method, 1, &[], &[])
            .unwrap();
        machine.heap.managed_heap_limit_throwables.insert(instance);
        if !indexed {
            machine
                .store_immutable_image_pixels(
                    pixels,
                    Arc::new(ImmutableImagePixels::Argb(vec![0x0012_3456; 32].into())),
                    &[],
                    &[],
                )
                .unwrap();
        }
        let bytes = machine.encode_checkpoint(instance).unwrap();
        let mut context = DefaultNativeContext;
        let mut restored = program.machine(Limits::default(), false, &mut context);
        restored.restore_checkpoint("Checkpoint", &bytes).unwrap();
        assert_eq!(
            restored.graphics_int_array_snapshot(pixels).unwrap(),
            [0x0012_3456; 32]
        );
        assert_eq!(
            restored.heap.string_values.get(&string).unwrap(),
            &"Text".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(
            restored.intern_string("Canonical", &[], &[]).unwrap(),
            canonical
        );
        assert_eq!(restored.heap.managed.bytes(), machine.heap.managed.bytes());
        assert_eq!(restored.heap.weak_references, machine.heap.weak_references);
        assert_eq!(
            restored.heap.throwable_traces,
            machine.heap.throwable_traces
        );
        assert_eq!(
            restored.heap.managed_heap_limit_throwables,
            machine.heap.managed_heap_limit_throwables
        );
    }
}
