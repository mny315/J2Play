use super::*;

#[test]
fn ambient_and_emissive_colors_ignore_finite_normal_and_view_directions() {
    let ambient = [LightSource::Ambient {
        color: 0x00ff_ffff,
        intensity: 0.5,
    }];
    for lights in [&[][..], &ambient[..]] {
        for specular in [0, 0x00ff_ffff] {
            let lighting = PreparedLighting::new(
                MaterialState {
                    ambient: 0x0040_6080,
                    diffuse: 0x4011_2233,
                    emissive: 0x0001_0203,
                    specular,
                    shininess: 16.0,
                },
                lights,
            )
            .unwrap();
            for tracking in [false, true] {
                let expected = match (lights.is_empty(), tracking) {
                    (true, false) => 0x4001_0203,
                    (true, true) => 0x8001_0203,
                    (false, false) => 0x4021_3243,
                    (false, true) => 0x8021_120b,
                };
                for scale in [0.0, f32::from_bits(1), 1.0, f32::MAX] {
                    assert_eq!(
                        lighting
                            .shade(
                                0x8040_2010,
                                tracking,
                                Vec3::new(scale, -scale, scale),
                                Vec3::default(),
                                Vec3::new(-scale, scale, -scale),
                            )
                            .unwrap(),
                        expected,
                    );
                }
                for invalid in [f32::NAN, f32::NEG_INFINITY, f32::INFINITY] {
                    for vector in [
                        Vec3::new(invalid, 0.0, 0.0),
                        Vec3::new(0.0, invalid, 0.0),
                        Vec3::new(0.0, 0.0, invalid),
                    ] {
                        for (normal, view) in [(vector, Vec3::default()), (Vec3::default(), vector)]
                        {
                            assert_eq!(
                                lighting
                                    .shade(0x8040_2010, tracking, normal, Vec3::default(), view)
                                    .unwrap_err()
                                    .code(),
                                "non-finite",
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn diffuse_lighting_ignores_view_direction_but_rejects_non_finite_vectors() {
    let lighting = PreparedLighting::new(
        MaterialState::default(),
        &[LightSource::Directional {
            direction: Vec3::new(0.0, 0.0, 1.0),
            color: 0x00ff_ffff,
            intensity: 1.0,
        }],
    )
    .unwrap();
    for scale in [0.0, f32::from_bits(1), 1.0, f32::MAX] {
        for direction in [-1.0, 1.0] {
            let color = lighting
                .shade(
                    u32::MAX,
                    false,
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec3::default(),
                    Vec3::new(scale, 0.0, scale * direction),
                )
                .unwrap();
            assert_eq!(color, 0xffcc_cccc);
        }
    }
    for invalid in [f32::NAN, f32::NEG_INFINITY, f32::INFINITY] {
        for view in [
            Vec3::new(invalid, 0.0, 0.0),
            Vec3::new(0.0, invalid, 0.0),
            Vec3::new(0.0, 0.0, invalid),
        ] {
            assert_eq!(
                lighting
                    .shade(
                        u32::MAX,
                        false,
                        Vec3::new(0.0, 0.0, 1.0),
                        Vec3::default(),
                        view,
                    )
                    .unwrap_err()
                    .code(),
                "non-finite",
            );
        }
    }
}

#[test]
fn prepared_lighting_rejects_invalid_constant_state_before_batch_execution() {
    let directional = LightSource::Directional {
        direction: Vec3::new(0.0, 0.0, 1.0),
        color: u32::MAX,
        intensity: 1.0,
    };
    assert!(
        PreparedLighting::new(MaterialState::default(), &[directional; crate::MAX_LIGHTS]).is_ok()
    );
    assert!(
        PreparedLighting::new(
            MaterialState::default(),
            &[directional; crate::MAX_LIGHTS + 1]
        )
        .is_err()
    );
    for light in [
        LightSource::Ambient {
            color: 0,
            intensity: f32::NAN,
        },
        LightSource::Directional {
            direction: Vec3::default(),
            color: 0,
            intensity: 1.0,
        },
        LightSource::Omni {
            position: Vec3::default(),
            color: 0,
            intensity: 1.0,
            attenuation: [-1.0, 0.0, 0.0],
        },
        LightSource::Omni {
            position: Vec3::default(),
            color: 0,
            intensity: 1.0,
            attenuation: [1.0, f32::INFINITY, 0.0],
        },
        LightSource::Spot {
            position: Vec3::default(),
            direction: Vec3::new(0.0, 0.0, 1.0),
            color: 0,
            intensity: 1.0,
            attenuation: [1.0, 0.0, 0.0],
            angle_degrees: 91.0,
            exponent: 0.0,
        },
        LightSource::Spot {
            position: Vec3::default(),
            direction: Vec3::new(0.0, 0.0, 1.0),
            color: 0,
            intensity: 1.0,
            attenuation: [1.0, 0.0, 0.0],
            angle_degrees: 45.0,
            exponent: f32::NAN,
        },
    ] {
        assert!(PreparedLighting::new(MaterialState::default(), &[light]).is_err());
    }
}

#[test]
fn negative_light_intensity_is_subtractive() {
    let color = shade_lit_vertex(
        MaterialState {
            ambient: 0,
            diffuse: 0xffff_ffff,
            emissive: 0x0080_8080,
            specular: 0,
            shininess: 0.0,
        },
        u32::MAX,
        false,
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::default(),
        Vec3::new(0.0, 0.0, 1.0),
        &[LightSource::Directional {
            direction: Vec3::new(0.0, 0.0, 1.0),
            color: 0x00ff_ffff,
            intensity: -0.25,
        }],
    )
    .unwrap();
    assert_eq!(color, 0xff40_4040);
}

#[test]
fn positional_spot_and_vertex_color_tracking_change_lighting() {
    let material = MaterialState {
        ambient: 0,
        diffuse: 0xffff_ffff,
        emissive: 0,
        specular: 0,
        shininess: 0.0,
    };
    let omni = shade_lit_vertex(
        material,
        0xffff_0000,
        true,
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        &[LightSource::Omni {
            position: Vec3::new(0.0, 0.0, 1.0),
            color: 0x00ff_ffff,
            intensity: 1.0,
            attenuation: [1.0, 0.0, 0.0],
        }],
    )
    .unwrap();
    assert_eq!(omni, 0xffff_0000);
    let outside_spot = shade_lit_vertex(
        material,
        0xffff_ffff,
        false,
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        &[LightSource::Spot {
            position: Vec3::new(0.0, 0.0, 1.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
            color: 0x00ff_ffff,
            intensity: 1.0,
            attenuation: [1.0, 0.0, 0.0],
            angle_degrees: 10.0,
            exponent: 1.0,
        }],
    )
    .unwrap();
    assert_eq!(outside_spot, 0xff00_0000);
}

#[test]
fn degenerate_lighting_vectors_do_not_abort_a_frame() {
    let material = MaterialState {
        ambient: 0,
        diffuse: 0xffff_ffff,
        emissive: 0xff12_3456,
        specular: 0,
        shininess: 0.0,
    };
    let color = shade_lit_vertex(
        material,
        0xffff_ffff,
        false,
        Vec3::default(),
        Vec3::default(),
        Vec3::default(),
        &[LightSource::Omni {
            position: Vec3::default(),
            color: 0x00ff_ffff,
            intensity: 1.0,
            attenuation: [1.0, 0.0, 0.0],
        }],
    )
    .unwrap();
    assert_eq!(color, 0xff12_3456);
}

#[test]
fn default_material_uses_the_jsr_diffuse_color() {
    let color = shade_lit_vertex(
        MaterialState::default(),
        u32::MAX,
        false,
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::default(),
        Vec3::new(0.0, 0.0, 1.0),
        &[LightSource::Directional {
            direction: Vec3::new(0.0, 0.0, 1.0),
            color: 0x00ff_ffff,
            intensity: 1.0,
        }],
    )
    .unwrap();
    assert_eq!(color, 0xffcc_cccc);
}

#[test]
fn vertex_color_tracking_replaces_ambient_and_diffuse_together() {
    let material = MaterialState {
        ambient: 0x0000_00ff,
        diffuse: 0xffff_0000,
        ..MaterialState::default()
    };
    let lights = [
        LightSource::Ambient {
            color: 0x00ff_ffff,
            intensity: 0.5,
        },
        LightSource::Directional {
            direction: Vec3::new(0.0, 0.0, 1.0),
            color: 0x00ff_ffff,
            intensity: 0.5,
        },
    ];
    for (tracking, expected) in [(false, 0xff80_0080), (true, 0x8060_4020)] {
        let color = shade_lit_vertex(
            material,
            0x8060_4020,
            tracking,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::default(),
            Vec3::new(0.0, 0.0, 1.0),
            &lights,
        )
        .unwrap();
        assert_eq!(color, expected, "tracking={tracking}");
    }
}

#[test]
fn specular_highlights_include_zero_shininess_and_require_front_lighting() {
    for shininess in [0.0, 16.0] {
        let material = MaterialState {
            ambient: 0,
            diffuse: 0xff00_0000,
            specular: 0x00ff_ffff,
            shininess,
            ..MaterialState::default()
        };
        for (direction, expected) in [
            (Vec3::new(0.0, 0.0, 1.0), 0xffff_ffff),
            (Vec3::new(1.0, 0.0, -1.0), 0xff00_0000),
            (Vec3::new(1.0, 0.0, 0.0), 0xff00_0000),
        ] {
            let color = shade_lit_vertex(
                material,
                u32::MAX,
                false,
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::default(),
                Vec3::new(0.0, 0.0, 1.0),
                &[LightSource::Directional {
                    direction,
                    color: 0x00ff_ffff,
                    intensity: 1.0,
                }],
            )
            .unwrap();
            assert_eq!(
                color, expected,
                "shininess={shininess} direction={direction:?}"
            );
        }
    }
}

#[test]
fn lighting_rejects_invalid_shininess_even_without_active_lights() {
    for shininess in [-1.0, 129.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            shade_lit_vertex(
                MaterialState {
                    shininess,
                    ..MaterialState::default()
                },
                u32::MAX,
                false,
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::default(),
                Vec3::new(0.0, 0.0, 1.0),
                &[],
            )
            .unwrap_err()
            .code(),
            "invalid-lighting"
        );
    }
}

#[test]
fn tiny_nonzero_normals_keep_the_same_lighting_direction() {
    let render = |scale| {
        shade_lit_vertex(
            MaterialState::default(),
            u32::MAX,
            false,
            Vec3::new(0.0, 0.0, scale),
            Vec3::default(),
            Vec3::new(0.0, 0.0, scale),
            &[LightSource::Directional {
                direction: Vec3::new(0.0, 0.0, 1.0),
                color: 0x00ff_ffff,
                intensity: 1.0,
            }],
        )
        .unwrap()
    };
    let expected = render(1.0);
    for scale in [1.0e-10, 1.0e-20, 1.0e-30, f32::from_bits(1)] {
        assert_eq!(render(scale), expected, "scale={scale}");
    }
}

fn shade_positional_light(
    spot: bool,
    position: Vec3,
    intensity: f32,
    attenuation: [f32; 3],
) -> u32 {
    let light = if spot {
        LightSource::Spot {
            position,
            direction: Vec3::new(0.0, 0.0, -1.0),
            color: 0x00ff_ffff,
            intensity,
            attenuation,
            angle_degrees: 45.0,
            exponent: 1.0,
        }
    } else {
        LightSource::Omni {
            position,
            color: 0x00ff_ffff,
            intensity,
            attenuation,
        }
    };
    shade_lit_vertex(
        MaterialState::default(),
        u32::MAX,
        false,
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::default(),
        Vec3::new(0.0, 0.0, 1.0),
        &[light],
    )
    .unwrap()
}

#[test]
fn scaling_light_intensity_and_attenuation_together_preserves_brightness() {
    for spot in [false, true] {
        for coefficients in [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ] {
            let expected =
                shade_positional_light(spot, Vec3::new(0.0, 0.0, 1.0), 0.5, coefficients);
            assert_ne!(expected, 0xff00_0000);
            for scale in [2.0_f32.powi(-60), 2.0_f32.powi(60)] {
                assert_eq!(
                    shade_positional_light(
                        spot,
                        Vec3::new(0.0, 0.0, 1.0),
                        0.5 * scale,
                        coefficients.map(|value| value * scale)
                    ),
                    expected
                );
            }
        }
    }
}

#[test]
fn positional_lighting_handles_tiny_and_large_finite_distances() {
    for spot in [false, true] {
        for distance in [2.0_f32.powi(-60), 2.0_f32.powi(60)] {
            // A reciprocal linear term preserves unit attenuation at any distance.
            assert_eq!(
                shade_positional_light(
                    spot,
                    Vec3::new(0.0, 0.0, distance),
                    1.0,
                    [0.0, distance.recip(), 0.0]
                ),
                0xffcc_cccc
            );
        }
        // Squaring these finite distances in f32 would underflow/overflow.
        for distance in [f32::MIN_POSITIVE, f32::MAX] {
            assert_eq!(
                shade_positional_light(spot, Vec3::new(0.0, 0.0, distance), 1.0, [1.0, 0.0, 0.0]),
                0xffcc_cccc
            );
        }
    }
}

#[test]
fn intense_spot_outside_its_cone_preserves_other_lighting() {
    let color = shade_lit_vertex(
        MaterialState {
            ambient: 0x00ff_ffff,
            ..Default::default()
        },
        u32::MAX,
        false,
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::default(),
        Vec3::new(0.0, 0.0, 1.0),
        &[
            LightSource::Ambient {
                color: 0x00ff_0000,
                intensity: 0.5,
            },
            LightSource::Spot {
                position: Vec3::new(1.0, 0.0, 1.0),
                direction: Vec3::new(0.0, 0.0, -1.0),
                color: 0x00ff_ffff,
                intensity: f32::MAX,
                attenuation: [f32::MIN_POSITIVE, 0.0, 0.0],
                angle_degrees: 10.0,
                exponent: 1.0,
            },
        ],
    )
    .unwrap();
    assert_eq!(color, 0xff80_0000);
}
