use super::*;
use crate::test_storage::Scratch;

#[derive(Debug)]
struct NativeDrop(PathBuf);

impl egui::DroppedFile for NativeDrop {
    fn path(&self) -> &Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        panic!("file bytes must be read by the bounded import worker")
    }
}

fn dropped(path: impl Into<PathBuf>) -> egui::DroppedFileHandle {
    Arc::new(NativeDrop(path.into()))
}

#[test]
fn native_drops_keep_paths_in_the_shell_and_join_the_activation_queue() {
    let scratch = Scratch::new();
    let mut server = OpenFiles::start(&scratch.0, None).unwrap();
    let paths = [
        PathBuf::from("/tmp/Игра % #.JaR"),
        PathBuf::from(OsString::from_vec(b"/tmp/nonutf8-\xff.jar".to_vec())),
    ];
    let mut input = egui::RawInput {
        hovered_files: vec![egui::HoveredFile {
            path: Some(paths[0].clone()),
            ..Default::default()
        }],
        dropped_files: paths.iter().map(|path| dropped(path.clone())).collect(),
        ..Default::default()
    };
    server.drops().take(&mut input);
    assert!(input.dropped_files.is_empty());
    assert!(input.hovered_files.is_empty());
    // No file is read until the shared UI is ready for the next import.
    for path in paths {
        assert_eq!(server.poll(), Some(path));
    }
    assert!(server.poll().is_none());
    assert!(server.poll_drop_error().is_none());
    forward(&scratch.0, Some(Path::new("/tmp/activated.jar"))).unwrap();
    assert_eq!(
        server.poll().as_deref(),
        Some(Path::new("/tmp/activated.jar"))
    );
}

#[test]
fn invalid_drops_and_overflow_report_one_error_without_losing_accepted_files() {
    let scratch = Scratch::new();
    let mut server = OpenFiles::start(&scratch.0, None).unwrap();
    let drops = server.drops();
    for path in ["", "/tmp/notes.txt", "relative.jar"] {
        let mut input = egui::RawInput {
            dropped_files: vec![dropped(path)],
            ..Default::default()
        };
        drops.take(&mut input);
    }
    assert!(server.poll().is_none());
    assert_eq!(server.poll_drop_error().unwrap().code(), "linux-file-drop");
    assert!(server.poll_drop_error().is_none());
    let mut input = egui::RawInput {
        dropped_files: (0..9)
            .map(|index| dropped(format!("/tmp/fixture-{index}.jar")))
            .collect(),
        ..Default::default()
    };
    drops.take(&mut input);
    assert!(server.poll_drop_error().is_some());
    for index in 0..8 {
        assert_eq!(
            server.poll(),
            Some(format!("/tmp/fixture-{index}.jar").into())
        );
    }
    assert!(server.poll().is_none());
    assert!(server.poll_drop_error().is_none());
}

#[test]
fn file_arguments_preserve_names_and_decode_local_uris_only() {
    let path = PathBuf::from("/tmp/Игра % #.JAR");
    let uri = url::Url::from_file_path(&path).unwrap();
    for value in [path.as_os_str().to_owned(), OsString::from(uri.as_str())] {
        assert_eq!(argument([value].into_iter()).unwrap(), Some(path.clone()));
    }
    for value in [
        "https://host/game.jar",
        "file://server/game.jar",
        "--inspect",
        "file:///tmp/a.jar?fetch=1",
    ] {
        assert!(argument([value.into()].into_iter()).is_err());
    }
    assert!(argument(["a.jar".into(), "b.jar".into()].into_iter()).is_err());
    let odd = PathBuf::from(OsString::from_vec(b"/tmp/nonutf8-\xff.jar".to_vec()));
    assert_eq!(
        argument([odd.as_os_str().to_owned()].into_iter()).unwrap(),
        Some(odd)
    );
}

#[test]
fn activation_reaches_the_owner_once_and_queue_overflow_is_reported() {
    let scratch = Scratch::new();
    let data = scratch.0.join("long-xdg-root".repeat(10));
    std::fs::create_dir(&data).unwrap();
    let path = Path::new("/tmp/test with spaces.jar");
    let mut server = OpenFiles::start(&data, Some(path.into())).unwrap();
    assert_eq!(server.poll().as_deref(), Some(path));
    forward(&data, Some(path)).unwrap();
    assert_eq!(server.poll().as_deref(), Some(path));
    assert!(server.poll().is_none());
    forward(&data, None).unwrap();
    assert!(server.focus.load(Ordering::Acquire));
    assert!(server.poll().is_none());
    for _ in 0..8 {
        forward(&data, Some(path)).unwrap();
    }
    assert!(forward(&data, Some(path)).is_err());
    server.shutdown().unwrap();
    drop(server);
    // A stale socket cannot prevent the next lock owner from accepting files.
    let mut replacement = OpenFiles::start(&data, None).unwrap();
    forward(&data, Some(path)).unwrap();
    assert_eq!(replacement.poll().as_deref(), Some(path));
}

#[test]
fn malformed_or_stalled_activation_is_bounded_and_does_not_poison_the_server() {
    let scratch = Scratch::new();
    let mut server = OpenFiles::start(&scratch.0, None).unwrap();
    for payload in [
        vec![255; 4],
        [3_u32.to_le_bytes().as_slice(), b"bad"].concat(),
    ] {
        let mut stream = UnixStream::connect(scratch.0.join("open.sock")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream.write_all(&payload).unwrap();
        let mut ack = [1];
        stream.read_exact(&mut ack).unwrap();
        assert_eq!(ack, [0]);
    }
    let _stalled = UnixStream::connect(scratch.0.join("open.sock")).unwrap();
    thread::sleep(Duration::from_millis(40));
    let start = Instant::now();
    server.shutdown().unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(server.poll().is_none());
}
