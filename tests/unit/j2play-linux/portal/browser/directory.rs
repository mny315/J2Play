use super::*;
use crate::test_storage::Scratch;

#[test]
fn lists_folders_and_matching_regular_files_including_symlinks() {
    let scratch = Scratch::new();
    let path = &scratch.0;
    std::fs::create_dir(path.join("Folder")).unwrap();
    std::fs::write(path.join("Java файл.JAR"), b"jar").unwrap();
    std::fs::write(path.join("descriptor.jad"), b"jad").unwrap();
    std::fs::write(path.join("ignored.txt"), b"text").unwrap();
    std::os::unix::fs::symlink(path.join("Folder"), path.join("Mounted folder")).unwrap();
    std::os::unix::fs::symlink(path.join("Java файл.JAR"), path.join("Linked.jar")).unwrap();
    std::os::unix::fs::symlink("/dev/zero", path.join("special.jar")).unwrap();
    std::os::unix::fs::symlink(path.join("missing"), path.join("broken.jar")).unwrap();
    let directory = read(path.clone(), DocumentKind::Jar, || Ok(())).unwrap();
    assert_eq!(
        directory
            .entries
            .iter()
            .map(|entry| (entry.0.as_str(), entry.2))
            .collect::<Vec<_>>(),
        [
            ("Folder", true),
            ("Mounted folder", true),
            ("Java файл.JAR", false),
            ("Linked.jar", false)
        ]
    );
    let directory = read(path.clone(), DocumentKind::Jad, || Ok(())).unwrap();
    assert_eq!(directory.entries.last().unwrap().0, "descriptor.jad");
    assert!(!directory.truncated);
}

#[test]
fn bounds_directory_inventory_and_honors_cancellation() {
    let scratch = Scratch::new();
    for index in 0..=MAX_ENTRIES {
        std::fs::write(scratch.0.join(format!("{index}.jar")), []).unwrap();
    }
    let directory = read(scratch.0.clone(), DocumentKind::Jar, || Ok(())).unwrap();
    assert_eq!(directory.entries.len(), MAX_ENTRIES);
    assert!(directory.truncated);
    let mut checks = 0;
    let result = read(scratch.0.clone(), DocumentKind::Jar, || {
        checks += 1;
        if checks > 3 {
            Err(error("cancelled", "cancelled"))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.err().unwrap().code(), "cancelled");
    assert_eq!(checks, 4);
}

#[test]
fn folder_errors_do_not_disclose_the_source_path() {
    let scratch = Scratch::new();
    let path = scratch.0.join("private-secret");
    let error = read(path, DocumentKind::Jar, || Ok(())).err().unwrap();
    assert!(!error.to_string().contains("private-secret"));
    assert!(read(PathBuf::from("relative"), DocumentKind::Jar, || Ok(())).is_err());
}
