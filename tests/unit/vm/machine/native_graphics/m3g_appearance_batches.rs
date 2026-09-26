use super::m3g_submeshes::{render, scene};
use super::*;

fn textured_scene(
    machine: &mut Machine<'_, '_>,
    batches: usize,
    shared: bool,
    sprite: bool,
) -> (m3g::Handle, m3g::Handle) {
    let scene = scene(machine, 64, batches, 0, sprite);
    let runtime = &mut machine.m3g.runtime;
    let m3g::ObjectKind::Group { children, .. } = runtime.kind(scene.root).unwrap() else {
        panic!("expected group");
    };
    let meshes = children.clone();
    let image = runtime
        .create(
            None,
            m3g::ObjectKind::Image2D(
                m3g::Image2DState::from_argb(m3g::ImageFormat::Rgb, 4, 4, &[0xffe0_4020; 16])
                    .unwrap(),
            ),
        )
        .unwrap();
    let texture = runtime
        .create(
            None,
            m3g::ObjectKind::Texture2D(m3g::TextureObjectState {
                transformable: m3g::TransformableState::default(),
                image,
                level_filter: 208,
                image_filter: 210,
                wrap_s: 240,
                wrap_t: 240,
                blending: 228,
                blend_color: 0,
            }),
        )
        .unwrap();
    let mut first_appearance = None;
    for mesh in meshes {
        let m3g::ObjectKind::Mesh(state) = runtime.kind(mesh).unwrap() else {
            panic!("expected mesh")
        };
        let appearances = state.appearances.clone();
        first_appearance = first_appearance.or(appearances[0]);
        for appearance in appearances.into_iter().flatten() {
            let m3g::ObjectKind::Appearance(state) = runtime.kind_mut(appearance).unwrap() else {
                panic!("expected appearance")
            };
            state.textures[0] = Some(texture);
        }
        if shared {
            let m3g::ObjectKind::Mesh(state) = runtime.kind_mut(mesh).unwrap() else {
                panic!("expected mesh")
            };
            state.appearances.fill(first_appearance);
        }
    }
    if sprite {
        let image = runtime
            .create(
                None,
                m3g::ObjectKind::Image2D(
                    m3g::Image2DState::from_argb(m3g::ImageFormat::Rgb, 4, 4, &[0xff20_40e0; 16])
                        .unwrap(),
                ),
            )
            .unwrap();
        let sprite = runtime
            .create(
                None,
                m3g::ObjectKind::Sprite3D(m3g::SpriteState {
                    node: m3g::NodeState::default(),
                    scaled: false,
                    image,
                    appearance: first_appearance,
                    crop: [0, 0, 4, 4],
                }),
            )
            .unwrap();
        runtime.add_child(scene.root, sprite).unwrap();
        let m3g::ObjectKind::Group { children, .. } = runtime.kind_mut(scene.root).unwrap() else {
            panic!("expected group")
        };
        let last = children.len() - 1;
        children.swap(1, last);
    }
    (scene.root, texture)
}

#[test]
fn repeated_appearance_observes_texture_changes_and_sprite_state() {
    for sprite in [false, true] {
        let program = Program::new();
        let mut shared_context = DefaultNativeContext;
        let mut separate_context = DefaultNativeContext;
        let mut shared = program.machine(Limits::default(), false, &mut shared_context);
        let mut separate = program.machine(Limits::default(), false, &mut separate_context);
        let shared_scene = textured_scene(&mut shared, 8, true, sprite);
        let separate_scene = textured_scene(&mut separate, 8, false, sprite);
        let mut previous = None;
        for blending in [228, 224] {
            for (machine, (root, texture)) in
                [(&mut shared, shared_scene), (&mut separate, separate_scene)]
            {
                let m3g::ObjectKind::Texture2D(state) =
                    machine.m3g.runtime.kind_mut(texture).unwrap()
                else {
                    panic!("expected texture")
                };
                state.blending = blending;
                render(machine, root);
            }
            let renderer = &shared.m3g.graphics.renderer;
            assert!(renderer.pixels().contains(&if blending == 228 {
                0xffe0_4020
            } else {
                0xffff_ffff
            }));
            if sprite {
                assert!(renderer.pixels().contains(&0xff20_40e0));
            }
            assert_eq!(renderer.pixels(), separate.m3g.graphics.renderer.pixels());
            assert_eq!(renderer.depth(), separate.m3g.graphics.renderer.depth());
            assert_eq!(renderer.stats(), separate.m3g.graphics.renderer.stats());
            if let Some(previous) = previous {
                assert_ne!(renderer.pixels(), previous);
            }
            previous = Some(renderer.pixels().to_vec());
        }
    }
}

#[test]
#[ignore = "manual repeated appearance throughput measurement"]
fn repeated_appearance_throughput() {
    for batches in [1, 8, 64] {
        for shared in [false, true] {
            let program = Program::new();
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let (root, _) = textured_scene(&mut machine, batches, shared, false);
            let started = std::time::Instant::now();
            for _ in 0..1024 {
                render(&mut machine, std::hint::black_box(root));
            }
            let checksum = machine
                .m3g
                .graphics
                .renderer
                .pixels()
                .iter()
                .fold(0_u64, |sum, &pixel| {
                    sum.wrapping_mul(31).wrapping_add(u64::from(pixel))
                });
            eprintln!(
                "appearance batches={batches} shared={shared} elapsed={:?} checksum={checksum} stats={:?}",
                started.elapsed(),
                machine.m3g.graphics.renderer.stats()
            );
        }
    }
}
