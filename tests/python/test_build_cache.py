"""Exercise bounded eviction without touching tools, products or user data."""

import fcntl
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import build_cache


class BuildCacheTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="j2play-cache-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def file(self, relative, data=b"retained fixture"):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return path

    def profile(self, relative, modified=1):
        self.file(relative + "/.cargo-lock", b"")
        artifact = self.file(relative + "/deps/library.rlib", b"x" * 8192)
        os.utime(artifact, ns=(modified, modified))
        return artifact.parent.parent

    def test_incremental_snapshots_are_removed_without_rebuilding_dependencies(self):
        profile = self.profile("target/debug")
        self.file("target/debug/incremental/session/dep-graph.bin", b"x" * 8192)
        before = build_cache.inventory(profile)[0]
        after = build_cache.trim(self.root)
        self.assertLess(after, before)
        self.assertFalse((profile / "incremental").exists())
        self.assertTrue((profile / "deps/library.rlib").is_file())

    def test_oldest_profiles_are_evicted_until_the_shared_budget_is_met(self):
        old = self.profile("linux-build/target/" + "0" * 64 + "/x86_64/release", 1)
        current = self.profile("target/debug", 2)
        budget = build_cache.inventory(current)[0]
        self.assertLessEqual(build_cache.trim(self.root, budget), budget)
        self.assertFalse((old / "deps").exists())
        self.assertTrue((old / ".cargo-lock").is_file())
        self.assertTrue((current / "deps/library.rlib").is_file())

    def test_eviction_uses_artifact_age_after_removing_incremental_snapshots(self):
        old = self.profile("target/release", 1)
        current = self.profile("target/debug", 2)
        snapshot = self.file("target/release/incremental/session/dep-graph.bin", b"x" * 8192)
        os.utime(snapshot, ns=(3, 3))
        budget = build_cache.inventory(current)[0]
        self.assertLessEqual(build_cache.trim(self.root, budget), budget)
        self.assertFalse((old / "incremental").exists())
        self.assertFalse((old / "deps").exists())
        self.assertTrue((current / "deps/library.rlib").is_file())

    def test_eviction_preserves_products_keys_tools_and_user_files(self):
        profile = self.profile("android-build/target/aarch64-linux-android/debug")
        protected = [self.file(name) for name in (
            "android-build/debug.keystore", "android-build/cargo/registry/dependency.crate",
            "target/dev-cargo/registry/dependency.crate", "target/build-inputs/ui/source.rs",
            "linux-build/sdk/compiler", "builds/android/arm64-v8a/j2play.apk",
            "android-build/target/release/apk/j2play.apk", "j2play-data/game.rms",
            "android-build/target/aarch64-linux-android/debug/measurement.log",
        )]
        self.assertLessEqual(build_cache.trim(self.root, 4096), 4096)
        self.assertFalse((profile / "deps").exists())
        for path in protected:
            self.assertEqual(path.read_bytes(), b"retained fixture", path)

    def test_active_cargo_lock_prevents_eviction_and_reports_an_unmet_budget(self):
        profile = self.profile("target/debug")
        with (profile / ".cargo-lock").open("r+b") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            with self.assertRaisesRegex(ValueError, "in use"):
                build_cache.trim(self.root, 0)
            self.assertTrue((profile / "deps/library.rlib").exists())
        self.assertEqual(build_cache.trim(self.root, 0), 0)

    def test_symlinks_and_unrecognized_directories_are_not_followed(self):
        outside = self.profile("user-data/debug")
        (self.root / "target").mkdir()
        (self.root / "target/debug").symlink_to(outside, target_is_directory=True)
        self.profile("target/user-data/debug")
        self.assertEqual(list(build_cache.profiles(self.root)), [])
        build_cache.trim(self.root, 0)
        self.assertTrue((outside / "deps/library.rlib").exists())
        self.assertTrue((self.root / "target/user-data/debug/deps/library.rlib").exists())

    def test_hardlinked_artifacts_are_counted_once(self):
        profile = self.profile("target/debug")
        before = build_cache.inventory(profile)[0]
        os.link(profile / "deps/library.rlib", profile / "test-binary")
        self.assertEqual(build_cache.inventory(profile)[0], before)

    def test_nested_builds_trim_only_at_outer_boundaries_even_after_failure(self):
        with patch.object(build_cache, "ROOT", self.root), \
                patch.object(build_cache, "trim") as trim, \
                patch.dict(os.environ, {build_cache.ACTIVE: "another checkout"}):
            with self.assertRaisesRegex(RuntimeError, "fixture failure"):
                with build_cache.managed_build():
                    with build_cache.managed_build():
                        raise RuntimeError("fixture failure")
            self.assertEqual(trim.call_count, 2)
            self.assertEqual(os.environ[build_cache.ACTIVE], "another checkout")


if __name__ == "__main__":
    unittest.main()
