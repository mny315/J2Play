use super::*;

#[path = "sleep_monitor/dbus.rs"]
mod dbus;

#[test]
fn cancellation_drops_a_host_operation_that_never_replies() {
    let (cancel, worker_cancel) = async_channel::bounded(1);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let thread = thread::spawn(move || {
        future::block_on(until_cancelled(&worker_cancel, async {
            ready_tx.send(()).unwrap();
            future::pending::<Result<(), EmuError>>().await
        }))
    });
    ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    let started = Instant::now();
    cancel.close();
    while !thread.is_finished() && started.elapsed() < Duration::from_secs(1) {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(thread.is_finished());
    thread.join().unwrap().unwrap();
}

#[test]
fn listener_failure_is_reported_without_waiting_for_cancellation() {
    let (_cancel, receiver) = async_channel::bounded(1);
    let error = future::block_on(until_cancelled(&receiver, async {
        Err::<(), _>(unavailable())
    }))
    .unwrap_err();
    assert_eq!(error.code(), "linux-system-lifecycle");
}

#[test]
fn missing_worker_acknowledgement_has_a_bounded_error() {
    let lifecycle = Lifecycle::default();
    let started = Instant::now();
    let error = future::block_on(await_suspension(&lifecycle)).unwrap_err();
    assert_eq!(error.code(), "linux-system-suspend-deadline");
    assert!(started.elapsed() < SUSPEND_DEADLINE + Duration::from_secs(1));
    assert!(lifecycle.signal.suspended());
}
