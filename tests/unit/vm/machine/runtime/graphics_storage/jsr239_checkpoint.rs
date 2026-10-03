//! Saved GL resources are validated before the restored VM is published.

use super::*;

const GL_CLASS: &str = "javax/microedition/khronos/opengles/GLImpl";

fn populate(machine: &mut Machine<'_, '_>) -> (Handle, Handle) {
    let gl = machine.allocate_native_instance(GL_CLASS, &[]).unwrap();
    machine.jsr239.gl = Some(gl);
    let bytes = nio_reference_call(
        machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(17)],
    );
    machine.jsr239_upload_texture(2, 2, bytes).unwrap();
    machine
        .heap
        .managed
        .set_field(bytes, "java/nio/Buffer.position:I", HeapValue::Int(1))
        .unwrap();
    let view = nio_reference_call(
        machine,
        "java/nio/ByteBuffer",
        "asFloatBuffer",
        "()Ljava/nio/FloatBuffer;",
        &[Value::Reference(Some(bytes))],
    );
    let backing = machine
        .graphics_reference_field(bytes, "java/nio/Buffer.bytes:[B")
        .unwrap();
    for (index, byte) in [1.5_f32, -2.25, 3.25]
        .into_iter()
        .flat_map(f32::to_ne_bytes)
        .enumerate()
    {
        machine
            .heap
            .managed
            .array_set(
                backing,
                i32::try_from(index + 5).unwrap(),
                HeapValue::Int(i32::from(byte.cast_signed())),
            )
            .unwrap();
    }
    machine
        .heap
        .managed
        .set_field(view, "java/nio/Buffer.position:I", HeapValue::Int(1))
        .unwrap();
    machine.jsr239.vertex_pointer = Some(machine.jsr239_float_pointer(view, 2, 0).unwrap());
    // The view has an unaligned byte offset and moves after GL captures its range.
    machine
        .heap
        .managed
        .set_field(view, "java/nio/Buffer.position:I", HeapValue::Int(3))
        .unwrap();
    (gl, bytes)
}

#[test]
fn jsr239_checkpoint_restores_texture_and_captured_unaligned_view() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let (gl, _) = populate(&mut machine);
    let bytes = machine.encode_checkpoint(gl).unwrap();
    let mut context = DefaultNativeContext;
    let mut restored = program.machine(Limits::default(), false, &mut context);
    assert_eq!(restored.restore_checkpoint(GL_CLASS, &bytes).unwrap(), gl);
    assert_eq!(jsr239_texture(&restored, 0).dimensions(), (2, 2));
    assert_eq!(restored.jsr239.texture_bytes, machine.jsr239.texture_bytes);
    let pointer = restored.jsr239.vertex_pointer.as_ref().unwrap();
    assert_eq!(pointer.bytes, 5..17);
    assert_eq!(
        nio_float_words(restored.jsr239_float_pointer_words(pointer, 0..3).unwrap()).unwrap(),
        [1.5, -2.25, 3.25]
    );
}

#[test]
fn jsr239_checkpoint_rejects_invalid_resources_without_replacing_the_live_vm() {
    let program = program_with_core_natives();
    let mut limits = Limits::default();
    limits.m3g_arena.objects = 2;
    limits.m3g_texture_pixels = 4;
    for invalid in [
        "matrix mode",
        "matrix stack",
        "texture accounting",
        "texture count",
        "texture dimensions",
        "texture pixel limit",
        "bound texture",
        "pointer end",
        "pointer alignment",
        "pointer class",
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(limits.clone(), false, &mut context);
        let (gl, byte_buffer) = populate(&mut machine);
        match invalid {
            "matrix mode" => machine.jsr239.matrix_mode = 0,
            "matrix stack" => machine.jsr239.model_view_stack = vec![m3g::Mat4::IDENTITY; 65],
            "texture accounting" => machine.jsr239.texture_bytes += 1,
            "texture count" => {
                machine.jsr239.textures.entry(1).or_default();
                machine.jsr239.textures.entry(2).or_default();
            }
            "texture dimensions" | "texture pixel limit" => {
                let (width, height) = if invalid == "texture dimensions" {
                    (3, 1)
                } else {
                    (4, 2)
                };
                let image =
                    m3g::Image2DState::mutable(m3g::ImageFormat::Rgba, width, height).unwrap();
                machine.jsr239.textures.get_mut(&0).unwrap().image =
                    Some(m3g::Texture2DState::new(image));
                machine.jsr239.texture_bytes =
                    jsr239_texture_retained_bytes(width, height).unwrap();
            }
            "bound texture" => machine.jsr239.bound_texture = 42,
            "pointer end" => machine.jsr239.vertex_pointer.as_mut().unwrap().bytes.end = 21,
            "pointer alignment" => machine.jsr239.vertex_pointer.as_mut().unwrap().bytes.start = 2,
            "pointer class" => machine.jsr239.vertex_pointer.as_mut().unwrap().buffer = byte_buffer,
            _ => unreachable!(),
        }
        let bytes = machine.encode_checkpoint(gl).unwrap();
        let mut context = DefaultNativeContext;
        let mut restored = program.machine(limits.clone(), false, &mut context);
        restored.jsr239.color = 0xff12_3456;
        restored.device.random_seed_sequence = 73;
        let error = restored
            .restore_checkpoint(GL_CLASS, &bytes)
            .expect_err(invalid);
        assert_eq!(error.code(), "checkpoint-jsr239", "{invalid}");
        assert_eq!(restored.jsr239.color, 0xff12_3456, "{invalid}");
        assert_eq!(restored.device.random_seed_sequence, 73, "{invalid}");
        assert!(restored.jsr239.textures.is_empty(), "{invalid}");
    }
}
