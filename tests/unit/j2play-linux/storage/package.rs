use super::*;
use frontend_core::{AppSettings, FpsLimit, GameSettings, ImportSource, inspect_import};
use std::process::{Command, Output};
use std::time::Duration;

#[path = "../../../support/process.rs"]
mod process;

/// Run with two delivered archives in the target userspace. The optional output
/// directory must not exist: it retains only this test's data for a subsequent
/// launch through the installed desktop entry.
#[test]
#[ignore = "requires previous/current Linux archives and the target runtime environment"]
fn delivered_archive_update_preserves_library_settings_and_midlet_rms() {
    let previous = archive_path("J2PLAY_LINUX_PREVIOUS_PACKAGE");
    let current = archive_path("J2PLAY_LINUX_PACKAGE");
    let scratch = Scratch::new();
    let root = std::env::var_os("J2PLAY_LINUX_PACKAGE_CHECK_DIR")
        .map(PathBuf::from)
        .inspect(|path| {
            assert!(path.is_absolute());
            fs::create_dir(path).expect("package check directory must be new");
        })
        .unwrap_or_else(|| scratch.0.clone());
    let data = root.join("data");
    let program = root.join("program % with $ and \"quotes\"");
    let paths = AppPaths {
        data: data.join(APP_ID),
        cache: root.join("cache").join(APP_ID),
    };
    let first = unpack(&previous, &root.join("previous"));
    install(&first, &program, &data, true);
    let original_release = program.join("current").canonicalize().unwrap();
    let original_desktop = desktop(&data);

    let repository = paths.repository(paths.acquire().unwrap()).unwrap();
    let prepared = inspect_import(ImportSource::new(
        None,
        include_bytes!("../../../fixtures/java-me/conformance.jar").to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(5)
    .unwrap();
    let entry = repository
        .commit_import(
            &prepared,
            GameSettings {
                fps_limit: FpsLimit::Manual {
                    frames_per_second: 45,
                },
                ..GameSettings::default()
            },
        )
        .unwrap();
    let settings = AppSettings {
        ui_scale_percent: 125,
        ..AppSettings::default()
    };
    repository.save_app_settings(&settings).unwrap();
    run_rms_midlet(repository.clone(), entry.id());
    let expected_entry = repository.load().unwrap().entries;
    let private_jar = repository.private_jar_path(&entry);
    drop(repository);
    let saved = files_below(&paths.data);
    assert_eq!(
        saved[&private_jar],
        include_bytes!("../../../fixtures/java-me/conformance.jar")
    );
    let rms = files_below(&paths.data.join("library/runtime/rms"));
    assert!(!rms.is_empty(), "the owned MIDlet did not write RMS");

    let next = unpack(&current, &root.join("next"));
    assert_ne!(
        fs::read(first.join("build-id")).unwrap(),
        fs::read(next.join("build-id")).unwrap(),
        "an update needs two different delivered builds"
    );
    let binary = next.join("usr/bin/j2play");
    let executable = fs::read(&binary).unwrap();
    fs::write(&binary, b"damaged package fixture").unwrap();
    install(&next, &program, &data, false);
    assert_eq!(
        program.join("current").canonicalize().unwrap(),
        original_release
    );
    assert_eq!(desktop(&data), original_desktop);
    assert_eq!(files_below(&paths.data), saved);

    fs::write(&binary, executable).unwrap();
    install(&next, &program, &data, true);
    let updated_release = program.join("current").canonicalize().unwrap();
    assert_ne!(updated_release, original_release);
    for release in [&original_release, &updated_release] {
        checked(
            Command::new("sha256sum")
                .args(["-c", "SHA256SUMS"])
                .current_dir(release),
        );
    }
    assert_eq!(files_below(&paths.data), saved);
    // Reinstalling the same delivered archive must remain data-preserving.
    let updated_desktop = desktop(&data);
    install(&next, &program, &data, true);
    assert_eq!(
        program.join("current").canonicalize().unwrap(),
        updated_release
    );
    assert_eq!(desktop(&data), updated_desktop);
    assert_eq!(files_below(&paths.data), saved);

    let repository = paths.repository(paths.acquire().unwrap()).unwrap();
    assert_eq!(repository.load().unwrap().entries, expected_entry);
    assert_eq!(repository.load_app_settings().unwrap(), settings);
    run_rms_midlet(repository, entry.id());
    assert_eq!(files_below(&paths.data.join("library/runtime/rms")), rms);
    eprintln!("Installed package fixture: {}", root.display());
}

fn archive_path(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")));
    assert!(path.is_absolute() && path.is_file(), "invalid {name}");
    path
}

fn unpack(archive: &Path, destination: &Path) -> PathBuf {
    fs::create_dir(destination).unwrap();
    checked(
        Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(destination),
    );
    destination.join("J2Play.AppDir")
}

fn install(source: &Path, program: &Path, data: &Path, success: bool) {
    let output = run(Command::new("sh")
        .arg(source.join("install.sh"))
        .arg(program)
        .env("XDG_DATA_HOME", data));
    assert_eq!(
        output.status.success(),
        success,
        "installer: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn desktop(data: &Path) -> Vec<u8> {
    fs::read(data.join("applications").join(format!("{APP_ID}.desktop"))).unwrap()
}

fn run(command: &mut Command) -> Output {
    let result = process::run(command, Duration::from_secs(30)).unwrap();
    assert!(!result.timed_out, "package command timed out: {command:?}");
    result.output
}

fn checked(command: &mut Command) {
    let output = run(command);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
