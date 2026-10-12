use super::*;

use crate::tests::Scratch;

#[test]
fn cache_rejects_corruption_and_changed_method_or_target() {
    let root = Scratch::new();
    let mut cache = Cache::open(&root.0, "test-target").unwrap();
    let spec = IntegerMethodSpec {
        static_integer_parameters: vec![],
        code: vec![0x1a, 0xac],
        integer_constants: vec![],
        parameter_slots: vec![0],
        max_locals: 1,
        max_stack: 1,
    };
    let identity = cache.identity(&spec);
    cache.store(&identity, &[1, 2, 3, 4]).unwrap();
    assert_eq!(cache.load(&identity), Some(vec![1, 2, 3, 4]));
    let mut changed = spec.clone();
    changed.code[0] = 0x03;
    assert!(cache.load(&cache.identity(&changed)).is_none());
    let other = Cache::open(&root.0, "other-target").unwrap();
    assert!(other.load(&other.identity(&spec)).is_none());
    let mut corrupt = fs::read(cache.path(&identity)).unwrap();
    corrupt[33] ^= 1;
    fs::write(cache.path(&identity), corrupt).unwrap();
    assert!(cache.load(&identity).is_none());
}

#[test]
fn replacing_a_cached_artifact_keeps_usage_equal_to_the_directory() {
    let root = Scratch::new();
    let mut cache = Cache::open(&root.0, "test-target").unwrap();
    let identity = [1; 32];
    for length in [64, 128, 16, 64] {
        let code = vec![7; length];
        cache.store(&identity, &code).unwrap();
        assert_eq!(cache.load(&identity).unwrap(), code);
        assert_usage_matches_directory(&cache);
    }
}

#[test]
fn replacing_an_artifact_accounts_for_an_interrupted_pending_file() {
    let root = Scratch::new();
    let mut cache = Cache::open(&root.0, "test-target").unwrap();
    let identity = [1; 32];
    cache.store(&identity, &[7; 64]).unwrap();
    fs::write(cache.path(&identity).with_extension("pending"), [8; 128]).unwrap();
    let mut cache = Cache::open(&root.0, "test-target").unwrap();
    cache.store(&identity, &[9; 16]).unwrap();
    assert_eq!(cache.load(&identity), Some(vec![9; 16]));
    assert_usage_matches_directory(&cache);
}

#[cfg(unix)]
#[test]
fn pending_links_cannot_redirect_cache_writes() {
    for symbolic in [false, true] {
        let root = Scratch::new();
        let directory = root.0.join("cache");
        let mut cache = Cache::open(&directory, "test-target").unwrap();
        let identity = [1; 32];
        cache.store(&identity, &[7; 64]).unwrap();
        let retained = root.0.join("retained");
        fs::write(&retained, b"unrelated data").unwrap();
        let pending = cache.path(&identity).with_extension("pending");
        if symbolic {
            std::os::unix::fs::symlink(&retained, &pending).unwrap();
        } else {
            fs::hard_link(&retained, &pending).unwrap();
        }
        let mut cache = Cache::open(&directory, "test-target").unwrap();
        cache.store(&identity, &[9; 16]).unwrap();
        assert_eq!(fs::read(&retained).unwrap(), b"unrelated data");
        assert_eq!(cache.load(&identity), Some(vec![9; 16]));
        assert!(!pending.exists());
        assert_usage_matches_directory(&cache);
    }
}

#[test]
fn failed_publication_preserves_reads_and_requires_reopening_before_writes() {
    let root = Scratch::new();
    let mut cache = Cache::open(&root.0, "test-target").unwrap();
    let identity = [1; 32];
    cache.store(&identity, &[7; 64]).unwrap();
    let temporary = cache.path(&identity).with_extension("pending");
    fs::create_dir(&temporary).unwrap();
    assert!(cache.store(&identity, &[8; 16]).is_err());
    assert_eq!(cache.load(&identity), Some(vec![7; 64]));
    fs::remove_dir(temporary).unwrap();
    cache.store(&[2; 32], &[9; 16]).unwrap();
    assert!(cache.load(&[2; 32]).is_none());
    let mut reopened = Cache::open(&root.0, "test-target").unwrap();
    reopened.store(&[2; 32], &[9; 16]).unwrap();
    assert_eq!(reopened.load(&[2; 32]), Some(vec![9; 16]));
    assert_usage_matches_directory(&reopened);
}

#[test]
fn a_full_cache_can_repair_an_existing_artifact_without_admitting_a_new_file() {
    let root = Scratch::new();
    let mut cache = Cache::open(&root.0, "test-target").unwrap();
    let identity = [1; 32];
    cache.store(&identity, &[7; 64]).unwrap();
    for index in cache.usage.unwrap().files..MAX_CACHE_FILES {
        fs::File::create(root.0.join(format!("other-artifact-{index}"))).unwrap();
    }
    let mut cache = Cache::open(&root.0, "test-target").unwrap();
    assert_eq!(cache.usage.unwrap().files, MAX_CACHE_FILES);
    cache.store(&identity, &[8; 64]).unwrap();
    assert_eq!(cache.load(&identity), Some(vec![8; 64]));
    cache.store(&[2; 32], &[9; 16]).unwrap();
    assert!(cache.load(&[2; 32]).is_none());
    assert_usage_matches_directory(&cache);
}

#[test]
fn replacing_an_artifact_reuses_its_byte_budget_and_rejects_excess_growth() {
    let root = Scratch::new();
    let mut cache = Cache::open(&root.0, "test-target").unwrap();
    let identity = [1; 32];
    cache.store(&identity, &[7; 64]).unwrap();
    fs::File::create(root.0.join("other-artifact"))
        .unwrap()
        .set_len(MAX_CACHE_BYTES - cache.usage.unwrap().bytes)
        .unwrap();
    let mut cache = Cache::open(&root.0, "test-target").unwrap();
    assert_eq!(cache.usage.unwrap().bytes, MAX_CACHE_BYTES);
    cache.store(&identity, &[8; 65]).unwrap();
    assert_eq!(cache.load(&identity), Some(vec![7; 64]));
    cache.store(&identity, &[8; 16]).unwrap();
    assert_eq!(cache.load(&identity), Some(vec![8; 16]));
    cache.store(&[2; 32], &[9; 16]).unwrap();
    assert_eq!(cache.load(&[2; 32]), Some(vec![9; 16]));
    assert_usage_matches_directory(&cache);
}

fn assert_usage_matches_directory(cache: &Cache) {
    let entries = fs::read_dir(&cache.root)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .collect::<Vec<_>>();
    assert_eq!(cache.usage.unwrap().files, entries.len());
    assert_eq!(cache.usage.unwrap().bytes, entries.iter().sum::<u64>());
}
