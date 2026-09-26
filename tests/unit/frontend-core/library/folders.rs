use super::*;
use crate::{GameSettings, ImportSource, inspect_import};

fn prepared() -> crate::PreparedImport {
    inspect_import(ImportSource::new(
        None,
        include_bytes!("../../../fixtures/java-me/conformance.jar").to_vec(),
        None,
    ))
    .unwrap()
    .select_midlet(1)
    .unwrap()
}

#[test]
fn folders_survive_restart_reimport_and_removal_keeps_game_data() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    assert!(repository.load_folders().unwrap().folders().is_empty());
    let mut entry = repository
        .commit_import(&prepared(), GameSettings::default())
        .unwrap();
    let (_, id) = repository.create_folder("  Избранное 😀  ").unwrap();
    repository.move_to_folder(&mut entry, Some(id)).unwrap();
    let reopened = LibraryRepository::open(&scratch.0).unwrap();
    let folders = reopened.load_folders().unwrap();
    assert_eq!(folders.get(id).unwrap().name(), "Избранное 😀");
    assert_eq!(
        folders.folder_for(&reopened.load().unwrap().entries[0]),
        Some(id)
    );
    let reimported = reopened
        .commit_import(&prepared(), GameSettings::default())
        .unwrap();
    assert_eq!(folders.folder_for(&reimported), Some(id));
    let renamed = reopened.rename_folder(id, "Аркады").unwrap();
    assert_eq!(renamed.folder_for(&reimported), Some(id));
    let archive_path = reopened.archives_dir().join(&entry.private_jar);
    let archive = std::fs::read(&archive_path).unwrap();
    let rms = reopened.rms_root(&entry);
    std::fs::create_dir_all(&rms).unwrap();
    let data_path = rms.join("score");
    std::fs::write(&data_path, b"user data").unwrap();
    let folders = reopened.delete_folder(id).unwrap();
    assert_eq!(folders.folder_for(&entry), None);
    assert_eq!(reopened.load().unwrap().entries.len(), 1);
    assert_eq!(std::fs::read(archive_path).unwrap(), archive);
    assert_eq!(std::fs::read(data_path).unwrap(), b"user data");
    let (folders, new_id) = reopened.create_folder("Аркады").unwrap();
    assert_ne!(id, new_id);
    assert_eq!(folders.folder_for(&entry), None);
    reopened.move_to_folder(&mut entry, None).unwrap();
    assert_eq!(entry.folder_id, None);
}

#[test]
fn invalid_names_and_missing_targets_do_not_change_existing_metadata() {
    let scratch = crate::test_storage::Scratch::new();
    let root = scratch.0.join("library");
    let repository = LibraryRepository::open(&root).unwrap();
    let (_, id) = repository.create_folder("Puzzle").unwrap();
    let before = std::fs::read(root.join("folders.json")).unwrap();
    for name in ["", " \t ", "puzzle", " Puzzle ", "a\nb", &"x".repeat(65)] {
        assert!(repository.create_folder(name).is_err(), "{name:?}");
    }
    assert!(repository.rename_folder(id + 1, "Other").is_err());
    assert!(repository.delete_folder(id + 1).is_err());
    assert_eq!(std::fs::read(root.join("folders.json")).unwrap(), before);
    let mut entry = repository
        .commit_import(&prepared(), GameSettings::default())
        .unwrap();
    let previous = entry.clone();
    assert!(repository.move_to_folder(&mut entry, Some(id + 1)).is_err());
    assert_eq!(entry, previous);
    // A folder name is a label, even when it resembles a host path.
    repository.create_folder("../outside").unwrap();
    assert!(!scratch.0.join("outside").exists());
}

#[test]
fn corrupt_or_oversized_catalog_cannot_hide_games_or_be_overwritten_by_edits() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    repository
        .commit_import(&prepared(), GameSettings::default())
        .unwrap();
    for bytes in [
        b"{broken".to_vec(),
        vec![b' '; usize::try_from(MAX_FOLDERS_BYTES + 1).unwrap()],
    ] {
        std::fs::write(scratch.0.join("folders.json"), &bytes).unwrap();
        assert!(repository.load_folders().is_err());
        assert!(repository.create_folder("New").is_err());
        assert_eq!(repository.load().unwrap().entries.len(), 1);
        assert_eq!(
            std::fs::read(scratch.0.join("folders.json")).unwrap(),
            bytes
        );
    }
}

#[test]
fn failed_move_preserves_entry_and_catalog_bounds_are_validated() {
    let scratch = crate::test_storage::Scratch::new();
    let repository = LibraryRepository::open(&scratch.0).unwrap();
    let mut entry = repository
        .commit_import(&prepared(), GameSettings::default())
        .unwrap();
    let (_, id) = repository.create_folder("Games").unwrap();
    let original = entry.clone();
    let path = repository
        .entries_dir()
        .join(format!("{}.json", entry.id()));
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(repository.move_to_folder(&mut entry, Some(id)).is_err());
    assert_eq!(entry, original);
    let mut catalog = LibraryFolders {
        next_id: 1000,
        folders: (1..=u64::try_from(MAX_LIBRARY_FOLDERS).unwrap())
            .map(|id| LibraryFolder {
                id,
                name: format!("Folder {id}"),
            })
            .collect(),
        ..LibraryFolders::default()
    };
    repository.write_folders(&mut catalog).unwrap();
    assert_eq!(
        repository.create_folder("Overflow").unwrap_err().code(),
        "library-folder-capacity"
    );
    catalog.folders[1].id = catalog.folders[0].id;
    assert!(catalog.validate().is_err());
}
