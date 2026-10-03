"""Exercise archive boundaries, ABI rejection and data-preserving installation."""

import io
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import linux_cc
import linux_package
import linux_sdk
import linux_sources


class LinuxInputsTest(unittest.TestCase):
    def test_build_manifests_keep_legal_and_ui_sources_separate(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary)
            linux_package.copy_build_manifests(destination)
            expected = {
                "Cargo.lock": ROOT / "Cargo.lock",
                "sdk.json": ROOT / "crates/j2play-linux/build-support/sdk.json",
                "legal-sources.json": ROOT / "legal/sources.json",
                "ui-sources.json": ROOT / "crates/frontend-ui/build-support/sources.json",
            }
            self.assertEqual({path.name for path in destination.iterdir()}, set(expected))
            for name, source in expected.items():
                self.assertEqual((destination / name).read_bytes(), source.read_bytes(), name)

    def test_checksums_cover_nested_inventories_and_ignore_only_the_root_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "SHA256SUMS").write_text("root inventory is excluded")
            nested = root / "licenses/SHA256SUMS"
            nested.parent.mkdir()
            nested.write_bytes(b"nested inventory is payload")
            (root / "empty").touch()
            self.assertEqual(linux_package.checksums(root),
                             linux_sources.sha256(b"") + "  empty\n"
                             + linux_sources.sha256(b"nested inventory is payload")
                             + "  licenses/SHA256SUMS\n")
            nested.write_bytes(b"changed")
            self.assertIn(linux_sources.sha256(b"changed") + "  licenses/SHA256SUMS\n",
                          linux_package.checksums(root))

    @unittest.skipUnless(shutil.which("cargo") and shutil.which("rustc"), "Cargo and rustc required")
    def test_cargo_accepts_sdk_paths_with_spaces(self):
        arch = platform.machine()
        if arch not in linux_sdk.ARCHITECTURES:
            self.skipTest("Supported Linux build host required")
        sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
        with tempfile.TemporaryDirectory(prefix="j2play sdk path ") as temporary, \
                patch.object(linux_sdk, "BUILD", Path(temporary) / "build with spaces"):
            root = Path(temporary)
            sdk = root / "sdk with spaces"
            sdk.mkdir()
            (sdk / "rust").symlink_to(sysroot, target_is_directory=True)
            project = root / "project"
            project.mkdir()
            (project / "Cargo.toml").write_text(
                '[package]\nname = "sdk-path-check"\nversion = "0.1.0"\nedition = "2024"\n'
                '[lib]\npath = "lib.rs"\n')
            (project / "lib.rs").write_text('pub fn value() -> u8 { 42 }\n')
            _, environment, _ = linux_sdk.environment(sdk, arch)
            completed = subprocess.run(
                [shutil.which("cargo"), "check", "--offline", "--target",
                 linux_sdk.manifest()["targets"][arch]["triple"]],
                cwd=project, env=environment, text=True, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, timeout=60,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout)

    def test_reusing_the_toolchain_preserves_compiler_and_cmake_timestamps(self):
        version = linux_sdk.manifest()["rust_version"]

        def output(command, **_kwargs):
            if command == ["rustc", "--print", "sysroot"]:
                return "/fixture/rust\n"
            if command[0] == "patchelf":
                return "/fixture/loader\n"
            if command[0] == "ldd":
                return "libc.so.6 => /fixture/lib/libc.so.6 (0x1)\n"
            if command[-1] == "--version":
                return f"rustc {version} fixture\n"
            self.fail(f"Unexpected toolchain probe: {command}")

        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(linux_sdk, "BUILD", Path(temporary) / "build"), \
                patch.dict(os.environ, {"J2PLAY_DEV_SHELL": "1"}), \
                patch.object(linux_sdk.subprocess, "check_output", side_effect=output), \
                patch.object(linux_sdk.shutil, "which", return_value="/fixture/cc"):
            sdk = Path(temporary) / "sdk"
            cargo, environment, work = linux_sdk.environment(sdk, "x86_64")
            files = {path: (path.read_bytes(), path.stat().st_mode)
                     for path in work.parent.rglob("*") if path.is_file()}
            for path in files:
                os.utime(path, ns=(1_000_000_000, 1_000_000_000))
            _, other_environment, _ = linux_sdk.environment(sdk, "aarch64")
            self.assertNotEqual(other_environment["CARGO_TARGET_DIR"], environment["CARGO_TARGET_DIR"],
                                "Different linker settings must not overwrite host build artifacts")
            repeated = linux_sdk.environment(sdk, "x86_64")
            self.assertEqual(repeated, (cargo, environment, work))
            for path, original in files.items():
                self.assertEqual((path.read_bytes(), path.stat().st_mode), original)
                self.assertEqual(path.stat().st_mtime_ns, 1_000_000_000, path)

    def test_target_translation_preserves_other_compiler_arguments(self):
        self.assertEqual(linux_cc.target_arguments([
            "-c", "source with spaces.c", "--target=x86_64-unknown-linux-gnu", "-O3",
            "-target", "aarch64-unknown-linux-gnu", "-o", "output.o",
        ]), ["-c", "source with spaces.c", "-O3", "-o", "output.o"])
        with self.assertRaises(ValueError):
            linux_cc.target_arguments(["--target"])

    def test_extraction_rejects_traversal_special_files_and_external_links(self):
        for name, kind, link in (("../escape", tarfile.REGTYPE, ""),
                                 ("fifo", tarfile.FIFOTYPE, ""),
                                 ("inside/link", tarfile.SYMTYPE, "../../outside")):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                data = io.BytesIO()
                with tarfile.open(fileobj=data, mode="w") as archive:
                    member = tarfile.TarInfo(name)
                    member.type, member.linkname = kind, link
                    archive.addfile(member)
                with self.assertRaises(ValueError):
                    linux_sources.extract_tar(data.getvalue(), Path(temporary) / "sdk")
                self.assertFalse((Path(temporary) / "escape").exists())

    def test_sdk_reuse_detects_modified_bytes_mode_and_links(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "compiler"
            source.write_text("original")
            source.chmod(0o755)
            target = root / "replacement"
            target.write_text("original")
            target.chmod(0o755)
            linux_sources.seal(root)
            linux_sources.verify(root)
            source.chmod(0o644)
            with self.assertRaises(ValueError):
                linux_sources.verify(root)
            source.chmod(0o755)
            source.write_text("modified")
            with self.assertRaises(ValueError):
                linux_sources.verify(root)
            source.unlink()
            source.symlink_to(target.name)
            with self.assertRaises(ValueError):
                linux_sources.verify(root)

    def test_archive_hardlink_copies_count_toward_the_tree_limit(self):
        data = io.BytesIO()
        with tarfile.open(fileobj=data, mode="w") as archive:
            source = tarfile.TarInfo("source")
            source.size = 6
            archive.addfile(source, io.BytesIO(b"source"))
            link = tarfile.TarInfo("copy")
            link.type = tarfile.LNKTYPE
            link.linkname = "source"
            archive.addfile(link)
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(linux_sources, "MAX_TREE", 10):
            with self.assertRaisesRegex(ValueError, "[Oo]versized"):
                linux_sources.extract_tar(data.getvalue(), Path(temporary))
            self.assertFalse((Path(temporary) / "copy").exists())

    def test_empty_directories_count_toward_the_inventory_limit(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(linux_sources, "MAX_FILES", 2):
            root = Path(temporary)
            for name in ("one", "two", "three"):
                (root / name).mkdir()
            with self.assertRaisesRegex(ValueError, "inventory limit"):
                linux_sources.inventory(root)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "FIFO support required")
    def test_inventory_stamp_rejects_a_fifo_without_waiting_for_a_writer(self):
        with tempfile.TemporaryDirectory() as temporary:
            os.mkfifo(Path(temporary) / ".inventory.json")
            completed = subprocess.run(
                [sys.executable, "-c",
                 "import sys; from pathlib import Path; sys.path.insert(0, sys.argv[1]); "
                 "import linux_sources; linux_sources.verify(Path(sys.argv[2]))",
                 str(ROOT / "tools"), temporary],
                text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=2,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("regular file", completed.stdout)

    def test_abi_gate_rejects_new_glibc_store_paths_and_unknown_dependencies(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "j2play"
            header = bytearray(20)
            header[:6] = b"\x7fELF\x02\x01"
            header[18:20] = (62).to_bytes(2, "little")
            binary.write_bytes(header)
            program = "[Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]"
            dynamic = "(NEEDED) Shared library: [libc.so.6]"
            with patch.object(linux_package, "readelf", side_effect=[program, dynamic, "Name: GLIBC_2.28"]):
                self.assertEqual(linux_package.elf_contract(binary, "x86_64")["maximum_glibc"], "2.28")
            for changed in ((program, dynamic, "Name: GLIBC_2.34"),
                            (program, "(NEEDED) [libunlisted.so]", ""),
                            ("[Requesting program interpreter: /nix/store/loader]", "", ""),
                            (program, dynamic + "\n(RUNPATH) [/tmp/build]", "")):
                with patch.object(linux_package, "readelf", side_effect=changed), self.assertRaises(ValueError):
                    linux_package.elf_contract(binary, "x86_64")


class LinuxPublishTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="j2play-publish-test-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.output = self.root / "builds/linux/aarch64"
        self.work = self.root / "linux-build/packages/aarch64"
        self.staging = self.work / "staging"
        self.staging.mkdir(parents=True)
        self.names = ("J2Play.AppDir", "j2play-linux-aarch64.tar.gz", "j2play-ports-aarch64.zip")

    def stage(self, value):
        appdir = self.staging / "J2Play.AppDir"
        appdir.mkdir()
        (appdir / "J2Play").write_text(value)
        for name in self.names[1:]:
            (self.staging / name).write_text(value)

    def assert_published(self, value):
        self.assertEqual((self.output / "J2Play.AppDir/J2Play").read_text(), value)
        self.assertFalse((self.output / "J2Play.AppDir").is_symlink())
        for name in self.names[1:]:
            self.assertEqual((self.output / name).read_text(), value)
        self.assertEqual(set(self.work.iterdir()), {self.staging})

    def test_repeated_publication_keeps_only_current_artifacts_and_other_outputs(self):
        self.output.mkdir(parents=True)
        (self.output / "j2play").write_text("native executable")
        for value in ("first", "second", "third"):
            self.stage(value)
            linux_package.publish(self.staging, self.output, "aarch64")
            self.assert_published(value)
            self.assertEqual({path.name for path in self.output.iterdir()}, set(self.names) | {"j2play"})
            self.assertEqual(list(self.staging.iterdir()), [])
        self.assertEqual((self.output / "j2play").read_text(), "native executable")

    def test_failed_publication_restores_the_complete_previous_package(self):
        self.stage("previous")
        linux_package.publish(self.staging, self.output, "aarch64")
        for failed_name in self.names:
            with self.subTest(artifact=failed_name):
                self.stage("replacement")
                rename = Path.rename

                def fail_publication(source, destination):
                    if source == self.staging / failed_name:
                        raise OSError("publication fixture failure")
                    return rename(source, destination)

                with patch.object(Path, "rename", fail_publication), self.assertRaises(OSError):
                    linux_package.publish(self.staging, self.output, "aarch64")
                self.assert_published("previous")
                self.assertEqual({path.name for path in self.output.iterdir()}, set(self.names))
                shutil.rmtree(self.staging)
                self.staging.mkdir()

    def test_publication_replaces_a_legacy_symlink_without_touching_its_target(self):
        self.output.mkdir(parents=True)
        legacy = self.output / "payload-legacy"
        legacy.mkdir()
        (legacy / "AppRun").write_text("legacy")
        (self.output / "J2Play.AppDir").symlink_to(legacy.name)
        self.stage("current")
        linux_package.publish(self.staging, self.output, "aarch64")
        self.assert_published("current")
        self.assertEqual((legacy / "AppRun").read_text(), "legacy")


class LinuxInstallTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="j2play-package-test-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.source = self.root / "AppDir with spaces"
        self.source.mkdir()
        self.data = self.root / "user data"
        self.data.mkdir()
        self.install = self.root / 'program % with $ and "quotes"'
        self.environment = {**os.environ, "XDG_DATA_HOME": str(self.data)}

    def payload(self, value):
        for name in ("J2Play", "install.sh"):
            (self.source / name).write_bytes((linux_sdk.SUPPORT / name).read_bytes())
            (self.source / name).chmod(0o755)
        binary = self.source / "usr/bin/j2play"
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_text('#!/bin/sh\nprintf "%s\\n" "$@"\n')
        binary.chmod(0o755)
        (self.source / "payload").write_text(value)
        (self.source / "build-id").write_text(linux_sources.sha256(value.encode()) + "\n")
        desktop = self.source / "usr/share/applications/io.github.mny315.j2play.desktop"
        desktop.parent.mkdir(parents=True, exist_ok=True)
        desktop.write_bytes((ROOT / "crates/j2play-linux/build-support/io.github.mny315.j2play.desktop").read_bytes())
        mime = self.source / "usr/share/mime/packages/io.github.mny315.j2play.xml"
        mime.parent.mkdir(parents=True, exist_ok=True)
        mime.write_bytes((linux_sdk.SUPPORT / "io.github.mny315.j2play.mime.xml").read_bytes())
        readme = self.source / "usr/share/doc/j2play/README.md"
        readme.parent.mkdir(parents=True, exist_ok=True)
        readme.write_bytes((ROOT / "README.md").read_bytes())
        (self.source / "SHA256SUMS").write_text(linux_package.checksums(self.source))

    def run_install(self):
        return subprocess.run(["sh", str(self.source / "install.sh"), str(self.install)],
                              env=self.environment, capture_output=True, text=True, timeout=10, check=False)

    def test_update_retains_previous_program_and_user_data(self):
        self.payload("first")
        self.assertEqual(self.run_install().returncode, 0)
        previous = (self.install / "current").resolve()
        saves = self.data / "io.github.mny315.j2play/library/saves"
        saves.mkdir(parents=True)
        (saves / "fixture.rms").write_bytes(b"owned fixture data")
        self.payload("second")
        result = self.run_install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((previous / "payload").read_text(), "first")
        self.assertEqual((self.install / "current/payload").read_text(), "second")
        self.assertEqual((saves / "fixture.rms").read_bytes(), b"owned fixture data")
        desktop = (self.data / "applications/io.github.mny315.j2play.desktop").read_text()
        self.assertIn("program %%", desktop)
        self.assertNotIn("Exec=j2play\n", desktop)
        self.assertIn("current/J2Play", desktop)
        self.assertIn('" %f\n', desktop)
        self.assertIn("MimeType=application/x-java-archive;application/java-archive;", desktop)
        self.assertTrue((self.data / "mime/packages/io.github.mny315.j2play.xml").is_file())
        cache = (self.data / "applications/mimeinfo.cache").read_text()
        self.assertIn("application/java-archive=io.github.mny315.j2play.desktop;", cache)
        arguments = ["argument with spaces", 'literal $value `command` "quotes"']
        launched = subprocess.run([str(self.install / "current/J2Play"), *arguments],
                                  cwd=self.root, env=self.environment, capture_output=True,
                                  text=True, timeout=5, check=False)
        self.assertEqual(launched.returncode, 0, launched.stderr)
        self.assertEqual(launched.stdout.splitlines(), arguments)

    def test_corrupt_update_keeps_current_program_and_unknown_destination(self):
        self.payload("first")
        self.assertEqual(self.run_install().returncode, 0)
        current = (self.install / "current").resolve()
        self.payload("second")
        (self.source / "payload").write_text("corrupt")
        self.assertNotEqual(self.run_install().returncode, 0)
        self.assertEqual((self.install / "current").resolve(), current)
        self.install = self.root / "existing user directory"
        self.install.mkdir()
        (self.install / "data").write_text("keep")
        self.assertNotEqual(self.run_install().returncode, 0)
        self.assertEqual((self.install / "data").read_text(), "keep")

    @unittest.skipUnless(shutil.which("gio"), "requires the desktop GIO launcher")
    def test_install_registers_java_archives_with_the_desktop_without_replacing_defaults(self):
        self.payload("mime association fixture")
        config = self.root / "config"
        config.mkdir()
        preferences = config / "mimeapps.list"
        original = "[Default Applications]\napplication/java-archive=other.desktop;\n"
        preferences.write_text(original)
        self.environment["XDG_CONFIG_HOME"] = str(config)
        applications = self.data / "applications"
        applications.mkdir()
        (applications / "other.desktop").write_text(
            "[Desktop Entry]\nType=Application\nName=Other\nExec=/bin/true %f\nMimeType=application/java-archive;\n")
        cache = applications / "mimeinfo.cache"
        cache.write_text("[MIME Cache]\napplication/java-archive=other.desktop;\ntext/plain=editor.desktop;\n")
        # Force the fallback even on desktops that install desktop-file-utils.
        binaries = self.root / "bin"
        binaries.mkdir()
        updater = binaries / "update-desktop-database"
        updater.write_text("#!/bin/sh\nexit 1\n")
        updater.chmod(0o755)
        self.environment["PATH"] = str(binaries) + os.pathsep + os.environ["PATH"]
        self.assertEqual(self.run_install().returncode, 0)
        self.assertEqual(self.run_install().returncode, 0)
        self.assertEqual(preferences.read_text(), original)
        self.assertIn("application/java-archive=other.desktop;io.github.mny315.j2play.desktop;\n", cache.read_text())
        self.assertIn("text/plain=editor.desktop;\n", cache.read_text())
        result = subprocess.run(["gio", "mime", "application/java-archive"], env=self.environment,
                                capture_output=True, text=True, timeout=5, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("io.github.mny315.j2play.desktop", result.stdout)

    @unittest.skipUnless(shutil.which("gio"), "requires the desktop GIO launcher")
    def test_desktop_launcher_preserves_literal_installation_path(self):
        self.payload("desktop launch fixture")
        # The owned launcher records its script path. GIO must expand %% after
        # executable lookup and preserve spaces, quotes and shell characters.
        (self.source / "J2Play").write_text(
            '#!/bin/sh\nset -eu\nprintf "%s\\n" "$0" "$@" > "$XDG_DATA_HOME/launched"\n')
        (self.source / "SHA256SUMS").write_text(linux_package.checksums(self.source))
        self.assertEqual(self.run_install().returncode, 0)
        desktop = self.data / "applications/io.github.mny315.j2play.desktop"
        jar = self.root / 'Игра with % $ `quotes` .JAR'
        jar.write_bytes(b"fixture")
        result = subprocess.run(["gio", "launch", str(desktop), str(jar)], env=self.environment,
                                capture_output=True, text=True, timeout=5, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        marker = self.data / "launched"
        deadline = time.monotonic() + 2
        while not marker.exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertTrue(marker.is_file(), "GIO did not run the owned launcher")
        self.assertEqual(marker.read_text().splitlines(), [str(self.install / "current/J2Play"), str(jar)])

    def test_delivered_archives_match_payload_and_reject_a_changed_launcher(self):
        self.payload("archive fixture")
        build_id = (self.source / "build-id").read_text().strip()
        tar = self.root / "j2play-linux-x86_64.tar.gz"
        ports = self.root / "j2play-ports-x86_64.zip"
        linux_package.archive(self.source, tar)
        linux_package.ports_archive(self.source, self.root, "x86_64", build_id)
        linux_package.validate_archives(self.source, self.root, "x86_64")
        self.assertEqual(tar.stat().st_mode & 0o777, 0o644)
        self.assertEqual(ports.stat().st_mode & 0o777, 0o644)
        with zipfile.ZipFile(ports) as archive:
            entries = [(info, archive.read(info)) for info in archive.infolist()]
        with zipfile.ZipFile(ports, "w") as archive:
            for info, data in entries:
                archive.writestr(info, b"broken launcher" if info.filename == "J2Play.sh" else data)
        with self.assertRaises(ValueError):
            linux_package.validate_archives(self.source, self.root, "x86_64")

    def test_archive_validation_uses_the_packaged_readme(self):
        self.payload("readme fixture")
        build_id = (self.source / "build-id").read_text().strip()
        linux_package.archive(self.source, self.root / "j2play-linux-x86_64.tar.gz")
        linux_package.ports_archive(self.source, self.root, "x86_64", build_id)
        checkout = self.root / "changed checkout"
        checkout.mkdir()
        (checkout / "README.md").write_text("Documentation for a later version.\n")
        with patch.object(linux_package.sdk, "ROOT", checkout):
            linux_package.validate_archives(self.source, self.root, "x86_64")

    def test_tar_validation_never_rewinds_the_compressed_stream(self):
        self.payload("streaming fixture")
        build_id = (self.source / "build-id").read_text().strip()
        linux_package.archive(self.source, self.root / "j2play-linux-x86_64.tar.gz")
        linux_package.ports_archive(self.source, self.root, "x86_64", build_id)
        original_seek = linux_package.gzip.GzipFile.seek

        def forward_seek(stream, offset, whence=os.SEEK_SET):
            position = original_seek(stream, 0, os.SEEK_CUR)
            target = offset if whence == os.SEEK_SET else position + offset
            self.assertGreaterEqual(target, position, "validation rewound gzip input")
            return original_seek(stream, offset, whence)

        with patch.object(linux_package.gzip.GzipFile, "seek", forward_seek):
            linux_package.validate_archives(self.source, self.root, "x86_64")

    def test_tar_validation_rejects_extra_directories_outside_the_payload(self):
        self.payload("directory fixture")
        build_id = (self.source / "build-id").read_text().strip()
        tar = self.root / "j2play-linux-x86_64.tar.gz"
        linux_package.archive(self.source, tar)
        linux_package.ports_archive(self.source, self.root, "x86_64", build_id)
        with tarfile.open(tar) as archive:
            entries = [(info, archive.extractfile(info).read() if info.isfile() else None)
                       for info in archive]
        with tarfile.open(tar, "w:gz") as archive:
            for info, data in entries:
                archive.addfile(info, io.BytesIO(data) if data is not None else None)
            extra = tarfile.TarInfo("../outside")
            extra.type = tarfile.DIRTYPE
            archive.addfile(extra)
        with self.assertRaises(ValueError):
            linux_package.validate_archives(self.source, self.root, "x86_64")


if __name__ == "__main__":
    unittest.main()
