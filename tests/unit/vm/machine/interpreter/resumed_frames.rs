use super::*;

use crate::machine::tests::process;

fn continuation(method: &Method, child: Option<Box<SuspendedCall>>) -> Box<SuspendedCall> {
    Box::new(SuspendedCall {
        method: method.clone(),
        locals: Vec::new(),
        stack: Vec::new(),
        pc: 0,
        synchronized_monitor: None,
        monitor_entry: None,
        pending: child.map(|child| PendingCall { child, next_pc: 0 }),
        native_resume: None,
        class_initialization: None,
    })
}

#[test]
fn resumed_frame_obeys_active_call_stack_limit() {
    let program = Program::new();
    let method = runtime_method("Resume", "run", "()V", &[0xb1], 0, 0, vec![], true);
    let mut context = DefaultNativeContext;
    let limits = Limits {
        max_frames: 1,
        ..Limits::default()
    };
    let mut machine = program.machine(limits, false, &mut context);
    machine
        .execution
        .call_stack
        .push(method.active_stack_frame(0));
    let result = machine.resume_suspended_call(continuation(&method, None), 1);
    assert!(matches!(result, Err(error) if error.code() == "stack-overflow"));
    assert_eq!(machine.execution.call_stack.len(), 1);
}

#[test]
fn deep_resumed_frames_preserve_host_stack() {
    const CHILD_ENV: &str = "J2PLAY_TEST_DEEP_RESUMED_FRAMES";
    if std::env::var_os(CHILD_ENV).is_none() {
        // An exhausted Rust stack aborts the process, so isolate this fixture
        // to preserve useful diagnostics for the rest of the test suite.
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                std::thread::current().name().unwrap(),
                "--nocapture",
            ])
            .env(CHILD_ENV, "1");
        let result = process::run(&mut command, std::time::Duration::from_secs(10)).unwrap();
        assert!(!result.timed_out, "resumed frame fixture timed out");
        assert!(
            result.output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.output.stdout),
            String::from_utf8_lossy(&result.output.stderr)
        );
        assert!(String::from_utf8_lossy(&result.output.stdout).contains("1 passed"));
        return;
    }
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let program = Program::new();
            let method = runtime_method("Resume", "run", "()V", &[0xb1], 0, 0, vec![], true);
            let mut call = continuation(&method, None);
            for _ in 1..96 {
                call = continuation(&method, Some(call));
            }
            let mut context = DefaultNativeContext;
            let mut machine = program.machine(Limits::default(), false, &mut context);
            assert!(matches!(
                machine.resume_suspended_call(call, 1).unwrap(),
                CallOutcome::Return(None)
            ));
            assert!(machine.execution.call_stack.is_empty());
            assert_eq!(machine.execution.instructions, 96);
        })
        .unwrap()
        .join()
        .unwrap();
}
