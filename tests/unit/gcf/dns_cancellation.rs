use super::*;
use std::sync::atomic::AtomicBool;

#[test]
fn pending_dns_can_be_cancelled_without_waiting_for_the_os_resolver() {
    let (_sender, receiver) = std::sync::mpsc::sync_channel(1);
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let trigger = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.store(true, Ordering::Release);
        Instant::now()
    });
    let cancellation = Cancellation::new(move || cancelled.load(Ordering::Acquire));
    let result = receive_dns(
        &receiver,
        Instant::now(),
        Duration::from_secs(30),
        &cancellation,
    );
    let elapsed = trigger.join().unwrap().elapsed();
    assert_eq!(result.unwrap_err().code(), "network-io");
    assert!(elapsed < Duration::from_millis(750), "{elapsed:?}");
}

#[test]
fn dns_deadline_and_disconnected_worker_are_explicit_errors() {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    assert_eq!(
        receive_dns(
            &receiver,
            Instant::now(),
            Duration::from_millis(30),
            &Cancellation::default()
        )
        .unwrap_err()
        .code(),
        "network-timeout"
    );
    drop(sender);
    assert_eq!(
        receive_dns(
            &receiver,
            Instant::now(),
            Duration::from_secs(30),
            &Cancellation::default()
        )
        .unwrap_err()
        .code(),
        "network-dns"
    );
}
