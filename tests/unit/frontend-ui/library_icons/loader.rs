use super::*;
use crate::tests::test_storage::Scratch;
use frontend_core::{GameSettings, ImportSource, inspect_import};

fn entry() -> (Scratch, LibraryEntry) {
    let scratch = Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let prepared = inspect_import(ImportSource::new(
        None,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/java-me/conformance.jar"
        ))
        .to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    (scratch, entry)
}

fn finish(loader: &mut LibraryAssetLoader) -> Option<ReadyAssets> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let ready = loader.take_ready().unwrap();
        if !loader.busy {
            return ready;
        }
        assert!(Instant::now() < deadline, "asset worker did not reply");
        thread::sleep(Duration::from_millis(1));
    }
}

fn image(edge: u32) -> Assets {
    Assets {
        image: Some(egui::ColorImage::from_rgba_unmultiplied(
            [edge as usize, 1],
            &vec![255; edge as usize * 4],
        )),
        info: None,
    }
}

#[test]
fn slow_asset_reads_do_not_block_requests_or_queue_more_work() {
    let (_scratch, entry) = entry();
    let (started, started_rx) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel(1);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut loader = LibraryAssetLoader::spawn_with(move |_, edge, _| {
        observed.fetch_add(1, Ordering::Relaxed);
        started.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        image(edge)
    })
    .unwrap();
    let ctx = egui::Context::default();
    loader.request(&entry, 64, &ctx).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    for _ in 0..100 {
        loader.request(&entry, 80, &ctx).unwrap();
    }
    assert!(loader.take_ready().unwrap().is_none());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    release.send(()).unwrap();
    let ready = finish(&mut loader).unwrap();
    assert_eq!(ready.entry_id, entry.id());
    assert_eq!(ready.image.unwrap().size, [64, 1]);
    loader.shutdown().unwrap();
    assert!(loader.thread.is_none());
}

#[test]
fn invalidation_discards_in_flight_results_and_allows_the_next_request() {
    let (_scratch, entry) = entry();
    let (started, started_rx) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel(1);
    let mut first = true;
    let mut loader = LibraryAssetLoader::spawn_with(move |_, edge, _| {
        if first {
            first = false;
            started.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        image(edge)
    })
    .unwrap();
    let ctx = egui::Context::default();
    loader.request(&entry, 64, &ctx).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    loader.invalidate();
    release.send(()).unwrap();
    assert!(finish(&mut loader).is_none());
    loader.request(&entry, 80, &ctx).unwrap();
    assert_eq!(finish(&mut loader).unwrap().image.unwrap().size, [80, 1]);
    loader.shutdown().unwrap();
}

#[test]
fn shutdown_cancels_active_reads_and_joins_the_worker() {
    let (_scratch, entry) = entry();
    let (started, started_rx) = mpsc::sync_channel(1);
    let mut loader = LibraryAssetLoader::spawn_with(move |_, edge, cancelled| {
        started.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !cancelled() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        image(edge)
    })
    .unwrap();
    loader
        .request(&entry, 64, &egui::Context::default())
        .unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    loader.shutdown().unwrap();
    assert!(loader.thread.is_none());
    assert!(loader.ready.try_recv().unwrap().image.is_none());
}

#[test]
fn shutdown_deadline_retains_the_unfinished_worker_for_a_later_join() {
    let (_scratch, entry) = entry();
    let (started, started_rx) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel(1);
    let mut loader = LibraryAssetLoader::spawn_with(move |_, _, _| {
        started.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        Assets::default()
    })
    .unwrap();
    loader
        .request(&entry, 64, &egui::Context::default())
        .unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        loader
            .shutdown_before(Instant::now() + Duration::from_millis(5))
            .is_err()
    );
    assert!(loader.thread.is_some());
    release.send(()).unwrap();
    loader.shutdown().unwrap();
    assert!(loader.thread.is_none());
}

#[test]
fn reimport_and_deletion_cannot_publish_an_old_thumbnail() {
    for delete in [false, true] {
        let (scratch, entry) = entry();
        let mut app = crate::FrontendApp::with_repository(
            LibraryRepository::open(&scratch.0).unwrap(),
            Box::new(crate::UnavailablePlatformBridge),
        )
        .unwrap();
        let (started, started_rx) = mpsc::sync_channel(1);
        let (release, release_rx) = mpsc::sync_channel(1);
        app.library_assets = LibraryAssetLoader::spawn_with(move |_, edge, _| {
            started.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            image(edge)
        })
        .unwrap();
        let ctx = egui::Context::default();
        app.prepare_library_icons(&ctx, 1, false);
        app.load_library_assets(&ctx, 0);
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if delete {
            app.execute_data_action(&crate::DataAction {
                entry_ids: vec![entry.id().to_owned()],
                kind: crate::DataActionKind::EntryRecord,
            });
            assert!(app.entries.is_empty());
        } else {
            let prepared = inspect_import(ImportSource::new(
                None,
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/java-me/conformance.jar"
                ))
                .to_vec(),
                Some(
                    format!("MIDlet-1: Updated, /new.png, {}\n", entry.midlet_class()).into_bytes(),
                ),
            ))
            .unwrap()
            .select_midlet(1)
            .unwrap();
            assert!(app.commit_import(&prepared, GameSettings::default()));
            assert_eq!(app.entries[0].icon_resource(), Some("/new.png"));
        }
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while app.library_assets.busy {
            assert!(Instant::now() < deadline);
            app.receive_library_assets(&ctx).unwrap();
            thread::sleep(Duration::from_millis(1));
        }
        assert!(!app.icons.contains_key(entry.id()));
        assert!(app.display_error.is_none());
    }
}
