use super::*;
use crate::machine::Limits;

#[test]
fn checkpoint_preserves_zero_valued_guest_handle_and_plain_user_objects() {
    let program = program();
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let instance = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    assert_eq!(instance.to_raw(), 0);
    let user = machine
        .heap
        .managed
        .allocate_object("Checkpoint", HashMap::new())
        .unwrap();
    let bound = machine
        .m3g
        .runtime
        .create(Some(instance.to_raw()), ::m3g::ObjectKind::Object)
        .unwrap();
    machine
        .m3g
        .runtime
        .set_user_object(bound, Some(user.to_raw()))
        .unwrap();
    let unbound = machine
        .m3g
        .runtime
        .create(None, ::m3g::ObjectKind::Object)
        .unwrap();
    machine
        .m3g
        .runtime
        .set_user_object(unbound, Some(instance.to_raw()))
        .unwrap();
    // Both native APIs receive raw managed handles, including the valid value zero.
    machine
        .micro3d
        .runtime
        .create(instance.to_raw(), ::micro3d::ObjectKind::Graphics)
        .unwrap();
    let bytes = machine.encode_checkpoint(instance).unwrap();
    let mut context = DefaultNativeContext;
    let mut restored = program.machine(Limits::default(), false, &mut context);
    assert_eq!(
        restored.restore_checkpoint("Checkpoint", &bytes).unwrap(),
        instance
    );
    assert_eq!(restored.m3g.runtime.resolve_guest(0).unwrap(), bound);
    assert_eq!(
        restored.m3g.runtime.user_object(bound).unwrap(),
        Some(user.to_raw())
    );
    assert_eq!(restored.m3g.runtime.user_object(unbound).unwrap(), Some(0));
    assert!(matches!(
        restored.micro3d.runtime.kind(0).unwrap(),
        ::micro3d::ObjectKind::Graphics
    ));
}

#[test]
fn checkpoint_rejects_stale_m3g_owners_and_user_objects_before_replacing_vm() {
    let program = program();
    for (invalid_owner, bound) in [(true, true), (false, true), (false, false)] {
        for invalid in [1, u64::MAX] {
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            let instance = machine
                .heap
                .managed
                .allocate_object("Checkpoint", HashMap::new())
                .unwrap();
            let owner = bound.then_some(if invalid_owner {
                invalid
            } else {
                instance.to_raw()
            });
            let handle = machine
                .m3g
                .runtime
                .create(owner, ::m3g::ObjectKind::Object)
                .unwrap();
            if !invalid_owner {
                machine
                    .m3g
                    .runtime
                    .set_user_object(handle, Some(invalid))
                    .unwrap();
            }
            let bytes = machine.encode_checkpoint(instance).unwrap();
            let mut context = DefaultNativeContext;
            let mut restored = program.machine(Limits::default(), false, &mut context);
            restored.device.random_seed_sequence = 83;
            assert_eq!(
                restored
                    .restore_checkpoint("Checkpoint", &bytes)
                    .unwrap_err()
                    .code(),
                "checkpoint-arena",
                "owner={invalid_owner}, bound={bound}, reference={invalid}"
            );
            assert_eq!(restored.device.random_seed_sequence, 83);
            assert!(restored.heap.managed.is_empty());
        }
    }
}
