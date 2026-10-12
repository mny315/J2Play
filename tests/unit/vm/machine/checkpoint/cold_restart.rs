use super::*;
use crate::machine::{ArrayKind, Limits};

use crate::machine::tests::process;
#[path = "../../../../support/storage.rs"]
mod storage;

#[test]
fn saved_array_class_objects_survive_process_exit_and_relinking() {
    const PHASE: &str = "J2PLAY_CHECKPOINT_RESTART_PHASE";
    const FILE: &str = "J2PLAY_CHECKPOINT_RESTART_FILE";
    let Ok(phase) = std::env::var(PHASE) else {
        let scratch = storage::Scratch::new();
        let file = scratch.0.join("checkpoint");
        for phase in ["save", "restore"] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    std::thread::current().name().unwrap(),
                    "--nocapture",
                ])
                .env(PHASE, phase)
                .env(FILE, &file);
            let result = process::run(&mut command, std::time::Duration::from_secs(10)).unwrap();
            assert!(!result.timed_out, "checkpoint {phase} timed out");
            assert!(
                result.output.status.success(),
                "checkpoint {phase}: {}\n{}",
                String::from_utf8_lossy(&result.output.stdout),
                String::from_utf8_lossy(&result.output.stderr)
            );
            if phase == "restore" {
                assert!(String::from_utf8_lossy(&result.output.stdout).contains("1 passed"));
            }
        }
        return;
    };
    let file = std::env::var_os(FILE).unwrap();
    let mut program = program();
    for name in ["java/lang/Object", "java/lang/Class"] {
        program
            .classes
            .insert(name.into(), test_class_definition(None));
    }
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    if phase == "save" {
        let instance = machine
            .heap
            .managed
            .allocate_object("Checkpoint", HashMap::new())
            .unwrap();
        for kind in [
            ArrayKind::Int,
            ArrayKind::Byte,
            ArrayKind::Short,
            ArrayKind::Long,
            ArrayKind::Reference("[S".into()),
            ArrayKind::Reference("Checkpoint".into()),
        ] {
            let name = crate::machine::array_descriptor(&kind);
            machine.intern_java_class(&name).unwrap();
        }
        machine.device.random_seed_sequence = 73;
        std::fs::write(file, machine.encode_checkpoint(instance).unwrap()).unwrap();
        // Exit without dropping the VM: restoration must use only persisted data.
        std::process::exit(0);
    }
    assert_eq!(phase, "restore");
    let bytes = std::fs::read(file).unwrap();
    machine.restore_checkpoint("Checkpoint", &bytes).unwrap();
    assert_eq!(machine.device.random_seed_sequence, 73);
    for name in ["[I", "[B", "[S", "[J", "[[S", "[LCheckpoint;"] {
        let original = machine.classes.objects[name];
        assert_eq!(machine.intern_java_class(name).unwrap(), original);
        assert_eq!(machine.class_name_for_handle(original).unwrap(), name);
        assert!(
            matches!(machine.heap.managed.get(original), Ok(heap::Allocation::Object { class, .. }) if class.as_ref() == "java/lang/Class")
        );
    }
}
