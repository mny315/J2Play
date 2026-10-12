use super::*;

#[test]
#[ignore = "manual GL array preparation throughput measurement"]
fn jsr239_array_preparation_throughput() {
    for count in [3, 128, 4096] {
        for textured in [false, true] {
            let program = program_with_core_natives();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let pixels = draw_target(&mut machine);
            let mut positions = vec![0.0; count * 2];
            positions[..6].copy_from_slice(&[-1.0, -1.0, 1.0, -1.0, -1.0, 1.0]);
            let vertices = float_buffer(&mut machine, &positions);
            machine.jsr239.vertex_pointer =
                Some(machine.jsr239_float_pointer(vertices, 2, 0).unwrap());
            machine.jsr239.vertex_array_enabled = true;
            machine.jsr239.color = 0xff80_a0c0;
            if textured {
                let coordinates = float_buffer(&mut machine, &positions);
                machine.jsr239.texture_pointer =
                    Some(machine.jsr239_float_pointer(coordinates, 2, 0).unwrap());
                machine.jsr239.texture_array_enabled = true;
            }
            let started = std::time::Instant::now();
            for _ in 0..512 {
                machine
                    .jsr239_draw_arrays(5, 0, std::hint::black_box(count as i32))
                    .unwrap();
                std::hint::black_box(machine.graphics_int_array_mut(pixels).unwrap());
            }
            let elapsed = started.elapsed();
            let checksum = machine
                .graphics_int_array_snapshot(pixels)
                .unwrap()
                .iter()
                .fold(0_u64, |sum, pixel| {
                    sum.wrapping_mul(31)
                        .wrapping_add(u64::from(pixel.cast_unsigned()))
                });
            eprintln!(
                "gl-arrays count={count} coordinates={textured} elapsed={elapsed:?} checksum={checksum}"
            );
        }
    }
}

#[test]
#[ignore = "manual GL framebuffer transfer throughput measurement"]
fn jsr239_framebuffer_transfer_throughput() {
    for (width, height) in [(8, 8), (240, 320)] {
        for coverage in [0.08, 1.0] {
            for textured in [false, true] {
                let program = program_with_core_natives();
                let mut context = DefaultNativeContext;
                let mut machine = program.machine(Limits::default(), false, &mut context);
                draw_target(&mut machine);
                let image = machine.graphics_reference_field(
                    machine.jsr239.target.unwrap(),
                    "javax/microedition/lcdui/Canvas.framebuffer:Ljavax/microedition/lcdui/Image;",
                ).unwrap();
                let pixels = machine
                    .heap
                    .managed
                    .allocate_array(ArrayKind::Int, width * height)
                    .unwrap();
                for (field, value) in [
                    (
                        "javax/microedition/lcdui/Image.width:I",
                        HeapValue::Int(width),
                    ),
                    (
                        "javax/microedition/lcdui/Image.height:I",
                        HeapValue::Int(height),
                    ),
                    (
                        "javax/microedition/lcdui/Image.pixels:[I",
                        HeapValue::Reference(Some(pixels)),
                    ),
                ] {
                    machine.heap.managed.set_field(image, field, value).unwrap();
                }
                let vertices = float_buffer(
                    &mut machine,
                    &[-coverage, -coverage, coverage, -coverage, 0.0, coverage],
                );
                machine.jsr239.vertex_pointer =
                    Some(machine.jsr239_float_pointer(vertices, 2, 0).unwrap());
                machine.jsr239.vertex_array_enabled = true;
                machine.jsr239.color = 0xff80_a0c0;
                if textured {
                    gl_set(&mut machine, "glBindTexture", &[0x0de1, 1]);
                    machine.jsr239.textures.get_mut(&1).unwrap().image =
                        Some(m3g::Texture2DState::new(
                            m3g::Image2DState::from_argb(
                                m3g::ImageFormat::Rgba,
                                1,
                                1,
                                &[0xffc0_a080],
                            )
                            .unwrap(),
                        ));
                    machine.jsr239.texture_enabled = true;
                }
                machine
                    .graphics_int_array_mut(pixels)
                    .unwrap()
                    .fill(HeapValue::Int(0xff12_3456_u32.cast_signed()));
                let started = std::time::Instant::now();
                for _ in 0..512 {
                    machine.jsr239_draw_arrays(5, 0, 3).unwrap();
                    std::hint::black_box(machine.graphics_int_array_mut(pixels).unwrap());
                }
                let elapsed = started.elapsed();
                let checksum = machine
                    .graphics_int_array_snapshot(pixels)
                    .unwrap()
                    .iter()
                    .fold(0_u64, |sum, &pixel| {
                        sum.wrapping_mul(31)
                            .wrapping_add(u64::from(pixel.cast_unsigned()))
                    });
                eprintln!(
                    "gl-framebuffer size={width}x{height} coverage={coverage} textured={textured} elapsed={elapsed:?} checksum={checksum}"
                );
            }
        }
    }
}

#[test]
fn frustum_preserves_projection_across_large_and_small_scene_scales() {
    for scale in [1.0_f32, 1.0e-30, 1.0e30] {
        for (near, far) in [(scale, 2.0 * scale), (2.0 * scale, scale)] {
            let projection = jsr239_frustum(-scale, scale, -scale, scale, near, far).unwrap();
            for (distance, expected_z) in [(near, -1.0), (far, 1.0)] {
                let clip = projection.transform(m3g::Vec4::new(0.0, 0.0, -distance, 1.0));
                assert!(
                    (clip.z / clip.w - expected_z).abs() < 1.0e-5,
                    "near={near}, far={far}"
                );
            }
            assert_eq!(projection.as_array()[0], near / scale);
            assert_eq!(projection.as_array()[5], near / scale);
        }
    }
}

#[test]
fn frustum_accepts_reversed_depth_through_the_gl_api() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = draw_target(&mut machine);
    let vertices = float_buffer(&mut machine, &[-4.0, -4.0, 4.0, -4.0, -4.0, 4.0]);
    machine.jsr239.vertex_pointer = Some(machine.jsr239_float_pointer(vertices, 2, 0).unwrap());
    machine.jsr239.model_view = m3g::Mat4::translation(0.0, 0.0, -4.0).unwrap();
    gl_set(&mut machine, "glEnableClientState", &[0x8074]);
    gl_set(&mut machine, "glMatrixMode", &[0x1701]);
    let mut frames = Vec::new();
    for (near, far) in [(2.0, 8.0), (8.0, 2.0)] {
        gl_set(&mut machine, "glLoadIdentity", &[]);
        let mut arguments = vec![Value::Reference(machine.jsr239.gl)];
        arguments.extend([-near, near, -near, near, near, far].map(Value::Float));
        assert!(matches!(
            jsr239_call(&mut machine, "glFrustumf", "(FFFFFF)V", &arguments).unwrap(),
            CallOutcome::Return(None)
        ));
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0));
        gl_set(&mut machine, "glDrawArrays", &[5, 0, 3]);
        frames.push(machine.graphics_int_array_snapshot(pixels).unwrap());
    }
    assert!(frames[0].iter().any(|pixel| *pixel != 0));
    assert_eq!(frames[0], frames[1]);
    for (near, far) in [(0.0, 1.0), (1.0, 0.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let previous = machine.jsr239.projection;
        let mut arguments = vec![Value::Reference(machine.jsr239.gl)];
        arguments.extend([-1.0, 1.0, -1.0, 1.0, near, far].map(Value::Float));
        assert!(matches!(
            jsr239_call(&mut machine, "glFrustumf", "(FFFFFF)V", &arguments).unwrap(),
            CallOutcome::Throw(_)
        ));
        assert_eq!(machine.jsr239.projection, previous);
    }
}

#[test]
#[ignore = "manual GL projection setup throughput measurement"]
fn frustum_setup_throughput() {
    let started = std::time::Instant::now();
    let mut checksum = 0_u64;
    for index in 0..2_000_000_u32 {
        let near = 1.0 + (index & 255) as f32 / 256.0;
        let [left, right, bottom, top, near, far] =
            std::hint::black_box([-1.0, 1.0, -1.0, 1.0, near, near + 16.0]);
        let projection = jsr239_frustum(left, right, bottom, top, near, far).unwrap();
        checksum = checksum.wrapping_add(u64::from(projection.as_array()[14].to_bits()));
        std::hint::black_box(projection);
    }
    eprintln!(
        "gl-frustum elapsed={:?} checksum={checksum:016x}",
        started.elapsed()
    );
}

fn float_buffer(machine: &mut Machine<'_, '_>, values: &[f32]) -> Handle {
    let bytes = nio_reference_call(
        machine,
        "java/nio/ByteBuffer",
        "allocateDirect",
        "(I)Ljava/nio/ByteBuffer;",
        &[Value::Int(i32::try_from(values.len() * 4).unwrap())],
    );
    let array = machine
        .graphics_reference_field(bytes, "java/nio/Buffer.bytes:[B")
        .unwrap();
    for (index, byte) in (0..).zip(values.iter().flat_map(|value| value.to_ne_bytes())) {
        machine
            .heap
            .managed
            .array_set(array, index, HeapValue::Int(i32::from(byte.cast_signed())))
            .unwrap();
    }
    nio_reference_call(
        machine,
        "java/nio/ByteBuffer",
        "asFloatBuffer",
        "()Ljava/nio/FloatBuffer;",
        &[Value::Reference(Some(bytes))],
    )
}

fn draw_target(machine: &mut Machine<'_, '_>) -> Handle {
    let gl = machine
        .allocate_native_instance("javax/microedition/khronos/opengles/GLImpl", &[])
        .unwrap();
    machine.jsr239.gl = Some(gl);
    let vertices = float_buffer(machine, &[-1.0, -1.0, 1.0, -1.0, -1.0, 1.0]);
    machine.jsr239.vertex_pointer = Some(machine.jsr239_float_pointer(vertices, 2, 0).unwrap());
    jsr239_canvas_target(machine)
}

fn draw_pixel(machine: &mut Machine<'_, '_>, pixels: Handle, background: u32) -> u32 {
    machine
        .graphics_int_array_mut(pixels)
        .unwrap()
        .fill(HeapValue::Int(background.cast_signed()));
    machine.jsr239_draw_arrays(5, 0, 3).unwrap();
    machine.graphics_int_array_snapshot(pixels).unwrap()[12].cast_unsigned()
}

#[test]
fn disabled_vertex_arrays_and_incomplete_strips_generate_no_geometry() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = draw_target(&mut machine);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0);
    gl_set(&mut machine, "glEnableClientState", &[0x8074]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), u32::MAX);
    for count in [0, 1, 2] {
        machine
            .graphics_int_array_mut(pixels)
            .unwrap()
            .fill(HeapValue::Int(0));
        machine.jsr239_draw_arrays(5, 0, count).unwrap();
        assert!(
            machine
                .graphics_int_array_snapshot(pixels)
                .unwrap()
                .iter()
                .all(|&pixel| pixel == 0)
        );
    }
    gl_set(&mut machine, "glDisableClientState", &[0x8074]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0);
}

#[test]
fn triangle_strip_keeps_its_requested_range_and_shared_edge_blending() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = draw_target(&mut machine);
    let vertices = float_buffer(
        &mut machine,
        &[
            99.0, 99.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 99.0, 99.0,
        ],
    );
    machine.jsr239.vertex_pointer = Some(machine.jsr239_float_pointer(vertices, 2, 0).unwrap());
    machine.jsr239.color = 0x8080_8080;
    gl_set(&mut machine, "glEnableClientState", &[0x8074]);
    gl_set(&mut machine, "glEnable", &[0x0be2]);
    gl_set(&mut machine, "glBlendFunc", &[0x0302, 0x0303]);
    machine
        .graphics_int_array_mut(pixels)
        .unwrap()
        .fill(HeapValue::Int(0x4040_4040));
    machine.jsr239_draw_arrays(5, 1, 4).unwrap();
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [0x6060_6060; 16]
    );
}

#[test]
fn invalid_late_vertex_preserves_the_framebuffer() {
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for component in [6, 7] {
            let program = program_with_core_natives();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let pixels = draw_target(&mut machine);
            let mut coordinates = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
            coordinates[component] = invalid;
            let vertices = float_buffer(&mut machine, &coordinates);
            machine.jsr239.vertex_pointer =
                Some(machine.jsr239_float_pointer(vertices, 2, 0).unwrap());
            machine.jsr239.vertex_array_enabled = true;
            machine
                .graphics_int_array_mut(pixels)
                .unwrap()
                .fill(HeapValue::Int(0x1234_5678));
            assert_eq!(
                machine.jsr239_draw_arrays(5, 0, 4).unwrap_err().code(),
                "non-finite-vertex"
            );
            assert_eq!(
                machine.graphics_int_array_snapshot(pixels).unwrap(),
                [0x1234_5678; 16]
            );
        }
    }
}

#[test]
fn cancelled_triangle_strip_preserves_the_framebuffer() {
    let program = program_with_core_natives();
    let checks = Rc::new(Cell::new(0));
    let cancel_at = Rc::new(Cell::new(usize::MAX));
    let mut context = CancellationContext {
        checks: Rc::clone(&checks),
        cancel_at: Rc::clone(&cancel_at),
    };
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = draw_target(&mut machine);
    let mut coordinates = vec![0.0; 2 * 1027];
    coordinates[..8].copy_from_slice(&[-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0]);
    let vertices = float_buffer(&mut machine, &coordinates);
    machine.jsr239.vertex_pointer = Some(machine.jsr239_float_pointer(vertices, 2, 0).unwrap());
    gl_set(&mut machine, "glEnableClientState", &[0x8074]);
    machine
        .graphics_int_array_mut(pixels)
        .unwrap()
        .fill(HeapValue::Int(0x1234_5678));
    checks.set(0);
    // Cancel after the first batch has rendered into the private framebuffer.
    cancel_at.set(3);
    assert_eq!(
        machine.jsr239_draw_arrays(5, 0, 1027).unwrap_err().code(),
        "execution-cancelled"
    );
    assert_eq!(
        machine.graphics_int_array_snapshot(pixels).unwrap(),
        [0x1234_5678; 16]
    );
}

#[test]
fn texturing_and_environment_follow_gl_state_across_texture_bindings() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = draw_target(&mut machine);
    gl_set(&mut machine, "glEnableClientState", &[0x8074]);
    machine.jsr239.color = 0xff00_ff00;
    for (name, color) in [(1, 0xffff_0000), (2, 0xff00_00ff)] {
        gl_set(&mut machine, "glBindTexture", &[0x0de1, name]);
        machine.jsr239.textures.get_mut(&name).unwrap().image = Some(m3g::Texture2DState::new(
            m3g::Image2DState::from_argb(m3g::ImageFormat::Rgba, 1, 1, &[color]).unwrap(),
        ));
    }
    gl_set(&mut machine, "glBindTexture", &[0x0de1, 1]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0xff00_ff00);
    gl_set(&mut machine, "glEnable", &[0x0de1]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0xff00_0000);
    gl_set(&mut machine, "glTexEnvi", &[0x2300, 0x2200, 0x1e01]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0xffff_0000);
    gl_set(&mut machine, "glBindTexture", &[0x0de1, 2]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0xff00_00ff);
    gl_set(&mut machine, "glDisable", &[0x0de1]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0xff00_ff00);
}

#[test]
fn disabled_texture_coordinate_arrays_are_not_read() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = draw_target(&mut machine);
    gl_set(&mut machine, "glEnableClientState", &[0x8074]);
    let coordinates = float_buffer(&mut machine, &[0.0, 0.0]);
    machine.jsr239.texture_pointer = Some(machine.jsr239_float_pointer(coordinates, 2, 0).unwrap());
    assert_eq!(draw_pixel(&mut machine, pixels, 0), u32::MAX);
    gl_set(&mut machine, "glEnableClientState", &[0x8078]);
    assert_eq!(
        machine.jsr239_draw_arrays(5, 0, 3).unwrap_err().code(),
        "illegal-argument-exception"
    );
    gl_set(&mut machine, "glDisableClientState", &[0x8078]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), u32::MAX);
}

#[test]
fn enabling_blending_preserves_the_default_function_until_gl_blend_func_changes_it() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = draw_target(&mut machine);
    gl_set(&mut machine, "glEnableClientState", &[0x8074]);
    machine.jsr239.color = 0x8080_8080;
    gl_set(&mut machine, "glEnable", &[0x0be2]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0x4040_4040), 0x8080_8080);
    gl_set(&mut machine, "glBlendFunc", &[0x0302, 0x0303]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0x4040_4040), 0x6060_6060);
    gl_set(&mut machine, "glDisable", &[0x0be2]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0x4040_4040), 0x8080_8080);
    gl_set(&mut machine, "glEnable", &[0x0be2]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0x4040_4040), 0x6060_6060);
    gl_set(&mut machine, "glBlendFunc", &[1, 0]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0x4040_4040), 0x8080_8080);
}

#[test]
fn unsupported_gl_states_return_java_errors_and_preserve_rendering() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = draw_target(&mut machine);
    gl_set(&mut machine, "glEnableClientState", &[0x8074]);
    for (name, values) in [
        ("glEnable", vec![0x0b71]),
        ("glDisable", vec![-1]),
        ("glEnableClientState", vec![0x8076]),
        ("glTexEnvi", vec![0, 0x2200, 0x2100]),
        ("glTexEnvi", vec![0x2300, 0, 0x2100]),
        ("glTexEnvi", vec![0x2300, 0x2200, -1]),
    ] {
        let outcome = gl_call(&mut machine, name, &values);
        assert!(
            matches!(outcome, CallOutcome::Throw(exception)
            if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException"),
            "{name} {values:?}"
        );
        assert_eq!(draw_pixel(&mut machine, pixels, 0), u32::MAX);
    }
}

#[test]
fn unsupported_texture_upload_parameters_preserve_the_previous_image() {
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
        &[Value::Int(64)],
    );
    machine.jsr239_upload_texture(1, 1, bytes).unwrap();
    let previous = jsr239_texture(&machine, 0).clone();
    for (index, value) in [
        (0, 0),
        (1, 1),
        (2, 0x1907),
        (3, 3),
        (4, 3),
        (5, 1),
        (6, 0x1907),
        (7, 0x1406),
    ] {
        let mut parameters = [0x0de1, 0, 0x1908, 1, 1, 0, 0x1908, 0x1401];
        parameters[index] = value;
        let mut arguments = vec![Value::Reference(Some(gl))];
        arguments.extend(parameters.map(Value::Int));
        arguments.push(Value::Reference(Some(bytes)));
        let outcome = jsr239_call(
            &mut machine,
            "glTexImage2D",
            "(IIIIIIIILjava/nio/Buffer;)V",
            &arguments,
        )
        .unwrap();
        assert!(
            matches!(outcome, CallOutcome::Throw(exception)
            if machine.object_class(exception).unwrap() == "java/lang/IllegalArgumentException"),
            "parameter {index}"
        );
        assert_eq!(jsr239_texture(&machine, 0), &previous);
        assert_eq!(machine.jsr239.texture_bytes, 4);
    }
}

#[test]
fn texture_filters_are_independent_of_order_and_image_uploads() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    draw_target(&mut machine);
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
    for (index, value) in (0..).zip([255, 0, 0, 255, 0, 0, 255, 255]) {
        machine
            .heap
            .managed
            .array_set(storage, index, HeapValue::Int(value))
            .unwrap();
    }
    let mut texture_name = 0;
    for before_upload in [false, true] {
        for reverse in [false, true] {
            for (min, mag) in [(0x2600, 0x2601), (0x2601, 0x2600)] {
                texture_name += 1;
                gl_set(&mut machine, "glBindTexture", &[0x0de1, texture_name]);
                if !before_upload {
                    machine.jsr239_upload_texture(2, 1, bytes).unwrap();
                    assert_eq!(
                        jsr239_texture(&machine, texture_name).shade(0.5, 0.5, u32::MAX),
                        0xff80_0080
                    );
                }
                let mut parameters = [(0x2801, min), (0x2800, mag)];
                if reverse {
                    parameters.reverse();
                }
                for (parameter, value) in parameters {
                    gl_set(&mut machine, "glTexParameteri", &[0x0de1, parameter, value]);
                }
                if before_upload {
                    machine.jsr239_upload_texture(2, 1, bytes).unwrap();
                }
                for upload in [false, true] {
                    if upload {
                        machine.jsr239_upload_texture(2, 1, bytes).unwrap();
                    }
                    for (lod, filter) in [(0.0, mag), (1.0, min)] {
                        let expected = if filter == 0x2601 {
                            0xff80_0080
                        } else {
                            0xff00_00ff
                        };
                        assert_eq!(
                            jsr239_texture(&machine, texture_name).shade_coordinates_lod(
                                [0.5, 0.5, 0.0],
                                u32::MAX,
                                lod
                            ),
                            expected,
                            "min={min:x} mag={mag:x} lod={lod} reverse={reverse} upload={upload}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn incomplete_mipmaps_disable_texturing_until_the_minification_filter_changes() {
    let program = program_with_core_natives();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let pixels = draw_target(&mut machine);
    gl_set(&mut machine, "glEnableClientState", &[0x8074]);
    gl_set(&mut machine, "glEnable", &[0x0de1]);
    gl_set(&mut machine, "glBindTexture", &[0x0de1, 1]);
    gl_set(&mut machine, "glTexEnvi", &[0x2300, 0x2200, 0x1e01]);
    machine.jsr239.color = 0xff00_ff00;
    machine.jsr239.textures.get_mut(&1).unwrap().image = Some(m3g::Texture2DState::new(
        m3g::Image2DState::from_argb(m3g::ImageFormat::Rgba, 2, 1, &[0xffff_0000; 2]).unwrap(),
    ));
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0xff00_ff00);
    gl_set(&mut machine, "glTexParameteri", &[0x0de1, 0x2801, 0x2600]);
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0xffff_0000);
    for filter in 0x2700..=0x2703 {
        gl_set(&mut machine, "glTexParameteri", &[0x0de1, 0x2801, filter]);
        assert_eq!(draw_pixel(&mut machine, pixels, 0), 0xff00_ff00);
    }
    machine.jsr239.textures.get_mut(&1).unwrap().image = Some(m3g::Texture2DState::new(
        m3g::Image2DState::from_argb(m3g::ImageFormat::Rgba, 1, 1, &[0xffff_0000]).unwrap(),
    ));
    assert_eq!(draw_pixel(&mut machine, pixels, 0), 0xffff_0000);
}

#[test]
pub(crate) fn jsr239_color_and_frustum_match_opengl_es_conventions() {
    assert_eq!(jsr239_argb(1.0, 0.5, 0.0, 0.25).unwrap(), 0x40ff_8000);
    assert!(jsr239_argb(f32::NAN, 0.0, 0.0, 1.0).is_err());
    assert_eq!(jsr239_texture_retained_bytes(1, 1), Some(4));
    assert_eq!(jsr239_texture_retained_bytes(4, 2), Some(44));
    assert_eq!(jsr239_texture_retained_bytes(0, 2), None);

    let projection = jsr239_frustum(-1.0, 1.0, -1.0, 1.0, 1.0, 10.0).unwrap();
    let near_center = projection.transform(m3g::Vec4::new(0.0, 0.0, -1.0, 1.0));
    assert_eq!(near_center.x, 0.0);
    assert_eq!(near_center.y, 0.0);
    assert_eq!(near_center.w, 1.0);
    assert!((near_center.z + 1.0).abs() < 1.0e-6);
    assert!(jsr239_frustum(-1.0, 1.0, -1.0, 1.0, 0.0, 10.0).is_err());
}

#[test]
pub(crate) fn jsr239_draw_rejects_hostile_counts_before_allocating_or_presenting() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(
        Limits {
            m3g_vertices: 4,
            ..Limits::default()
        },
        false,
        &mut context,
    );
    let bytes = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Byte, 24)
        .unwrap();
    let buffer = machine
        .heap
        .managed
        .allocate_object(
            "java/nio/FloatBuffer",
            HashMap::from([
                (
                    "java/nio/Buffer.bytes:[B".into(),
                    HeapValue::Reference(Some(bytes)),
                ),
                ("java/nio/Buffer.capacity:I".into(), HeapValue::Int(6)),
                ("java/nio/Buffer.position:I".into(), HeapValue::Int(0)),
                ("java/nio/Buffer.byteOffset:I".into(), HeapValue::Int(0)),
            ]),
        )
        .unwrap();
    machine.jsr239.vertex_pointer = Some(machine.jsr239_float_pointer(buffer, 2, 0).unwrap());
    machine.jsr239.vertex_array_enabled = true;

    assert_eq!(
        machine
            .jsr239_draw_arrays(5, 0, i32::MAX)
            .unwrap_err()
            .code(),
        "resource-limit"
    );
    assert_eq!(
        machine.jsr239_draw_arrays(5, 0, 4).unwrap_err().code(),
        "illegal-argument-exception"
    );
}
