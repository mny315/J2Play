use std::path::PathBuf;

#[test]
fn cooperative_threads_and_monitors_execute_on_rust_vm() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "cooperative_threads_and_monitors_execute_on_rust_vm"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = [
        ("producerConsumer", 60),
        ("recursiveMonitor", 61),
        ("exceptionUnlocksMonitor", 62),
        ("interruptWait", 63),
        ("virtualTimeTimer", 64),
        ("deterministicReplay", 65),
        ("stress1000", 66),
        ("lifecycle", 67),
        ("threadLimit", 68),
        ("interruptSleep", 69),
        ("recursiveWaitInterrupt", 70),
        ("timerThreadIdentity", 71),
        ("timerDeadlineOrder", 72),
        ("uncaughtChildDoesNotAbort", 73),
        ("mainThreadAccounting", 74),
        ("timerFailureIsIsolated", 75),
        ("terminatedInterruptIgnored", 76),
        ("parallelSleepClock", 77),
        ("quantumContinuations", 78),
        ("stackSemantics", 79),
        ("writeCharsSemantics", 80),
        ("dataInterfacesRoundTrip", 81),
        ("timerTaskSleepDoesNotAbort", 82),
    ];
    for (method, expected) in fixtures {
        let output =
            crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
                .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
                .run_static("fixtures/Stage6Fixtures", method, "()I");
        assert!(output.success(), "{method}: {}", output.diagnostics);
        let diagnostics = &output.diagnostics;
        assert_eq!(
            output.int_value(),
            Some(expected),
            "{method}: {diagnostics}"
        );
    }
}

#[test]
fn midlet_reports_uncaught_worker_failure_without_vm_trace() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "midlet_reports_uncaught_worker_failure_without_vm_trace"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
            .midlet(7)
            .run_midlet();
    assert!(!output.success());
    let stderr = &output.diagnostics;
    assert!(stderr.contains("vm[uncaught-thread-exception]"), "{stderr}");
    assert!(
        stderr.contains("java/lang/IllegalStateException"),
        "{stderr}"
    );
    assert!(
        stderr.contains("Stage6WorkerFailureMidlet$1::run()V"),
        "{stderr}"
    );
    let execution = output.execution.as_ref().unwrap();
    assert_eq!(execution.thread_failure_count, 1);
    assert_eq!(execution.thread_failures.len(), 1);
    assert_eq!(stderr.matches("vm[uncaught-thread-exception]").count(), 1);
}

#[test]
fn deadlock_is_reported_deterministically() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "deadlock_is_reported_deterministically"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut diagnostics = Vec::new();
    for _ in 0..2 {
        let output =
            crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
                .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
                .run_static("fixtures/Stage6Fixtures", "deadlock", "()I");
        assert!(!output.success());
        diagnostics.push(output.diagnostics);
    }
    assert_eq!(diagnostics[0], diagnostics[1]);
    assert!(
        diagnostics[0].contains("vm[deadlock]"),
        "{}",
        diagnostics[0]
    );
}
