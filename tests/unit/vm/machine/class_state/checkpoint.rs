use super::*;
use crate::machine::tests::test_class_definition;

fn program() -> Program {
    let mut program = Program::new();
    program
        .classes
        .insert("Element".into(), test_class_definition(None));
    program
}

#[test]
fn array_mirrors_require_complete_bounded_descriptors_and_loaded_reference_components() {
    let program = program();
    for name in [
        "[Z",
        "[B",
        "[C",
        "[S",
        "[I",
        "[J",
        "[F",
        "[D",
        "[[S",
        "[LElement;",
        "[[LElement;",
    ] {
        let mut state = ClassState::with_program(&program);
        state.objects.insert(name.into(), Handle::from_raw(0));
        let bytes = state.encode_checkpoint(&|| false).unwrap();
        let restored = ClassState::restore_checkpoint(&program, &bytes).unwrap();
        assert_eq!(restored.objects[name], Handle::from_raw(0));
    }
    for name in [
        "Missing",
        "[LMissing;",
        "[[LMissing;",
        "[",
        "[V",
        "[Q",
        "[Iextra",
        "[L;",
        "[LElement",
        "[LElement;;",
    ] {
        let mut state = ClassState::with_program(&program);
        state.objects.insert(name.into(), Handle::from_raw(0));
        let bytes = state.encode_checkpoint(&|| false).unwrap();
        assert_eq!(
            ClassState::restore_checkpoint(&program, &bytes)
                .unwrap_err()
                .code(),
            "checkpoint-classes",
            "{name}"
        );
    }
    assert!(class_object_matches_program(
        &program,
        &format!("{}I", "[".repeat(255))
    ));
    assert!(!class_object_matches_program(
        &program,
        &format!("{}I", "[".repeat(256))
    ));
}

#[test]
fn array_mirrors_do_not_allow_array_descriptors_in_class_initialization_state() {
    let program = program();
    for table in 0..4 {
        let mut state = ClassState::with_program(&program);
        match table {
            0 => {
                state.initialized.insert("[I".into());
            }
            1 => {
                state.initializing.insert("[I".into(), 0);
            }
            2 => {
                state
                    .failed_initialization
                    .insert("[I".into(), "failure".into());
            }
            _ => {
                state.initialization_waiters.insert("[I".into(), vec![]);
            }
        }
        let bytes = state.encode_checkpoint(&|| false).unwrap();
        assert_eq!(
            ClassState::restore_checkpoint(&program, &bytes)
                .unwrap_err()
                .code(),
            "checkpoint-classes"
        );
    }
}
