use super::*;

pub(super) fn object(
    machine: &mut Machine<'_, '_>,
    class: &str,
    kind: m3g::ObjectKind,
) -> (Handle, m3g::Handle) {
    let guest = machine
        .heap
        .managed
        .allocate_object(class, HashMap::new())
        .unwrap();
    let native = machine
        .m3g
        .runtime
        .create(Some(guest.to_raw()), kind)
        .unwrap();
    (guest, native)
}

pub(super) fn setup_renderer(
    machine: &mut Machine<'_, '_>,
    projection: m3g::Mat4,
    camera_transform: m3g::Mat4,
) -> Handle {
    let (camera, _) = object(
        machine,
        "javax/microedition/m3g/Camera",
        m3g::ObjectKind::Camera {
            node: m3g::NodeState::default(),
            projection: m3g::CameraProjection::Generic(projection),
        },
    );
    machine.m3g.graphics.camera = Some((camera, camera_transform));
    let (target, _) = object(
        machine,
        "javax/microedition/m3g/Image2D",
        m3g::ObjectKind::Image2D(
            m3g::Image2DState::mutable(m3g::ImageFormat::Rgba, 32, 32).unwrap(),
        ),
    );
    machine.m3g.graphics.target = Some(target);
    machine.m3g.graphics.viewport = [0, 0, 32, 32];
    machine.m3g.graphics.renderer =
        m3g::SoftwareRenderer::new(32, 32, m3g::RenderLimits::default()).unwrap();
    target
}
