use super::*;
use m3g_support::{object, setup_renderer};

struct SpriteScene {
    group: m3g::Handle,
    sprite: m3g::Handle,
    intersection: m3g::Handle,
}

fn sprite_scene(machine: &mut Machine<'_, '_>, scaled: bool) -> SpriteScene {
    let (_, group) = object(
        machine,
        "javax/microedition/m3g/Group",
        m3g::ObjectKind::Group {
            node: m3g::NodeState::default(),
            children: Vec::new(),
        },
    );
    let (_, appearance) = object(
        machine,
        "javax/microedition/m3g/Appearance",
        m3g::ObjectKind::Appearance(m3g::AppearanceState::default()),
    );
    let (_, image) = object(
        machine,
        "javax/microedition/m3g/Image2D",
        m3g::ObjectKind::Image2D(
            m3g::Image2DState::from_argb(
                m3g::ImageFormat::Rgb,
                2,
                2,
                &[0xffcc_7733, 0xff33_77cc, 0xffaa_cc33, 0xffcc_33aa],
            )
            .unwrap(),
        ),
    );
    let (_, sprite) = object(
        machine,
        "javax/microedition/m3g/Sprite3D",
        m3g::ObjectKind::Sprite3D(m3g::SpriteState {
            node: m3g::NodeState::default(),
            scaled,
            image,
            appearance: Some(appearance),
            crop: [0, 0, 2, 2],
        }),
    );
    machine.m3g.runtime.add_child(group, sprite).unwrap();
    machine
        .m3g
        .runtime
        .set_translation(sprite, m3g::Vec3::new(0.0, 0.0, -3.0))
        .unwrap();
    let (_, intersection) = object(
        machine,
        "javax/microedition/m3g/RayIntersection",
        m3g::ObjectKind::RayIntersection(m3g::RayIntersectionState::default()),
    );
    SpriteScene {
        group,
        sprite,
        intersection,
    }
}

fn projection(scale: f32) -> m3g::Mat4 {
    let projection = m3g::CameraProjection::Perspective {
        field_of_view: 90.0,
        aspect_ratio: 1.0,
        near: 1.0,
        far: 10.0,
    }
    .render_matrix()
    .unwrap();
    m3g::Mat4::from_array(projection.as_array().map(|value| value * scale)).unwrap()
}

#[test]
fn sprites_render_identically_with_equivalent_homogeneous_projections() {
    for scaled in [false, true] {
        let program = Program::new();
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let scene = sprite_scene(&mut machine, scaled);
        let mut reference = None;
        for scale in [1.0, 2.0_f32.powi(-60), 2.0_f32.powi(60)] {
            // Each draw gets a fresh bound image so previous output cannot hide a skipped draw.
            setup_renderer(&mut machine, projection(scale), m3g::Mat4::IDENTITY);
            machine
                .m3g_render_retained(scene.group, None, false)
                .unwrap();
            let renderer = &machine.m3g.graphics.renderer;
            let frame = (renderer.pixels().to_vec(), renderer.depth().to_vec());
            assert!(frame.0.contains(&0xffcc_7733), "{scaled}, {scale}");
            assert!(renderer.stats().shaded_fragments > 0, "{scaled}, {scale}");
            if let Some(reference) = &reference {
                assert!(&frame == reference, "scaled={scaled}, projection={scale}");
            } else {
                reference = Some(frame);
            }
        }
    }
}

fn assert_sprite_center_hit(
    machine: &mut Machine<'_, '_>,
    scene: &SpriteScene,
    projection: m3g::Mat4,
) {
    assert!(
        machine
            .m3g_pick_ray(
                scene.group,
                u32::MAX,
                m3g::Vec3::new(0.0, 0.0, -1.0),
                m3g::Vec3::new(0.0, 0.0, -9.0),
                Some(scene.intersection),
                Some(M3gSpritePickContext {
                    viewport_point: [0.5, 0.5],
                    projection,
                    group_to_camera: m3g::Mat4::IDENTITY,
                }),
            )
            .unwrap()
    );
    let m3g::ObjectKind::RayIntersection(hit) =
        machine.m3g.runtime.kind(scene.intersection).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(hit.intersected, Some(scene.sprite));
    assert!((hit.distance - 2.0 / 9.0).abs() < 1.0e-6);
    for coordinate in hit.texture[0] {
        assert!((coordinate - 0.5).abs() < 1.0e-6, "{coordinate}");
    }
}

#[test]
fn sprite_picking_accepts_equivalent_homogeneous_projections() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = sprite_scene(&mut machine, true);
    for scale in [1.0, 2.0_f32.powi(-60), 2.0_f32.powi(60)] {
        assert_sprite_center_hit(&mut machine, &scene, projection(scale));
    }
}

#[test]
fn sprite_picking_preserves_center_coordinates_below_pixel_size() {
    let program = Program::new();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let scene = sprite_scene(&mut machine, true);
    let scale = 2.0_f32.powi(-60);
    machine
        .m3g
        .runtime
        .set_scale(scene.sprite, m3g::Vec3::new(scale, scale, scale))
        .unwrap();
    assert_sprite_center_hit(&mut machine, &scene, projection(1.0));
}

#[test]
pub(crate) fn sprite_negative_crop_mirrors_without_moving_the_source_rectangle() {
    assert_eq!(m3g_sprite_crop_coordinate(129, 31, 0.0), 129.0);
    assert_eq!(m3g_sprite_crop_coordinate(129, 31, 1.0), 160.0);
    assert_eq!(m3g_sprite_crop_coordinate(129, -31, 0.0), 160.0);
    assert_eq!(m3g_sprite_crop_coordinate(129, -31, 1.0), 129.0);
    assert_eq!(m3g_sprite_crop_sample(129, 31, 0.0), 129);
    assert_eq!(m3g_sprite_crop_sample(129, 31, 1.0), 159);
    assert_eq!(m3g_sprite_crop_sample(129, -31, 0.0), 159);
    assert_eq!(m3g_sprite_crop_sample(129, -31, 1.0), 129);
}
