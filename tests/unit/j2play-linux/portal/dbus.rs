use super::*;
use crate::{sleep_monitor::timeout, test_bus::PrivateBus, test_storage::Scratch};
use futures_lite::future;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zbus::{OwnedObjectPath, OwnedVariant, Variant};

struct RequestHandle(Arc<AtomicBool>);
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl RequestHandle {
    fn close(&self) {
        self.0.store(true, Ordering::Release);
    }
}

struct FileChooser {
    code: Option<u32>,
    closed: Arc<AtomicBool>,
}

#[zbus::interface(name = "org.freedesktop.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &self,
        parent: &str,
        title: &str,
        options: HashMap<String, OwnedVariant>,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert!(parent.is_empty());
        assert_eq!(title, "Fixture");
        let sender = header.sender().unwrap().as_str();
        let token = <&str>::try_from(&options["handle_token"]).unwrap();
        let path = format!(
            "/org/freedesktop/portal/desktop/request/{}/{token}",
            sender.trim_start_matches(':').replace('.', "_")
        );
        connection
            .object_server()
            .at(path.as_str(), RequestHandle(self.closed.clone()))
            .await?;
        if let Some(code) = self.code {
            let mut values = HashMap::new();
            values.insert("uris", Variant::from(vec!["file:///tmp/fixture.jar"]));
            // Deliberately emit before returning the method reply.
            connection
                .emit_signal(
                    Some(sender),
                    path.as_str(),
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(code, values),
                )
                .await?;
        }
        Ok(OwnedObjectPath::try_from(path).unwrap())
    }
}

#[test]
#[ignore = "requires dbus-daemon; uses only a private fixture bus"]
fn portal_handles_immediate_response_cancel_and_missing_backend() {
    let scratch = Scratch::new();
    let bus = PrivateBus::start(&scratch.0);
    zbus::block_on(future::race(
        async {
            let closed = Arc::new(AtomicBool::new(false));
            let server = timeout(
                zbus::connection::Builder::address(bus.address.as_str())
                    .name(DESTINATION)
                    .serve_at(
                        PATH,
                        FileChooser {
                            code: Some(0),
                            closed: closed.clone(),
                        },
                    )
                    .build(),
            )
            .await
            .unwrap();
            let client = connect_client(&bus).await;
            let lifecycle = Lifecycle::default();
            lifecycle.sleep(false);
            let (cancel, receiver) = async_channel::bounded(1);
            let run = |id| {
                request::request(
                    &client,
                    id,
                    "org.freedesktop.portal.FileChooser",
                    "OpenFile",
                    "Fixture",
                    HashMap::new(),
                    (&receiver, &lifecycle),
                )
            };
            let values = run(1).await.unwrap().unwrap();
            assert_eq!(
                Vec::<String>::try_from(values["uris"].try_clone().unwrap()).unwrap(),
                ["file:///tmp/fixture.jar"]
            );
            let interface = server
                .object_server()
                .interface::<_, FileChooser>(PATH)
                .await
                .unwrap();
            interface.get_mut().await.code = Some(1);
            assert!(run(2).await.unwrap().is_none());
            interface.get_mut().await.code = Some(2);
            assert_eq!(run(3).await.unwrap_err().code(), "linux-portal-rejected");
            interface.get_mut().await.code = None;
            closed.store(false, Ordering::Release);
            let (result, ()) = Box::pin(future::zip(run(4), async {
                async_io::Timer::after(Duration::from_millis(50)).await;
                cancel.close();
            }))
            .await;
            assert!(result.unwrap().is_none());
            assert!(closed.load(Ordering::Acquire));
            server.release_name(DESTINATION).await.unwrap();
            let fresh_client = connect_client(&bus).await;
            let start = Instant::now();
            let (_fresh_cancel, fresh_receiver) = async_channel::bounded(1);
            assert!(
                request::request(
                    &fresh_client,
                    5,
                    "org.freedesktop.portal.FileChooser",
                    "OpenFile",
                    "Fixture",
                    HashMap::new(),
                    (&fresh_receiver, &lifecycle)
                )
                .await
                .is_err()
            );
            assert!(start.elapsed() < Duration::from_secs(3));
        },
        async {
            async_io::Timer::after(Duration::from_secs(10)).await;
            panic!("portal fixture exceeded its deadline");
        },
    ));
}

async fn connect_client(bus: &PrivateBus) -> zbus::Connection {
    timeout(zbus::connection::Builder::address(bus.address.as_str()).build())
        .await
        .unwrap()
}
