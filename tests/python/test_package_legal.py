"""Check dependency selection, omitted upstream licenses and atomic APK assets."""

import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
import package_legal


class PackageLegalTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.addCleanup(patch.stopall)
        patch.object(package_legal, "ROOT", self.root).start()
        patch.object(package_legal, "SOURCES", self.root / "legal/sources.json").start()
        for path, data in {
            "LICENSE": "project MIT", "NOTICE": "project notices",
            "legal/THIRD_PARTY_NOTICES.txt": "shared font/native notices",
            "legal/Unicode-3.0.txt": "Unicode text",
            "legal/sources.json": '{"schema": 1, "licenses": []}',
            "crates/frontend-ui/build-support/sources.json": '{"crates": []}',
            "crates/frontend-ui/assets/fonts/OFL.txt": "font license",
            "crates/vm/assets/fonts/README.md": "font provenance",
            "crates/frontend-ui/assets/artwork.provenance.md": "art provenance",
            "crates/frontend-ui/assets/gamepad-body.provenance.md": "gamepad provenance",
            "Cargo.lock": '[[package]]\nname="linked"\nversion="1.0.0"\nchecksum="' + "a" * 64 + '"\n',
            "flake.lock": "{}",
            "registry/linked/COPYRIGHT": "upstream attribution",
            "registry/linked/LICENSE": "upstream license",
        }.items():
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(data)
        self.metadata = {
            "workspace_members": ["app"],
            "packages": [
                {"id": "app", "name": "app", "version": "1.0.0"},
                {"id": "linked", "name": "linked", "version": "1.0.0", "license": "MIT",
                 "manifest_path": str(self.root / "registry/linked/Cargo.toml")},
                {"id": "test-only", "name": "test-only", "version": "1.0.0"},
            ],
        }
        self.output = self.root / "output"

    def collect(self):
        with patch.object(package_legal.subprocess, "check_output", side_effect=[
                "app v1.0.0\nlinked v1.0.0\nlinked v1.0.0 (*)\n", json.dumps(self.metadata)]):
            return package_legal.cargo_notices(self.output, "cargo", {}, "app", "target",
                                               self.root / "cache", offline=True)

    def test_actual_graph_excludes_workspace_and_test_dependencies_and_retains_attribution(self):
        entries = self.collect()
        self.assertEqual(len(entries), 1)
        self.assertEqual(entries[0]["source"], "https://static.crates.io/crates/linked/linked-1.0.0.crate")
        self.assertEqual(entries[0]["sha256"], "a" * 64)
        self.assertEqual((self.output / "linked-1.0.0/COPYRIGHT").read_text(), "upstream attribution")
        self.assertEqual((self.output / "NOTICE").read_text(), "project notices")
        self.assertTrue((self.output / "crates/vm/assets/fonts/README.md").is_file())

    def test_missing_upstream_text_fails_packaging(self):
        for path in (self.root / "registry/linked").iterdir():
            path.unlink()
        with self.assertRaisesRegex(ValueError, "linked-1.0.0"):
            self.collect()

    def test_pinned_supplement_is_included_even_when_other_notices_exist(self):
        source = {"url": "https://example.org/LICENSE", "sha256": "b" * 64,
                  "crates": {"linked": "1.0.0"}}
        (self.root / "legal/sources.json").write_text(json.dumps({"schema": 1, "licenses": [source]}))
        with patch.object(package_legal.sources, "download", return_value=b"workspace license") as fetch:
            self.collect()
        fetch.assert_called_once_with(source, self.root / "cache" / ("b" * 64), True)
        self.assertEqual((self.output / "linked-1.0.0/UPSTREAM-LICENSE-0.txt").read_text(), "workspace license")

    def test_failed_android_collection_keeps_previous_assets_and_success_drops_stale_notices(self):
        old = self.root / "android-build/assets/legal/obsolete"
        old.parent.mkdir(parents=True)
        old.write_text("previous assets")
        # copytree preserves read-only directories from the Nix Rust toolchain.
        old.parent.chmod(0o555)

        def collect(destination, *_args):
            destination.mkdir(parents=True)
            (destination / "NOTICE").write_text("current notices")
            return [{}]

        rust, ndk = self.root / "rust", self.root / "ndk"
        with patch.object(package_legal, "cargo_notices", side_effect=collect):
            with self.assertRaises(FileNotFoundError):
                package_legal.android_assets(rust, ndk, True)
            self.assertEqual(old.read_text(), "previous assets")
            docs = rust / "share/doc/rust"
            (docs / "licenses").mkdir(parents=True)
            (docs / "licenses/MIT.txt").write_text("runtime MIT")
            (docs / "COPYRIGHT-library.html").write_text("Rust notices")
            ndk.mkdir()
            for name in ("NOTICE", "NOTICE.toolchain", "source.properties"):
                (ndk / name).write_text(name)
            package_legal.android_assets(rust, ndk, True)
        self.assertFalse(old.exists())
        self.assertEqual((old.parent / "NOTICE").read_text(), "current notices")
        self.assertEqual((old.parent / "ndk/NOTICE").read_text(), "NOTICE")

    def test_android_publication_failure_restores_previous_assets(self):
        for failure in (OSError("publication failed"), KeyboardInterrupt()):
            with self.subTest(failure=type(failure).__name__):
                self.check_android_publication_failure(failure, restore_fails=False)

    def test_android_failed_restore_retains_previous_assets_for_recovery(self):
        self.check_android_publication_failure(OSError("publication failed"), restore_fails=True)

    def check_android_publication_failure(self, failure, restore_fails):
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            root = Path(temporary)
            old = root / "android-build/assets/legal/NOTICE"
            old.parent.mkdir(parents=True)
            old.write_text("previous assets")
            rust, ndk = root / "rust", root / "ndk"
            docs = rust / "share/doc/rust"
            (docs / "licenses").mkdir(parents=True)
            (docs / "COPYRIGHT-library.html").write_text("Rust notices")
            ndk.mkdir()
            for name in ("NOTICE", "NOTICE.toolchain", "source.properties"):
                (ndk / name).write_text(name)
            for relative in ("Cargo.lock", "flake.lock", "crates/frontend-ui/build-support/sources.json"):
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((self.root / relative).read_bytes())

            def collect(destination, *_args):
                destination.mkdir(parents=True)
                (destination / "NOTICE").write_text("current notices")
                return [{}]

            rename = Path.rename
            previous = None

            def publish(source, destination):
                nonlocal previous
                if source == old.parents[1]:
                    previous = destination
                elif source == previous:
                    if restore_fails:
                        raise OSError("restore failed")
                elif destination == old.parents[1]:
                    raise failure
                return rename(source, destination)

            with patch.object(package_legal, "ROOT", root), \
                    patch.object(package_legal, "cargo_notices", side_effect=collect), \
                    patch.object(Path, "rename", publish):
                with self.assertRaises(OSError if restore_fails else type(failure)) as raised:
                    package_legal.android_assets(rust, ndk, True)
            self.assertIsNotNone(previous)
            retained = previous / "legal/NOTICE" if restore_fails else old
            self.assertTrue(retained.is_file(), "the last successful assets must survive cleanup")
            self.assertEqual(retained.read_text(), "previous assets")
            if restore_fails:
                self.assertIn(str(previous), str(raised.exception))


if __name__ == "__main__":
    unittest.main()
