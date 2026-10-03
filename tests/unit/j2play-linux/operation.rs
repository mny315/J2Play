use super::*;
use std::sync::mpsc;

#[test]
fn shutdown_deadline_retains_the_thread_for_a_later_join() {
    let (release, wait) = mpsc::sync_channel::<()>(1);
    let mut handle = Some(thread::spawn(move || {
        let _ = wait.recv();
    }));
    let started = Instant::now();
    let result = join(&mut handle);
    release.send(()).unwrap();
    assert_eq!(result.unwrap_err().code(), "linux-adapter-deadline");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(handle.is_some());
    join(&mut handle).unwrap();
    assert!(handle.is_none());
    join(&mut handle).unwrap();
}

#[test]
fn panicked_adapter_is_joined_and_reported_once() {
    let mut handle = Some(thread::spawn(|| panic!("adapter panic fixture")));
    assert_eq!(
        join(&mut handle).unwrap_err().code(),
        "linux-adapter-thread"
    );
    assert!(handle.is_none());
    join(&mut handle).unwrap();
}
