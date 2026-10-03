use super::*;
use crate::machine::{
    CallOutcome, DefaultNativeContext, HashMap, fnv1a64, suspended_monitor_entry,
    suspended_native_call,
    tests::{method, runtime_method},
};

#[test]
fn checkpoint_restores_synchronized_entry_before_locals_are_initialized() {
    let mut method = runtime_method("T", "locked", "(J)J", &[0x1f, 0xad], 2, 3, vec![], false);
    method.is_synchronized = true;
    let mut program = Program::new();
    program.methods.insert(method.key.clone(), method.clone());
    let mut context = DefaultNativeContext;
    let mut machine = program.machine(Limits::default(), false, &mut context);
    let receiver = machine
        .heap
        .managed
        .allocate_object("T", HashMap::new())
        .unwrap();
    let CallOutcome::Suspend(call) = suspended_monitor_entry(
        &method,
        receiver,
        vec![Value::Reference(Some(receiver)), Value::Long(42)],
    ) else {
        unreachable!()
    };
    assert!(call.locals.is_empty());
    let bytes = save_state::encode(call.as_ref()).unwrap();
    let saved: SavedCall = save_state::decode(&bytes).unwrap();
    let restored = saved.restore(&program, &Limits::default()).unwrap();
    assert_eq!(save_state::encode(restored.as_ref()).unwrap(), bytes);
    assert!(matches!(
        machine.resume_suspended_call(restored, 1).unwrap(),
        CallOutcome::Return(Some(Value::Long(42)))
    ));
    assert!(!machine.scheduler.monitors.contains_key(&receiver));
}

#[test]
fn checkpoint_validates_synchronized_entry_arguments_and_slot_budget() {
    let receiver = Handle::from_raw(1);
    let receiver_arg = Value::Reference(Some(receiver));
    for (is_static, is_native, synchronized, args, max_stack_slots, valid) in [
        (
            false,
            false,
            true,
            vec![receiver_arg, Value::Long(7)],
            3,
            true,
        ),
        (
            false,
            false,
            true,
            vec![receiver_arg, Value::Long(7)],
            2,
            false,
        ),
        (
            false,
            true,
            true,
            vec![receiver_arg, Value::Long(7)],
            3,
            true,
        ),
        (true, false, true, vec![Value::Long(7)], 2, true),
        (true, true, true, vec![Value::Long(7)], 1, false),
        (
            false,
            false,
            false,
            vec![receiver_arg, Value::Long(7)],
            3,
            false,
        ),
        (
            false,
            false,
            true,
            vec![receiver_arg, Value::Int(7)],
            3,
            false,
        ),
        (false, false, true, vec![receiver_arg], 3, false),
        (
            false,
            false,
            true,
            vec![Value::Reference(None), Value::Long(7)],
            3,
            false,
        ),
        (
            false,
            false,
            true,
            vec![Value::Reference(Some(Handle::from_raw(2))), Value::Long(7)],
            3,
            false,
        ),
    ] {
        let mut method = runtime_method(
            "T",
            "locked",
            "(J)J",
            &[0x1f, 0xad],
            2,
            3,
            vec![],
            is_static,
        );
        method.is_synchronized = synchronized;
        method.is_native = is_native;
        let mut program = Program::new();
        program.methods.insert(method.key.clone(), method.clone());
        let CallOutcome::Suspend(call) = suspended_monitor_entry(&method, receiver, args) else {
            unreachable!()
        };
        let saved: SavedCall =
            save_state::decode(&save_state::encode(call.as_ref()).unwrap()).unwrap();
        let result = saved.restore(
            &program,
            &Limits {
                max_stack_slots,
                ..Limits::default()
            },
        );
        if valid {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().code(), "checkpoint-stack");
        }
    }
}

#[test]
fn checkpoint_rejects_monitor_entry_with_an_initialized_frame() {
    let mut method = runtime_method("T", "locked", "()V", &[0xb1], 1, 1, vec![], false);
    method.is_synchronized = true;
    let mut program = Program::new();
    program.methods.insert(method.key.clone(), method.clone());
    let receiver = Handle::from_raw(1);
    for (locals, stack, synchronized_monitor) in [
        (vec![Some(Value::Reference(Some(receiver)))], vec![], None),
        (vec![], vec![Value::Int(1)], None),
        (vec![], vec![], Some(receiver)),
    ] {
        let CallOutcome::Suspend(mut call) =
            suspended_monitor_entry(&method, receiver, vec![Value::Reference(Some(receiver))])
        else {
            unreachable!()
        };
        call.locals = locals;
        call.stack = stack;
        call.synchronized_monitor = synchronized_monitor;
        let saved: SavedCall =
            save_state::decode(&save_state::encode(call.as_ref()).unwrap()).unwrap();
        assert_eq!(
            saved
                .restore(&program, &Limits::default())
                .unwrap_err()
                .code(),
            "checkpoint-stack"
        );
    }
}

#[test]
fn restored_operand_stacks_count_both_slots_of_long_and_double_values() {
    for (stack, max_stack, max_stack_slots, valid) in [
        (vec![Value::Long(7)], 1, 8, false),
        (vec![Value::Double(7.0)], 1, 8, false),
        (vec![Value::Long(7), Value::Int(3)], 2, 8, false),
        (vec![Value::Long(7)], 2, 1, false),
        (vec![Value::Long(7)], 2, 2, true),
        (vec![Value::Double(7.0), Value::Int(3)], 3, 3, true),
    ] {
        let method = method(&[0xb1], max_stack, 0);
        let mut program = Program::new();
        program.methods.insert(method.key.clone(), method.clone());
        let saved = SavedCall(vec![SavedFrame {
            method: method.key,
            locals: vec![],
            stack: stack.clone(),
            pc: 0,
            synchronized_monitor: None,
            monitor_entry: None,
            next_pc: None,
            native_resume: None,
            class_initialization: None,
        }]);
        let result = saved.restore(
            &program,
            &Limits {
                max_stack_slots,
                ..Limits::default()
            },
        );
        if valid {
            assert_eq!(result.unwrap().stack, stack);
        } else {
            assert_eq!(result.err().unwrap().code(), "checkpoint-stack");
        }
    }
}

#[test]
fn flat_call_checkpoint_preserves_frame_order_and_enforces_the_depth_boundary() {
    let mut method = method(&[], 1, 1);
    method.is_native = true;
    let mut program = Program::new();
    program.methods.insert(method.key.clone(), method.clone());
    let limits = Limits {
        max_frames: 512,
        ..Limits::default()
    };
    for count in [1, 2, 512, 513] {
        let mut chain = None;
        for index in (0..count).rev() {
            let CallOutcome::Suspend(mut frame) =
                suspended_native_call(&method, NativeResume::Sleep)
            else {
                unreachable!()
            };
            frame.locals.push(Some(Value::Int(index)));
            frame.pending = chain.map(|child| PendingCall { child, next_pc: 0 });
            chain = Some(frame);
        }
        let chain = chain.unwrap();
        let encoded = save_state::encode(chain.as_ref());
        if count > 512 {
            assert_eq!(encoded.unwrap_err().code(), "checkpoint-encode");
            continue;
        }
        let bytes = encoded.unwrap();
        // Existing flat encoding of this synthetic call chain, before sharing
        // its traversal with GC. Moving runtime types must not change save bytes.
        let expected = match count {
            1 => 0x1af2_606f_ac83_3ee6,
            2 => 0xe031_524c_5504_20fe,
            _ => 0x490d_f706_de97_4b18,
        };
        assert_eq!(fnv1a64(&bytes), expected);
        let saved: SavedCall = save_state::decode(&bytes).unwrap();
        let restored = saved.restore(&program, &limits).unwrap();
        assert_eq!(save_state::encode(restored.as_ref()).unwrap(), bytes);
        let mut current = Some(restored.as_ref());
        for index in 0..count {
            let frame = current.take().unwrap();
            assert_eq!(frame.locals, [Some(Value::Int(index))]);
            assert!(matches!(frame.native_resume, Some(NativeResume::Sleep)));
            current = frame.pending.as_ref().map(|pending| pending.child.as_ref());
        }
        assert!(current.is_none());
    }
}
