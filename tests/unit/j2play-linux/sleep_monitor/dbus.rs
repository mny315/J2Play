use super::*;
use crate::{test_bus::PrivateBus, test_storage::Scratch};
use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

struct LoginManager {
    inhibitors: Arc<Mutex<Vec<UnixStream>>>,
    sleeping: bool,
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl LoginManager {
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::zvariant::OwnedFd {
        assert_eq!((what, who, mode), ("sleep", "J2Play", "delay"));
        assert!(!why.is_empty());
        let (guard, observer) = UnixStream::pair().unwrap();
        observer.set_nonblocking(true).unwrap();
        self.inhibitors.lock().unwrap().push(observer);
        let fd: std::os::fd::OwnedFd = guard.into();
        fd.into()
    }

    #[zbus(property)]
    fn preparing_for_sleep(&self) -> bool {
        self.sleeping
    }

    #[zbus(signal)]
    async fn prepare_for_sleep(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        sleeping: bool,
    ) -> zbus::Result<()>;
}

async fn wait_for(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < deadline, "private logind fixture deadline");
        async_io::Timer::after(Duration::from_millis(5)).await;
    }
}

fn released(inhibitors: &Mutex<Vec<UnixStream>>, index: usize) -> bool {
    let mut guard = inhibitors.lock().unwrap();
    match guard[index].read(&mut [0]) {
        Ok(0) => true,
        Err(error) if error.kind() == ErrorKind::WouldBlock => false,
        result => panic!("unexpected inhibitor state: {result:?}"),
    }
}

#[test]
#[ignore = "requires dbus-daemon; uses a private bus and never suspends the host"]
fn logind_delays_sleep_until_worker_ack_and_reacquires_after_resume() {
    let scratch = Scratch::new();
    let bus = PrivateBus::start(&scratch.0);
    future::block_on(async {
        let inhibitors = Arc::new(Mutex::new(Vec::new()));
        let server = timeout(
            zbus::connection::Builder::address(bus.address.as_str())
                .unwrap()
                .name("org.freedesktop.login1")
                .unwrap()
                .serve_at(
                    "/org/freedesktop/login1",
                    LoginManager {
                        inhibitors: inhibitors.clone(),
                        sleeping: false,
                    },
                )
                .unwrap()
                .build(),
        )
        .await
        .unwrap();
        let client = timeout(
            zbus::connection::Builder::address(bus.address.as_str())
                .unwrap()
                .internal_executor(false)
                .max_queued(8)
                .build(),
        )
        .await
        .unwrap();
        let lifecycle = Lifecycle::default();
        let ctx = egui::Context::default();
        let check = exercise_sleep(&server, &inhibitors, &lifecycle, &scratch);
        let (error, ()) = future::race(
            future::zip(monitor_connection(&client, &lifecycle, &ctx), check),
            async {
                async_io::Timer::after(Duration::from_secs(12)).await;
                panic!("private logind check timed out");
            },
        )
        .await;
        assert_eq!(error.unwrap_err().code(), "linux-system-lifecycle");
    });
}

async fn exercise_sleep(
    server: &zbus::Connection,
    inhibitors: &Mutex<Vec<UnixStream>>,
    lifecycle: &Lifecycle,
    scratch: &Scratch,
) {
    let manager = server
        .object_server()
        .interface::<_, LoginManager>("/org/freedesktop/login1")
        .await
        .unwrap();
    lifecycle.window(true, false);
    wait_for(|| lifecycle.poll() == Some(frontend_ui::PlatformLifecycleEvent::Resumed)).await;
    assert_eq!(inhibitors.lock().unwrap().len(), 1);
    LoginManager::prepare_for_sleep(manager.signal_emitter(), true)
        .await
        .unwrap();
    wait_for(|| lifecycle.signal.suspended()).await;
    async_io::Timer::after(Duration::from_millis(100)).await;
    assert!(
        !released(inhibitors, 0),
        "signalling alone must not release the delay"
    );
    let mut worker = frontend_core::RuntimeWorker::spawn_with_lifecycle(
        frontend_core::LibraryRepository::open(scratch.0.join("library")).unwrap(),
        lifecycle.signal.clone(),
    )
    .unwrap();
    wait_for(|| released(inhibitors, 0)).await;
    assert!(lifecycle.signal.suspension_acknowledged());
    LoginManager::prepare_for_sleep(manager.signal_emitter(), false)
        .await
        .unwrap();
    wait_for(|| inhibitors.lock().unwrap().len() == 2).await;
    wait_for(|| lifecycle.poll() == Some(frontend_ui::PlatformLifecycleEvent::Resumed)).await;
    assert!(!released(inhibitors, 1));
    LoginManager::prepare_for_sleep(manager.signal_emitter(), true)
        .await
        .unwrap();
    wait_for(|| released(inhibitors, 1)).await;
    worker.shutdown().unwrap();
    // A login-manager restart invalidates its lock. It must become an explicit
    // listener failure, not an unnoticed loss of sleep coverage.
    server.release_name("org.freedesktop.login1").await.unwrap();
}
