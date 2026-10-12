use super::*;

mod allocation;
mod animation;
mod checkpoint;
mod composition;
mod hierarchy;
mod ownership;
mod transforms;
mod vertex_buffers;

fn group(runtime: &mut Runtime) -> Handle {
    runtime
        .create(
            None,
            ObjectKind::Group {
                node: NodeState::default(),
                children: Vec::new(),
            },
        )
        .unwrap()
}

#[test]
fn compositing_mode_defaults_to_opaque_replace() {
    let state = CompositingModeState::default();

    assert_eq!(state.blending, 68);
    assert!(state.depth_test);
    assert!(state.depth_write);
    assert!(state.color_write);
    assert!(state.alpha_write);
}

#[test]
fn world_validates_camera_and_background_types() {
    let mut runtime = Runtime::default();
    let world = runtime
        .create(
            None,
            ObjectKind::World {
                node: NodeState::default(),
                children: Vec::new(),
                camera: None,
                background: None,
            },
        )
        .unwrap();
    let camera = runtime
        .create(
            None,
            ObjectKind::Camera {
                node: NodeState::default(),
                projection: CameraProjection::default(),
            },
        )
        .unwrap();
    let background = runtime
        .create(None, ObjectKind::Background(BackgroundState::default()))
        .unwrap();
    runtime.set_world_camera(world, camera).unwrap();
    runtime
        .set_world_background(world, Some(background))
        .unwrap();
    assert_eq!(
        runtime
            .set_world_camera(world, background)
            .unwrap_err()
            .code(),
        "not-camera"
    );
    assert_eq!(
        runtime
            .set_world_background(world, Some(camera))
            .unwrap_err()
            .code(),
        "not-background"
    );
    let ObjectKind::World {
        camera: active_camera,
        background: active_background,
        ..
    } = runtime.kind(world).unwrap()
    else {
        unreachable!();
    };
    assert_eq!(*active_camera, Some(camera));
    assert_eq!(*active_background, Some(background));
    runtime.set_world_background(world, None).unwrap();
    assert!(matches!(
        runtime.kind(world).unwrap(),
        ObjectKind::World {
            background: None,
            ..
        }
    ));
}
