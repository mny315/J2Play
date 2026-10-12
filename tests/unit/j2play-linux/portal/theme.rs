use super::*;
use crate::{test_bus::PrivateBus, test_storage::Scratch};
use std::time::Duration;

struct Settings(u32);

#[zbus::interface(name = "org.freedesktop.portal.Settings")]
impl Settings {
    fn read(&self, namespace: &str, key: &str) -> OwnedValue {
        assert_eq!(
            (namespace, key),
            ("org.freedesktop.appearance", "color-scheme")
        );
        self.0.into()
    }
}

#[test]
#[ignore = "requires dbus-daemon; uses only a private fixture bus"]
fn portal_owner_loss_ends_theme_watch_without_waiting_for_another_setting() {
    let scratch = Scratch::new();
    let bus = PrivateBus::start(&scratch.0);
    future::block_on(future::race(
        async {
            let server = timeout(
                zbus::connection::Builder::address(bus.address.as_str())
                    .unwrap()
                    .name(DESTINATION)
                    .unwrap()
                    .serve_at(PATH, Settings(1))
                    .unwrap()
                    .build(),
            )
            .await
            .unwrap();
            let client = timeout(
                zbus::connection::Builder::address(bus.address.as_str())
                    .unwrap()
                    .internal_executor(false)
                    .build(),
            )
            .await
            .unwrap();
            let preference = AtomicU32::new(0);
            let ctx = egui::Context::default();
            let (result, ()) = future::zip(watch_connection(&client, &preference, &ctx), async {
                while preference.load(Ordering::Relaxed) != 1 {
                    async_io::Timer::after(Duration::from_millis(5)).await;
                }
                server.release_name(DESTINATION).await.unwrap();
            })
            .await;
            assert_eq!(result.unwrap_err().code(), "linux-portal-unavailable");
        },
        async {
            async_io::Timer::after(Duration::from_secs(5)).await;
            panic!("theme watcher did not detect portal owner loss");
        },
    ));
}
