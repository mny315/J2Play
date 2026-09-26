use super::*;
use crate::test_storage::Scratch;
use std::os::unix::fs::PermissionsExt;

#[path = "storage/package.rs"]
mod package;

#[test]
fn xdg_overrides_and_flatpak_roots_do_not_depend_on_the_working_directory() {
    let paths = AppPaths::resolve(|name| match name {
        "XDG_DATA_HOME" => Some("/home/user/.var/app/io.github.mny315.j2play/data".into()),
        "XDG_CACHE_HOME" => Some("/home/user/.var/app/io.github.mny315.j2play/cache".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(
        paths.data,
        Path::new("/home/user/.var/app/io.github.mny315.j2play/data").join(APP_ID)
    );
    assert_eq!(
        paths.cache,
        Path::new("/home/user/.var/app/io.github.mny315.j2play/cache").join(APP_ID)
    );
}

#[test]
fn invalid_xdg_paths_fall_back_to_home_and_missing_home_fails_closed() {
    for invalid in ["", "relative/path"] {
        let paths = AppPaths::resolve(|name| match name {
            "HOME" => Some("/home/fixture".into()),
            _ => Some(invalid.into()),
        })
        .unwrap();
        assert_eq!(
            paths.data,
            Path::new("/home/fixture/.local/share").join(APP_ID)
        );
        assert_eq!(paths.cache, Path::new("/home/fixture/.cache").join(APP_ID));
    }
    assert!(AppPaths::resolve(|_| None).is_err());
    assert!(AppPaths::resolve(|_| Some("relative".into())).is_err());
}

#[test]
fn instance_lock_excludes_a_second_writer_and_releases_without_deleting_data() {
    let scratch = Scratch::new();
    let paths = AppPaths {
        data: scratch.0.join("data"),
        cache: scratch.0.join("cache"),
    };
    let guard = paths.acquire().unwrap();
    fs::write(paths.data.join("save.rms"), b"keep fixture data").unwrap();
    assert_eq!(
        paths.acquire().err().unwrap().code(),
        "linux-already-running"
    );
    assert_eq!(
        fs::metadata(&paths.data).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(paths.data.join("instance.lock"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    drop(guard);
    let _reopened = paths.acquire().unwrap();
    assert_eq!(
        fs::read(paths.data.join("save.rms")).unwrap(),
        b"keep fixture data"
    );
    assert!(paths.data.join("instance.lock").is_file());
}

#[test]
fn inaccessible_root_is_an_error_and_does_not_fall_back_to_the_checkout() {
    let scratch = Scratch::new();
    let data = scratch.0.join("file");
    fs::write(&data, b"fixture").unwrap();
    let paths = AppPaths {
        data,
        cache: scratch.0.join("cache"),
    };
    assert_eq!(
        paths.acquire().err().unwrap().code(),
        "linux-data-directory"
    );
    assert!(!paths.cache.exists());
}

#[test]
fn worker_repository_retains_the_lock_after_the_window_owner_is_dropped() {
    let scratch = Scratch::new();
    let paths = AppPaths {
        data: scratch.0.join("data"),
        cache: scratch.0.join("cache"),
    };
    let repository = paths.repository(paths.acquire().unwrap()).unwrap();
    let worker_repository = repository.clone();
    drop(repository);
    assert_eq!(
        paths.acquire().err().unwrap().code(),
        "linux-already-running"
    );
    drop(worker_repository);
    assert!(paths.acquire().is_ok());
}

#[test]
fn library_and_settings_reopen_after_the_instance_and_cache_are_replaced() {
    use frontend_core::{AppSettings, GameSettings, ImportSource, inspect_import};
    let scratch = Scratch::new();
    let paths = AppPaths {
        data: scratch.0.join("data"),
        cache: scratch.0.join("cache"),
    };
    let guard = paths.acquire().unwrap();
    let repository = paths.repository(guard).unwrap();
    let defaults = repository.load_app_settings().unwrap();
    assert!(!defaults.control_layout.visible);
    assert!(!defaults.landscape_control_layout.visible);
    let prepared = inspect_import(ImportSource::new(
        None,
        include_bytes!("../../fixtures/java-me/conformance.jar").to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap();
    let entry = repository
        .commit_import(&prepared, GameSettings::default())
        .unwrap();
    let settings = AppSettings {
        ui_scale_percent: 125,
        ..AppSettings::default()
    };
    repository.save_app_settings(&settings).unwrap();
    drop(repository);
    fs::remove_dir_all(&paths.cache).unwrap(); // Only this test's exclusively owned cache.
    let guard = paths.acquire().unwrap();
    let reopened = paths.repository(guard).unwrap();
    assert_eq!(reopened.load().unwrap().entries, vec![entry]);
    assert_eq!(reopened.load_app_settings().unwrap(), settings);
}

#[test]
fn a_real_midlet_reopens_its_rms_from_xdg_data_after_worker_restart() {
    let scratch = Scratch::new();
    let paths = AppPaths {
        data: scratch.0.join("data"),
        cache: scratch.0.join("cache"),
    };
    let repository = paths.repository(paths.acquire().unwrap()).unwrap();
    let prepared = frontend_core::inspect_import(frontend_core::ImportSource::new(
        None,
        include_bytes!("../../fixtures/java-me/conformance.jar").to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(5)
    .unwrap();
    let entry = repository
        .commit_import(&prepared, frontend_core::GameSettings::default())
        .unwrap();
    run_rms_midlet(repository.clone(), entry.id());
    let data = files_below(&paths.data.join("library/runtime/rms"));
    assert!(
        !data.is_empty(),
        "fixture did not persist RMS below XDG data"
    );
    drop(repository);
    fs::remove_dir_all(&paths.cache).unwrap(); // Exclusively owned fixture cache.
    let repository = paths.repository(paths.acquire().unwrap()).unwrap();
    run_rms_midlet(repository, entry.id());
    assert_eq!(files_below(&paths.data.join("library/runtime/rms")), data);
}

fn run_rms_midlet(repository: LibraryRepository, id: &str) {
    use frontend_core::{RuntimeWorker, SessionController, SessionEventKind};
    use std::time::{Duration, Instant};
    let mut worker = RuntimeWorker::spawn(repository).unwrap();
    worker
        .submit(SessionController::default().start(id).unwrap())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let events = worker.poll_events().unwrap();
        for event in events {
            match event.kind {
                SessionEventKind::Stopped => {
                    worker.shutdown().unwrap();
                    return;
                }
                SessionEventKind::TerminalError { .. } | SessionEventKind::HostRequest(_) => {
                    panic!("RMS fixture failed: {event:?}")
                }
                _ => {}
            }
        }
        assert!(Instant::now() < deadline, "RMS fixture did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn files_below(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(files_below(&path));
        } else {
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}
