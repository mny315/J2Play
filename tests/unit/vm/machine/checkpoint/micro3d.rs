use super::*;
use crate::machine::Limits;

#[test]
fn checkpoint_rejects_stale_micro3d_owners_and_owned_references_before_replacing_vm() {
    let program = program();
    for invalid in [
        "owner",
        "disposed owner",
        "affine",
        "light",
        "sphere texture",
    ] {
        let mut context = DefaultNativeContext;
        let mut machine = program.machine(Limits::default(), false, &mut context);
        let instance = machine
            .heap
            .managed
            .allocate_object("Checkpoint", HashMap::new())
            .unwrap();
        let wrapper = machine
            .heap
            .managed
            .allocate_object("Checkpoint", HashMap::new())
            .unwrap();
        let (owner, kind) = match invalid {
            "owner" | "disposed owner" => (u64::MAX, ::micro3d::ObjectKind::Graphics),
            "affine" => (
                wrapper.to_raw(),
                ::micro3d::ObjectKind::Layout(::micro3d::FigureLayoutState {
                    affines: vec![u64::MAX],
                    ..::micro3d::FigureLayoutState::default()
                }),
            ),
            "light" | "sphere texture" => (
                wrapper.to_raw(),
                ::micro3d::ObjectKind::Effect(::micro3d::EffectState {
                    light: (invalid == "light").then_some(u64::MAX),
                    sphere_texture: (invalid == "sphere texture").then_some(u64::MAX),
                    ..::micro3d::EffectState::default()
                }),
            ),
            _ => unreachable!(),
        };
        machine.micro3d.runtime.create(owner, kind).unwrap();
        if invalid == "disposed owner" {
            machine.micro3d.runtime.dispose(owner).unwrap();
        }
        let bytes = machine.encode_checkpoint(instance).unwrap();
        let mut context = DefaultNativeContext;
        let mut restored = program.machine(Limits::default(), false, &mut context);
        restored.device.random_seed_sequence = 83;
        assert_eq!(
            restored
                .restore_checkpoint("Checkpoint", &bytes)
                .expect_err(invalid)
                .code(),
            "checkpoint-arena",
            "{invalid}"
        );
        assert_eq!(restored.device.random_seed_sequence, 83, "{invalid}");
        assert!(restored.heap.managed.is_empty(), "{invalid}");
    }
}

#[test]
fn checkpoint_preserves_ordinary_affine_endpoints_and_disposed_wrappers() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let handles = (0..4)
        .map(|_| {
            machine
                .heap
                .managed
                .allocate_object("Checkpoint", HashMap::new())
                .unwrap()
        })
        .collect::<Vec<_>>();
    machine
        .micro3d
        .runtime
        .create(
            handles[1].to_raw(),
            ::micro3d::ObjectKind::Layout(::micro3d::FigureLayoutState {
                affines: vec![handles[2].to_raw()],
                ..::micro3d::FigureLayoutState::default()
            }),
        )
        .unwrap();
    machine
        .micro3d
        .runtime
        .create(handles[3].to_raw(), ::micro3d::ObjectKind::Graphics)
        .unwrap();
    machine
        .micro3d
        .runtime
        .dispose(handles[3].to_raw())
        .unwrap();
    let bytes = machine.encode_checkpoint(handles[0]).unwrap();
    let mut context = DefaultNativeContext;
    let mut restored = program.machine(Limits::default(), false, &mut context);
    restored.restore_checkpoint("Checkpoint", &bytes).unwrap();
    assert_eq!(
        restored
            .micro3d
            .runtime
            .layout_affine(handles[1].to_raw(), 0)
            .unwrap(),
        Some(handles[2].to_raw())
    );
    assert!(restored.heap.managed.get(handles[2]).is_ok());
    assert!(restored.micro3d.runtime.kind(handles[3].to_raw()).is_err());
    assert_eq!(restored.micro3d.runtime.metrics().live_objects, 1);
}
