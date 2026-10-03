//! Re-execute a potentially blocking unit test with a process deadline.

#[path = "process.rs"]
mod process;

pub fn isolate(root: &std::path::Path, timeout: std::time::Duration) -> bool {
    let name = std::thread::current()
        .name()
        .expect("isolation must be called from a named libtest case")
        .to_owned();
    if std::env::var("J2PLAY_UNIT_TEST_CASE").as_deref() == Ok(&name) {
        return false;
    }
    let result = process::run(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture", "--test-threads=1"])
            .env("J2PLAY_UNIT_TEST_CASE", &name)
            .env("TMPDIR", root),
        timeout,
    )
    .unwrap();
    let output = result.output;
    assert!(output.stdout.len() <= process::MAX_CAPTURE_BYTES);
    assert!(output.stderr.len() <= process::MAX_CAPTURE_BYTES);
    assert!(
        !result.timed_out && output.status.success(),
        "{name}: timed out={}; {}\n{}\n{}",
        result.timed_out,
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    true
}
