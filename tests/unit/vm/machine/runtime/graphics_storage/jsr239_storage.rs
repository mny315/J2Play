//! Texture parameters persist across uploads and failed updates.

use super::*;

mod names;
mod pointers;

#[test]
fn jsr239_texture_wrapping_is_independent_and_survives_image_uploads() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let gl = machine
        .allocate_native_instance("javax/microedition/khronos/opengles/GLImpl", &[])
        .unwrap();
    machine.jsr239.gl = Some(gl);
    let bytes = nio_reference_call(
        &mut machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(16)],
    );
    let storage = machine
        .graphics_reference_field(bytes, "java/nio/Buffer.bytes:[B")
        .unwrap();
    for (index, value) in [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ]
    .into_iter()
    .enumerate()
    {
        machine
            .heap
            .managed
            .array_set(
                storage,
                i32::try_from(index).unwrap(),
                HeapValue::Int(value),
            )
            .unwrap();
    }
    let mut expected_textures = Vec::new();
    for before_upload in [false, true] {
        for reverse in [false, true] {
            for (wrap_s, wrap_t, color) in [
                (0x2901, 0x2901, 0xffff_0000),
                (0x812f, 0x2901, 0xff00_ff00),
                (0x2901, 0x812f, 0xff00_00ff),
                (0x812f, 0x812f, 0xffff_ffff),
            ] {
                let name = i32::try_from(expected_textures.len() + 1).unwrap();
                gl_set(&mut machine, "glBindTexture", &[0x0de1, name]);
                if !before_upload {
                    machine.jsr239_upload_texture(2, 2, bytes).unwrap();
                }
                let mut parameters = [(0x2802, wrap_s), (0x2803, wrap_t)];
                if reverse {
                    parameters.reverse();
                }
                for (parameter, value) in parameters {
                    gl_set(&mut machine, "glTexParameteri", &[0x0de1, parameter, value]);
                }
                if before_upload {
                    machine.jsr239_upload_texture(2, 2, bytes).unwrap();
                }
                assert_eq!(
                    jsr239_texture(&machine, name).shade(1.25, 1.25, u32::MAX),
                    color
                );
                machine.jsr239_upload_texture(2, 2, bytes).unwrap();
                assert_eq!(
                    jsr239_texture(&machine, name).shade(1.25, 1.25, u32::MAX),
                    color
                );
                expected_textures.push((name, color));
            }
        }
    }
    for (name, color) in expected_textures {
        assert_eq!(
            jsr239_texture(&machine, name).shade(1.25, 1.25, u32::MAX),
            color
        );
    }
    gl_set(&mut machine, "glBindTexture", &[0x0de1, 0]);
    machine.jsr239_upload_texture(2, 2, bytes).unwrap();
    assert_eq!(
        jsr239_texture(&machine, 0).shade(1.25, 1.25, u32::MAX),
        0xffff_0000
    );
}

#[test]
fn jsr239_texture_parameter_errors_preserve_the_image_and_reserved_names() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut limits = Limits::default();
    limits.m3g_arena.objects = 1;
    let mut machine = program.machine(limits, false, &mut context);
    let gl = machine
        .allocate_native_instance("javax/microedition/khronos/opengles/GLImpl", &[])
        .unwrap();
    machine.jsr239.gl = Some(gl);
    for uploaded in [false, true] {
        if uploaded {
            for parameter in [0x2800, 0x2801] {
                gl_set(
                    &mut machine,
                    "glTexParameteri",
                    &[0x0de1, parameter, 0x2601],
                );
            }
            let bytes = nio_reference_call(
                &mut machine,
                "java/nio/ByteBuffer",
                "allocateDirect",
                "(I)Ljava/nio/ByteBuffer;",
                &[Value::Int(8)],
            );
            let storage = machine
                .graphics_reference_field(bytes, "java/nio/Buffer.bytes:[B")
                .unwrap();
            for index in 3..8 {
                machine
                    .heap
                    .managed
                    .array_set(storage, index, HeapValue::Int(255))
                    .unwrap();
            }
            for _ in 0..2 {
                machine.jsr239_upload_texture(2, 1, bytes).unwrap();
                assert_eq!(
                    jsr239_texture(&machine, 0).shade(0.5, 0.5, u32::MAX),
                    0xff80_8080
                );
            }
        }
        let previous = machine
            .jsr239
            .textures
            .get(&0)
            .and_then(|texture| texture.image.clone());
        let count = machine.jsr239.textures.len();
        let retained = machine.jsr239.texture_bytes;
        for (target, parameter, value) in [
            (0, 0x2802, 0x2901),
            (0x0de1, 0, 0x2901),
            (0x0de1, 0x2802, 0),
            (0x0de1, 0x2803, 0),
            (0x0de1, 0x2800, 0),
            (0x0de1, 0x2801, 0),
        ] {
            let outcome = jsr239_call(
                &mut machine,
                "glTexParameteri",
                "(III)V",
                &[
                    Value::Reference(Some(gl)),
                    Value::Int(target),
                    Value::Int(parameter),
                    Value::Int(value),
                ],
            )
            .unwrap();
            assert!(
                matches!(outcome, CallOutcome::Throw(exception) if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException")
            );
            assert_eq!(machine.jsr239.textures.len(), count);
            assert_eq!(machine.jsr239.texture_bytes, retained);
            assert_eq!(
                machine
                    .jsr239
                    .textures
                    .get(&0)
                    .and_then(|texture| texture.image.as_ref()),
                previous.as_ref()
            );
        }
    }
}
