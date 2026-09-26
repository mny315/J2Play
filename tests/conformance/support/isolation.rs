use std::process::Command;
use std::time::Duration;

pub(super) fn is_current_test_child() -> bool {
    std::env::var("J2PLAY_CONFORMANCE_CASE")
        .ok()
        .is_some_and(|name| std::thread::current().name() == Some(name.as_str()))
}

/// Re-executes one named libtest case, never a caller-supplied guest entrypoint.
/// The parent bounds output, kills a timed-out process, and always reaps it.
pub fn isolate(full_name: &str) -> bool {
    let name = full_name.strip_prefix("conformance::").unwrap();
    if std::env::var("J2PLAY_CONFORMANCE_CASE").as_deref() == Ok(name) {
        return false;
    }
    run_case(name, None);
    true
}

/// Run fixed persistence phases in distinct OS processes sharing one owned root.
pub fn process_phase(full_name: &str, count: usize) -> Option<(usize, std::path::PathBuf)> {
    assert!((1..=16).contains(&count));
    if let Ok(phase) = std::env::var("J2PLAY_CONFORMANCE_PHASE") {
        let phase: usize = phase.parse().unwrap();
        assert!(phase < count);
        return Some((
            phase,
            std::env::var_os("J2PLAY_CONFORMANCE_PHASE_ROOT")
                .unwrap()
                .into(),
        ));
    }
    let root = super::Scratch::new();
    let name = full_name.strip_prefix("conformance::").unwrap();
    for phase in 0..count {
        run_case(name, Some((phase, &root.0)));
    }
    None
}

fn run_case(name: &str, phase: Option<(usize, &std::path::Path)>) {
    // The parent owns the child's entire temporary root, including artifacts
    // left behind when a deadline terminates the child before its destructors.
    let scratch = super::Scratch::new();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("J2PLAY_CONFORMANCE_CASE", name)
        .env_remove("J2PLAY_CONFORMANCE_PHASE")
        .env_remove("J2PLAY_CONFORMANCE_PHASE_ROOT")
        .env("TMPDIR", &scratch.0)
        .env(
            "RUST_MIN_STACK",
            frontend_core::VM_HOST_STACK_BYTES.to_string(),
        );
    if let Some((phase, root)) = phase {
        command
            .env("J2PLAY_CONFORMANCE_PHASE", phase.to_string())
            .env("J2PLAY_CONFORMANCE_PHASE_ROOT", root);
    }
    let result = super::process::run(&mut command, Duration::from_secs(30)).unwrap();
    let super::process::TestOutput { output, timed_out } = result;
    let std::process::Output {
        status,
        stdout,
        stderr,
    } = output;
    assert!(
        stdout.len() <= super::MAX_CAPTURE_BYTES && stderr.len() <= super::MAX_CAPTURE_BYTES,
        "{name}: excessive test output"
    );
    assert!(
        !timed_out && status.success(),
        "{name}: deadline exceeded={timed_out}; {status}\n{}\n{}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        String::from_utf8_lossy(&stdout).contains("1 passed"),
        "child did not execute {name}"
    );
}

fn subprocess_test(full_name: &str) -> Option<Command> {
    if std::env::var("J2PLAY_PROCESS_TEST_CASE").as_deref() == Ok(full_name) {
        return None;
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            full_name.strip_prefix("conformance::").unwrap(),
            "--nocapture",
        ])
        .env("J2PLAY_PROCESS_TEST_CASE", full_name);
    Some(command)
}

#[test]
fn subprocess_deadline_kills_and_reaps_a_stuck_test() {
    let Some(mut command) = subprocess_test(concat!(
        module_path!(),
        "::subprocess_deadline_kills_and_reaps_a_stuck_test"
    )) else {
        std::fs::write(
            std::env::var_os("J2PLAY_PROCESS_PID_FILE").unwrap(),
            std::process::id().to_string(),
        )
        .unwrap();
        loop {
            std::thread::park();
        }
    };
    let scratch = super::Scratch::new();
    let pid_file = scratch.0.join("pid");
    command.env("J2PLAY_PROCESS_PID_FILE", &pid_file);
    let result = super::process::run(&mut command, Duration::from_secs(2)).unwrap();
    assert!(result.timed_out);
    assert!(!result.output.status.success());
    let pid: u32 = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
    #[cfg(target_os = "linux")]
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}

#[test]
fn subprocess_capture_drains_both_pipes_after_the_retention_limit() {
    use std::io::Write;
    let Some(mut command) = subprocess_test(concat!(
        module_path!(),
        "::subprocess_capture_drains_both_pipes_after_the_retention_limit"
    )) else {
        let bytes = vec![b'x'; super::MAX_CAPTURE_BYTES + 4096];
        std::io::stdout().write_all(&bytes).unwrap();
        std::io::stderr().write_all(&bytes).unwrap();
        return;
    };
    let result = super::process::run(&mut command, Duration::from_secs(5)).unwrap();
    assert!(!result.timed_out);
    assert!(result.output.status.success());
    assert_eq!(result.output.stdout.len(), super::MAX_CAPTURE_BYTES + 1);
    assert_eq!(result.output.stderr.len(), super::MAX_CAPTURE_BYTES + 1);
}

#[test]
#[should_panic(expected = "guest fixtures must run inside isolate() or process_phase()")]
fn fixture_rejects_execution_without_a_process_deadline() {
    super::Fixture::new("unused.jar").run_static("Unused", "unused", "()I");
}
