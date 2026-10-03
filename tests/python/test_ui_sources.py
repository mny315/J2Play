"""Exercise source integrity, atomic preparation and Cargo override ownership."""

import hashlib
import io
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import prepare_ui_sources as ui_sources


class LockedUiSourcesTest(unittest.TestCase):
    def test_source_archives_match_the_workspace_lockfile(self):
        crates, _ = ui_sources.read_manifest(ui_sources.MANIFEST)
        locked = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
        for crate in crates:
            with self.subTest(crate=crate["name"]):
                packages = [item for item in locked if item["name"] == crate["name"]]
                self.assertEqual(len(packages), 1)
                self.assertEqual(packages[0]["version"], crate["version"])
                if crate.get("cargo_override") == "patch":
                    self.assertNotIn("source", packages[0])
                else:
                    self.assertEqual(packages[0]["checksum"], crate["sha256"])
                    self.assertEqual(packages[0]["source"], "registry+https://github.com/rust-lang/crates.io-index")


class UiSourcesTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="j2play-ui-inputs-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name) / "source checkout with spaces"
        self.root.mkdir()
        self.archives = self.root / "archives"
        self.archives.mkdir()
        self.output = self.root / "output"
        self.manifest = self.root / "sources.json"
        self.patch = self.root / "value.patch"
        self.crate = {"name": "ui-probe", "version": "1.0.0", "patches": [self.patch.name]}
        self.make_archive({
            "ui-probe-1.0.0/Cargo.toml": b'[package]\nname = "ui-probe"\nversion = "1.0.0"\nedition = "2024"\n',
            "ui-probe-1.0.0/src/lib.rs": b'pub const VALUE: u8 = 1;\n',
        })
        self.change_patch(2)

    def make_archive(self, files):
        archive = self.archives / "ui-probe-1.0.0.crate"
        with tarfile.open(archive, mode="w:gz") as contents:
            for name, data in files.items():
                member = tarfile.TarInfo(name)
                if isinstance(data, bytes):
                    member.size = len(data)
                    contents.addfile(member, io.BytesIO(data))
                else:
                    member.type = data
                    member.linkname = "../../outside"
                    contents.addfile(member)
        self.crate["sha256"] = hashlib.sha256(archive.read_bytes()).hexdigest()
        self.manifest.write_text(json.dumps({"schema": 1, "crates": [self.crate]}))
        return archive

    def change_patch(self, value):
        self.patch.write_text(
            "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n"
            f"-pub const VALUE: u8 = 1;\n+pub const VALUE: u8 = {value};\n"
        )

    def prepare(self):
        return ui_sources.prepare_sources(self.manifest, self.output, self.archives, offline=True)

    def test_offline_preparation_is_reproducible_and_reuses_verified_sources(self):
        with patch.object(ui_sources.urllib.request, "urlopen", side_effect=AssertionError("network")):
            first = self.prepare()
            self.assertIn("VALUE: u8 = 2", (first / "ui-probe/src/lib.rs").read_text())
            self.assertEqual(self.prepare(), first)
            other = ui_sources.prepare_sources(
                self.manifest, self.root / "other", self.archives, offline=True,
            )
        self.assertEqual(ui_sources.inventory(first), ui_sources.inventory(other))
        self.assertEqual((first / ui_sources.STAMP).read_bytes(), (other / ui_sources.STAMP).read_bytes())

    def test_checksum_failure_publishes_no_sources(self):
        (self.archives / "ui-probe-1.0.0.crate").write_bytes(b"corrupt cache")
        with self.assertRaisesRegex(ValueError, "checksum"):
            self.prepare()
        self.assertEqual(list(self.output.iterdir()), [self.output / ".lock"])

    def test_download_checks_bounds_and_hash_before_caching(self):
        archive = self.archives / "ui-probe-1.0.0.crate"
        valid = archive.read_bytes()
        archive.unlink()
        for data, limit, error in ((b"corrupt", ui_sources.MAX_ARCHIVE, "checksum"),
                                   (valid, len(valid) - 1, "size limit")):
            with self.subTest(error=error):
                with patch.object(ui_sources.urllib.request, "urlopen", return_value=io.BytesIO(data)):
                    with patch.object(ui_sources, "MAX_ARCHIVE", limit):
                        with self.assertRaisesRegex(ValueError, error):
                            ui_sources.archive_bytes(self.crate, self.archives, offline=False)
                self.assertFalse(archive.exists())
        with patch.object(ui_sources.urllib.request, "urlopen", return_value=io.BytesIO(valid)) as download:
            self.assertEqual(ui_sources.archive_bytes(self.crate, self.archives, offline=False), valid)
        self.assertEqual(download.call_count, 1)
        self.assertEqual(archive.read_bytes(), valid)
        with patch.object(ui_sources.urllib.request, "urlopen", side_effect=AssertionError("network")):
            self.assertEqual(ui_sources.archive_bytes(self.crate, self.archives, offline=True), valid)

    def test_extraction_uses_the_verified_archive_snapshot(self):
        verified = ui_sources.archive_bytes(self.crate, self.archives, offline=True)
        self.make_archive({
            "ui-probe-1.0.0/Cargo.toml": b"[package]\n",
            "ui-probe-1.0.0/injected": b"not the verified archive",
        })
        destination = self.root / "extracted"
        ui_sources.extract_crate(verified, self.crate, destination)
        self.assertFalse((destination / "injected").exists())
        self.assertIn("VALUE: u8 = 1", (destination / "src/lib.rs").read_text())

    def test_preparation_applies_the_patches_used_for_its_fingerprint(self):
        _, fingerprint = ui_sources.read_manifest(self.manifest)
        original = ui_sources.archive_bytes

        def fetch(*args):
            self.change_patch(3)
            return original(*args)

        with patch.object(ui_sources, "archive_bytes", side_effect=fetch):
            sources = self.prepare()
        self.assertEqual(sources.name, fingerprint)
        self.assertIn("VALUE: u8 = 2", (sources / "ui-probe/src/lib.rs").read_text())
        fresh = self.prepare()
        self.assertNotEqual(fresh, sources)
        self.assertIn("VALUE: u8 = 3", (fresh / "ui-probe/src/lib.rs").read_text())

    def test_patch_failure_does_not_replace_previous_sources_or_cargo_config(self):
        first = self.prepare()
        home = self.root / "cargo"
        ui_sources.configure_cargo(first, home)
        config = (home / "config.toml").read_bytes()
        self.patch.write_text("--- a/missing.rs\n+++ b/missing.rs\n@@ -1 +1 @@\n-old\n+new\n")
        with self.assertRaises(subprocess.CalledProcessError):
            self.prepare()
        self.assertEqual((home / "config.toml").read_bytes(), config)
        self.assertTrue((first / "ui-probe/src/lib.rs").is_file())
        self.assertFalse(list(self.output.glob(".prepare-*")))

    def test_rejects_path_escape_links_duplicates_and_missing_manifest(self):
        cases = (
            {"ui-probe-1.0.0/../../outside": b"no"},
            {"/tmp/outside": b"no"},
            {"another-crate/Cargo.toml": b"no"},
            {"ui-probe-1.0.0/link": tarfile.SYMTYPE},
            {"ui-probe-1.0.0/link": tarfile.LNKTYPE},
            {"ui-probe-1.0.0/file": b"a", "ui-probe-1.0.0/./file": b"b"},
            {"ui-probe-1.0.0/src/lib.rs": b"no manifest"},
        )
        for files in cases:
            with self.subTest(files=files):
                self.make_archive(files)
                with self.assertRaises(ValueError):
                    self.prepare()
                self.assertFalse((self.root / "outside").exists())

    def test_expanded_size_limit_applies_before_writing_large_members(self):
        with patch.object(ui_sources, "MAX_EXPANDED", 1):
            with self.assertRaisesRegex(ValueError, "expanded size"):
                self.prepare()

    def test_changed_cached_source_is_rejected(self):
        first = self.prepare()
        (first / "ui-probe/src/lib.rs").write_text("modified")
        with self.assertRaisesRegex(ValueError, "do not match"):
            self.prepare()

    def test_nested_stamp_names_are_part_of_source_integrity(self):
        sources = self.prepare()
        (sources / "ui-probe" / ui_sources.STAMP).write_text("unexpected source")
        with self.assertRaisesRegex(ValueError, "do not match"):
            self.prepare()

    def test_cache_special_files_and_size_limits_fail_without_waiting(self):
        sources = self.prepare()
        fifo = sources / "ui-probe/fifo"
        os.mkfifo(fifo)
        # A regression to blocking open() must fail this test on a deadline.
        # The parent owns the temporary tree; subprocess.run kills and reaps.
        for command, error in (
            ([sys.executable, str(ROOT / "tools/prepare_ui_sources.py"),
              "--manifest", str(self.manifest), "--output", str(self.output),
              "--archive-dir", str(self.archives), "--offline"], "regular files"),
            ([sys.executable, "-c",
              "import sys; from pathlib import Path; sys.path.insert(0, sys.argv[1]); "
              "from prepare_ui_sources import read_bounded_file; "
              "read_bounded_file(Path(sys.argv[2]), 10)",
              str(ROOT / "tools"), str(fifo)], "regular UI input file"),
        ):
            with self.subTest(operation=error):
                result = subprocess.run(command, capture_output=True, text=True,
                                        timeout=3, check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(error, result.stderr)
        fifo.unlink()
        with patch.object(ui_sources, "MAX_EXPANDED", 1):
            with self.assertRaisesRegex(ValueError, "size limit"):
                self.prepare()
        with patch.object(ui_sources, "MAX_FILES", 1):
            with self.assertRaisesRegex(ValueError, "file count"):
                self.prepare()

    def test_new_patch_changes_absolute_cargo_path_and_keeps_previous_generation(self):
        first = self.prepare()
        home = self.root / 'cargo with "quotes"'
        extra = self.root / "android-winit"
        extra.mkdir()
        (extra / "Cargo.toml").write_text("[package]\n")
        ui_sources.configure_cargo(first, home, [extra])
        first_config = tomllib.loads((home / "config.toml").read_text())
        self.assertEqual(first_config["paths"], [str(first / "ui-probe"), str(extra)])
        self.change_patch(3)
        second = self.prepare()
        ui_sources.configure_cargo(second, home, [extra])
        second_config = tomllib.loads((home / "config.toml").read_text())
        self.assertNotEqual(first_config, second_config)
        self.assertIn("VALUE: u8 = 2", (first / "ui-probe/src/lib.rs").read_text())
        self.assertIn("VALUE: u8 = 3", (second / "ui-probe/src/lib.rs").read_text())

    def test_custom_cargo_configuration_is_preserved(self):
        first = self.prepare()
        home = self.root / "cargo"
        home.mkdir()
        config = home / "config.toml"
        for original in ('[net]\noffline = true\n', 'paths = ["/custom/overrides"]\n'):
            with self.subTest(original=original):
                config.write_text(original)
                with self.assertRaisesRegex(ValueError, "custom Cargo"):
                    ui_sources.configure_cargo(first, home)
                self.assertEqual(config.read_text(), original)

    def test_legacy_android_override_is_retained_during_migration(self):
        first = self.prepare()
        home = self.root / "cargo"
        home.mkdir()
        extra = self.root / "winit"
        extra.mkdir()
        (extra / "Cargo.toml").write_text("[package]\n")
        (home / "config.toml").write_text("paths = " + json.dumps([str(extra)]) + "\n")
        ui_sources.configure_cargo(first, home, [extra])
        paths = tomllib.loads((home / "config.toml").read_text())["paths"]
        self.assertEqual(paths, [str(first / "ui-probe"), str(extra)])

    def test_dependency_patch_is_checked_and_moves_with_its_generation(self):
        self.output = self.root / "исходники🙂"
        self.crate["cargo_override"] = "patch"
        self.manifest.write_text(json.dumps({"schema": 1, "crates": [self.crate]}))
        first = self.prepare()
        home = self.root / "cargo"
        ui_sources.configure_cargo(first, home, patch_names=["ui-probe"])
        config = home / "config.toml"
        settings = tomllib.loads(config.read_text())
        self.assertEqual(settings["paths"], [])
        self.assertEqual(settings["patch"]["crates-io"]["ui-probe"]["path"], str(first / "ui-probe"))
        self.assertEqual(ui_sources.check_cargo_home(self.manifest, home), first)
        self.change_patch(3)
        second = self.prepare()
        with self.assertRaises(ValueError):
            ui_sources.check_cargo_home(self.manifest, home)
        ui_sources.configure_cargo(second, home, patch_names=["ui-probe"])
        self.assertEqual(ui_sources.check_cargo_home(self.manifest, home), second)
        for invalid in ('paths = []\n', 'paths = []\npatch = 3\n',
                        'paths = []\n[patch.crates-io.ui-probe]\npath = 3\n'):
            config.write_text(invalid)
            with self.assertRaisesRegex(ValueError, "patches"):
                ui_sources.check_cargo_home(self.manifest, home)

    def test_custom_dependency_patch_is_never_replaced(self):
        sources = self.prepare()
        home = self.root / "cargo"
        home.mkdir()
        config = home / "config.toml"
        for original in (
            'paths = []\n[patch.crates-io.ui-probe]\npath = "/custom"\n',
            ui_sources.CONFIG_HEADER + 'paths = []\n[patch.crates-io.unowned]\npath = "/custom"\n',
        ):
            config.write_text(original)
            with self.assertRaisesRegex(ValueError, "custom Cargo"):
                ui_sources.configure_cargo(sources, home, patch_names=["ui-probe"])
            self.assertEqual(config.read_text(), original)

    def copy_entrypoints(self):
        support = self.root / "crates/frontend-ui/build-support"
        support.mkdir(parents=True)
        shutil.copy2(self.manifest, support / "sources.json")
        shutil.copy2(self.patch, support / self.patch.name)
        (self.root / "tools").mkdir()
        for name in ("prepare_ui_sources.py", "build_outputs.py", "build_cache.py", "check.py",
                     "check_catalog.py", "check_report.py"):
            shutil.copy2(ROOT / "tools" / name, self.root / "tools" / name)
        for name in ("dev.sh",):
            shutil.copy2(ROOT / name, self.root / name)
        return support

    def test_run_and_gates_reject_stale_or_missing_overrides_before_cargo(self):
        sources = self.prepare()
        home = self.root / "cargo"
        ui_sources.configure_cargo(sources, home)
        self.assertEqual(ui_sources.check_cargo_home(self.manifest, home), sources)
        config = home / "config.toml"
        valid_config = config.read_text()
        support = self.copy_entrypoints()
        binary_dir = self.root / "bin"
        binary_dir.mkdir()
        cargo = binary_dir / "cargo"
        cargo.write_text('#!/bin/sh\nprintf "CARGO_INVOKED\\n"\n')
        cargo.chmod(0o755)
        environment = {**os.environ, "CARGO_HOME": str(home), "J2PLAY_DEV_SHELL": "1",
                       "PATH": f"{binary_dir}:{os.environ['PATH']}"}
        commands = ([sys.executable, str(self.root / "tools/check.py"), "conformance", "--verbose"],)
        for command in commands:
            result = subprocess.run(command, cwd=self.root.parent, env=environment,
                                    capture_output=True, text=True, timeout=10, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("CARGO_INVOKED", result.stdout)
        commands += ([str(self.root / "dev.sh"), "run"], [str(self.root / "dev.sh"), "linux"],
                     [sys.executable, str(self.root / "tools/check.py"), "host"])
        for cause in ("missing", "empty", "stale", "corrupt"):
            config.write_text(valid_config)
            shutil.copy2(self.patch, support / self.patch.name)
            if cause == "missing":
                config.unlink()
            elif cause == "empty":
                config.write_text("paths = []\n")
            elif cause == "stale":
                (support / self.patch.name).write_text("changed tracked patch")
            else:
                (sources / "ui-probe/src/lib.rs").write_text("corrupted source")
            for command in commands:
                with self.subTest(cause=cause, command=command):
                    result = subprocess.run(command, cwd=self.root.parent, env=environment,
                                            capture_output=True, text=True, timeout=10, check=False)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertNotIn("CARGO_INVOKED", result.stdout)
                    self.assertIn("Cannot prepare J2Play UI sources", result.stdout + result.stderr)

    def test_nix_hook_failure_stops_the_command_after_shell_setup(self):
        sources = self.prepare()
        self.copy_entrypoints()
        home = self.root / "target/dev-cargo"
        home.mkdir(parents=True)
        config = home / "config.toml"
        original = 'paths = ["/custom/overrides"]\n'
        config.write_text(original)
        hook = (ROOT / "flake.nix").read_text().split("shellHook = ''", 1)[1].split("'';", 1)[0]
        hook = hook.replace("${uiSourcesFor pkgs}", shlex.quote(str(sources)))
        hook_path = self.root / "shell-hook.sh"
        hook_path.write_text(hook)
        result = subprocess.run(
            ["bash", "-c", '. "$1"; printf "BUILD_STARTED\\n"', "bash", str(hook_path)],
            cwd=self.root, capture_output=True, text=True, timeout=10, check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("BUILD_STARTED", result.stdout)
        self.assertIn("custom Cargo", result.stderr)
        self.assertEqual(config.read_text(), original)

    def test_malformed_manifest_reports_an_error_without_traceback(self):
        for manifest in ([], {"schema": 1, "crates": [3]},
                         {"schema": 1, "crates": [{**self.crate, "patches": [3]}]}):
            with self.subTest(manifest=manifest):
                self.manifest.write_text(json.dumps(manifest))
                result = subprocess.run(
                    [sys.executable, str(ROOT / "tools/prepare_ui_sources.py"),
                     "--manifest", str(self.manifest), "--output", str(self.output), "--offline"],
                    capture_output=True, text=True, timeout=10, check=False,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("Traceback", result.stderr)
                self.assertFalse(self.output.exists())

    def test_fresh_checkout_help_does_not_prepare_or_download(self):
        script = self.root / "tools/prepare_ui_sources.py"
        script.parent.mkdir()
        shutil.copy2(ROOT / "tools/prepare_ui_sources.py", script)
        result = subprocess.run([sys.executable, str(script), "--help"], cwd=self.root,
                                capture_output=True, text=True, timeout=10, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.root / "target").exists())

    @unittest.skipUnless(shutil.which("cargo"), "Cargo required for incremental build check")
    def test_cargo_rebuilds_both_profiles_after_patched_input_changes_with_fixed_mtimes(self):
        first = self.prepare()
        upstream = self.root / "upstream"
        shutil.copytree(first / "ui-probe", upstream)
        project = self.root / "application"
        (project / "src").mkdir(parents=True)
        (project / "Cargo.toml").write_text(
            '[package]\nname="probe-app"\nversion="1.0.0"\nedition="2024"\n'
            '[dependencies]\nui-probe={path="../upstream"}\n'
        )
        (project / "src/main.rs").write_text('fn main() { println!("{}", ui_probe::VALUE); }\n')
        home = self.root / "cargo"
        environment = {**os.environ, "CARGO_HOME": str(home), "CARGO_TARGET_DIR": str(project / "target")}

        def build(sources, expected):
            # Reproduce immutable Nix inputs, whose old timestamps must not let
            # Cargo reuse the previous generation's patched code.
            for file in sources.rglob("*"):
                os.utime(file, (1, 1))
            ui_sources.configure_cargo(sources, home)
            for profile in ([], ["--release"]):
                result = subprocess.run(["cargo", "run", "--offline", "--quiet", *profile],
                                        cwd=project, env=environment, capture_output=True,
                                        text=True, timeout=60, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), str(expected))

        build(first, 2)
        self.change_patch(3)
        build(self.prepare(), 3)


if __name__ == "__main__":
    unittest.main()
