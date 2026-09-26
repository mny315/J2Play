"""Exercise the shared build menu and Android packaging with offline tool stand-ins."""

import errno
import json
import os
from pathlib import Path
import pty
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import prepare_ui_sources as ui_sources
import build_outputs

FAKE_TOOL = r"""
import json
import os
from pathlib import Path
import subprocess
import sys

name = Path(sys.argv[0]).name
args = sys.argv[1:]
with open(os.environ["ANDROID_TEST_LOG"], "a", encoding="utf-8") as log:
    log.write(json.dumps({"tool": name, "args": args, "cwd": os.getcwd(),
                          "key": os.environ.get("CARGO_APK_RELEASE_KEYSTORE"),
                          "development_key_password": os.environ.get("CARGO_APK_RELEASE_KEYSTORE_PASSWORD") == "android"}) + "\n")
if os.environ.get("ANDROID_TEST_FAIL") == " ".join([name, *args[:1]]):
    sys.exit(9)
if name == "uname":
    print(os.environ.get("ANDROID_TEST_SYSTEM", "Linux") if args == ["-s"] else "x86_64")
elif name == "nix":
    if args[0] == "develop":
        command = args[args.index("--command") + 1:]
        sys.exit(subprocess.call(command, env={**os.environ, "J2PLAY_DEV_SHELL": "1",
                                              "CARGO_HOME": os.environ["ANDROID_TEST_DEV_CARGO"]}))
    link = Path(args[args.index("--out-link") + 1])
    if link.is_symlink():
        link.unlink()
    link.symlink_to(os.environ["ANDROID_TEST_TOOLCHAIN"], target_is_directory=True)
elif name == "rustc" and "-o" in args:
    binary = Path(args[args.index("-o") + 1])
    binary.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    binary.chmod(0o755)
elif name == "jar":
    Path(args[args.index("--file") + 1]).write_bytes(b"test callback archive")
elif name == "keytool":
    key = Path(args[args.index("-keystore") + 1])
    assert not key.exists(), "existing signing key must be preserved"
    key.write_bytes(b"test signing key")
elif name == "apksigner" and args[0] == "sign":
    assert Path(args[args.index("--ks") + 1]).is_file()
    password = args[args.index("--ks-pass") + 1]
    assert password.startswith("file:"), "release password must not appear in command arguments"
    assert Path(password[5:]).read_text().strip()
elif name == "cargo-apk2" and "build" in args:
    assert (Path.cwd() / "android-build/assets/legal/NOTICE").is_file(), "legal assets must be prepared before packaging"
    output = Path(args[args.index("--target-dir") + 1]) / "release/apk"
    output.mkdir(parents=True, exist_ok=True)
    (output / "AndroidManifest.xml").unlink(missing_ok=True)
    if os.environ.get("ANDROID_TEST_MANIFEST") != "missing":
        manifest = "<manifest/>"
        if os.environ.get("ANDROID_TEST_MANIFEST") == "invalid":
            manifest = "<manifest><android:bad/></manifest>"
        (output / "AndroidManifest.xml").write_text(manifest, encoding="utf-8")
    (output / "j2play.apk").write_bytes(b"test APK")
elif name == "package_legal.py":
    notice = Path.cwd() / "android-build/assets/legal/NOTICE"
    notice.parent.mkdir(parents=True, exist_ok=True)
    notice.write_text("test notice")
elif name == "cargo" and args[0] == "build":
    binary = Path(os.environ["ANDROID_TEST_LINUX_BINARY"])
    binary.parent.mkdir(parents=True, exist_ok=True)
    header = bytearray(20)
    header[:6] = b"\x7fELF\x02\x01"
    header[18:20] = int(os.environ.get("ANDROID_TEST_ELF_MACHINE", "62")).to_bytes(2, "little")
    binary.write_bytes(header + b"project-owned build fixture")
    print(json.dumps({"reason": "compiler-artifact", "target": {"name": "j2play", "kind": ["bin"]},
                      "executable": str(binary)}))
"""


class BuildScriptTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="j2play-build-script-")
        self.addCleanup(self.directory.cleanup)
        self.base = Path(self.directory.name)
        self.root = self.base / "checkout with spaces"
        self.root.mkdir()
        self.script = self.root / "dev.sh"
        shutil.copy2(ROOT / "dev.sh", self.script)
        (self.root / "tools").mkdir()
        shutil.copy2(ROOT / "tools/prepare_ui_sources.py", self.root / "tools/prepare_ui_sources.py")
        shutil.copy2(ROOT / "tools/build_outputs.py", self.root / "tools/build_outputs.py")
        shutil.copy2(ROOT / "tools/build_cache.py", self.root / "tools/build_cache.py")
        shutil.copy2(ROOT / "tools/android_signing.py", self.root / "tools/android_signing.py")
        self.signing = self.root / ".android-signing"
        self.signing.mkdir()
        (self.signing / "release.keystore").write_bytes(b"existing release key")
        (self.signing / "release.keystore.password").write_text("project-owned signing fixture\n")
        for name in ("check.py", "check_catalog.py", "check_report.py"):
            shutil.copy2(ROOT / "tools" / name, self.root / "tools" / name)
        (self.root / "tools/package_legal.py").write_text(FAKE_TOOL, encoding="utf-8")
        shutil.copytree(ROOT / "crates/frontend-ui/build-support", self.root / "crates/frontend-ui/build-support")
        self.tools = self.base / "bin"
        self.tools.mkdir()
        self.toolchain = self.base / "toolchain"
        self.toolchain.mkdir()
        for name in ("sdk", "opencore-amr", "rust", "packager", "ui-sources"):
            (self.toolchain / name).mkdir()
        sources = self.toolchain / "ui-sources"
        crates, fingerprint = ui_sources.read_manifest(ui_sources.MANIFEST)
        for crate in crates:
            (sources / crate["name"]).mkdir()
            (sources / crate["name"] / "Cargo.toml").write_text("[package]\n")
        (sources / ui_sources.STAMP).write_text(json.dumps({
            "fingerprint": fingerprint, "files": ui_sources.inventory(sources),
        }))
        dev_cargo = self.base / "dev-cargo"
        ui_sources.configure_cargo(sources, dev_cargo, patch_names=[
            crate["name"] for crate in crates if crate.get("cargo_override") == "patch"
        ])
        self.log = self.base / "calls.jsonl"
        for name in ("nix", "cargo", "cargo-apk2", "rustc", "jar", "keytool", "adb", "uname"):
            tool = self.tools / name
            tool.write_text(f"#!{sys.executable}\n{FAKE_TOOL}", encoding="utf-8")
            tool.chmod(0o755)
        signer = self.toolchain / "sdk/libexec/android-sdk/build-tools/37.0.0/apksigner"
        signer.parent.mkdir(parents=True)
        signer.write_text(f"#!{sys.executable}\n{FAKE_TOOL}", encoding="utf-8")
        signer.chmod(0o755)
        self.environment = {
            **os.environ,
            "PATH": f"{self.tools}:{os.environ['PATH']}",
            "ANDROID_TEST_LOG": str(self.log),
            "ANDROID_TEST_TOOLCHAIN": str(self.toolchain),
            "ANDROID_TEST_DEV_CARGO": str(dev_cargo),
            "ANDROID_TEST_LINUX_BINARY": str(self.base / "custom target/release/j2play"),
            "CARGO_HOME": str(dev_cargo),
            "J2PLAY_DEV_SHELL": "",
        }

    def calls(self, tool=None):
        calls = [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []
        return [call for call in calls if tool is None or call["tool"] == tool]

    def invoke(self, *args, **environment):
        return subprocess.run(
            [str(self.script), *args], cwd=self.base,
            env={**self.environment, **environment}, stdin=subprocess.DEVNULL,
            capture_output=True, text=True, timeout=10, check=False,
        )

    def menu(self, answer):
        master, slave = pty.openpty()
        try:
            process = subprocess.Popen(
                [str(self.script)], cwd=self.base, env=self.environment,
                stdin=slave, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            )
            try:
                os.write(master, answer)
                stdout, stderr = process.communicate(timeout=10)
                return process.returncode, stdout, stderr
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate()
        finally:
            os.close(master)
            os.close(slave)

    def test_help_and_invalid_commands_have_no_setup_side_effects(self):
        for args, code in (
            (["--help"], 0), (["unknown"], 2), (["build", "extra"], 2),
            (["linux", "extra"], 2), (["menu"], 2),
        ):
            with self.subTest(args=args):
                self.assertEqual(self.invoke(*args).returncode, code)
                self.assertEqual(self.calls(), [])
                self.assertFalse((self.root / "android-build").exists())
                self.assertFalse((self.root / "builds").exists())

    def test_terminal_menu_can_retry_and_exit_without_downloading(self):
        code, stdout, stderr = self.menu(b"9\n0\n")
        self.assertEqual(code, 0, stderr)
        self.assertIn("J2Play development", stdout)
        self.assertIn("4) Build and launch Linux", stdout)
        self.assertIn("7) Run", stdout)
        self.assertIn("Choose a number", stderr)
        self.assertEqual(self.calls(), [])
        self.assertFalse((self.root / "builds").exists())

    def test_common_menu_opens_test_categories_without_preparing_tools(self):
        code, stdout, stderr = self.menu(b"8\n0\n")
        self.assertEqual(code, 0, stderr)
        self.assertIn("Tests and checks", stdout)
        self.assertIn("J2Play checks", stdout)
        self.assertIn("graphics", stdout)
        self.assertEqual(self.calls(), [])
        self.assertFalse((self.root / "test-results").exists())

    def test_run_menu_launches_the_ready_binary_without_rebuilding(self):
        binary = self.root / "builds/linux/x86_64/j2play"
        binary.parent.mkdir(parents=True)
        binary.write_text('#!/bin/sh\nprintf "READY_EMULATOR_STARTED\\n"\n')
        binary.chmod(0o755)
        before = binary.read_bytes()
        code, stdout, stderr = self.menu(b"7\n")
        self.assertEqual(code, 0, stderr)
        self.assertIn("READY_EMULATOR_STARTED", stdout)
        self.assertEqual(binary.read_bytes(), before)
        self.assertEqual([call["args"][0] for call in self.calls("nix")], ["develop"])
        self.assertEqual(self.calls("cargo"), [])
        self.assertEqual(self.calls("cargo-apk2"), [])
        self.assertEqual(self.calls("adb"), [])

    def test_run_without_a_ready_binary_explains_how_to_build_before_setup(self):
        result = self.invoke("run-current")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("./dev.sh linux", result.stderr)
        self.assertEqual(self.calls("nix"), [])
        self.assertEqual(self.calls("cargo"), [])
        self.assertFalse((self.root / "builds").exists())

    def test_terminal_eof_exits_without_downloading(self):
        self.assertEqual(self.menu(b"\x04")[0], 0)
        self.assertEqual(self.calls(), [])

    def test_android_prepare_enters_the_shell_once_and_only_fetches(self):
        result = self.invoke("android-prepare")
        self.assertEqual(result.returncode, 0, result.stderr)
        nix_calls = self.calls("nix")
        self.assertEqual([call["args"][0] for call in nix_calls], ["develop", "build"])
        self.assertEqual([call["args"][0] for call in self.calls("cargo")], ["fetch", "metadata"])
        self.assertTrue(all("--locked" in call["args"] for call in self.calls("cargo")))
        self.assertTrue(all(call["cwd"] == str(self.root) for call in self.calls("cargo")))
        self.assertFalse(any("build" in call["args"] for call in self.calls("cargo-apk2")))
        self.assertEqual(self.calls("keytool"), [])
        self.assertEqual(self.calls("adb"), [])

    def test_already_open_shell_is_reused(self):
        result = self.invoke("android-prepare", J2PLAY_DEV_SHELL="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call["args"][0] for call in self.calls("nix")], ["build"])

    def test_android_uses_verified_absolute_ui_and_winit_inputs(self):
        result = self.invoke("android-check", J2PLAY_DEV_SHELL="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        config = (self.root / "android-build/cargo/config.toml").read_text()
        for name in ("ui-sources/eframe", "ui-sources/egui", "ui-sources/egui-winit", "ui-sources/winit"):
            self.assertIn(str(self.toolchain / name), config)
        self.assertFalse((self.root / "target/ui-sources").exists())
        self.assertFalse(any(call["args"][0] == "clean" for call in self.calls("cargo")))

    def test_corrupt_ui_input_stops_before_fetch_build_or_device_access(self):
        (self.toolchain / "ui-sources/egui/Cargo.toml").write_text("corrupt input")
        result = self.invoke("android-run", J2PLAY_DEV_SHELL="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls("cargo"), [])
        self.assertEqual(self.calls("cargo-apk2"), [])
        self.assertEqual(self.calls("adb"), [])

    def test_noninteractive_default_builds_locked_release_without_device(self):
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        builds = self.calls("cargo-apk2")
        self.assertEqual(len(builds), 1)
        self.assertIn("--release", builds[0]["args"])
        self.assertEqual(builds[0]["key"], str(self.root / "android-build/debug.keystore"))
        self.assertTrue(builds[0]["development_key_password"])
        self.assertFalse((self.root / "android-build/target/release/apk/j2play.apk").exists())
        self.assertEqual((self.root / "builds/android/arm64-v8a/j2play.apk").read_bytes(), b"test APK")
        self.assertEqual([call["args"][0] for call in self.calls("nix")], ["develop", "build"])
        binary = self.root / "builds/linux/x86_64/j2play"
        self.assertEqual(binary.read_bytes()[20:], b"project-owned build fixture")
        self.assertFalse(Path(self.environment["ANDROID_TEST_LINUX_BINARY"]).exists())
        self.assertTrue(os.access(binary, os.X_OK))
        self.assertIn(str(self.root / "builds"), result.stdout)
        self.assertEqual(self.calls("adb"), [])

    def test_run_installs_with_data_preserved_then_launches(self):
        result = self.invoke("android-run", J2PLAY_DEV_SHELL="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        adb = self.calls("adb")
        self.assertEqual(adb[0]["args"][:2], ["install", "-r"])
        self.assertEqual(adb[0]["args"][2], str(self.root / "builds/android/arm64-v8a/j2play.apk"))
        self.assertEqual(adb[1]["args"][:3], ["shell", "am", "start"])
        self.assertEqual([call["args"][0] for call in self.calls("apksigner")], ["sign", "verify"])
        self.assertEqual(self.calls("apksigner")[0]["args"][2], str(self.signing / "release.keystore"))
        tools = [call["tool"] for call in self.calls()]
        self.assertLess(tools.index("cargo-apk2"), tools.index("apksigner"))
        self.assertLess(max(i for i, tool in enumerate(tools) if tool == "apksigner"), tools.index("adb"))
        self.assertIn("Android APK signing key:", result.stdout)
        self.assertNotIn("project-owned signing fixture", self.log.read_text() + result.stdout + result.stderr)

    def test_key_creation_is_explicit_private_and_never_replaces_existing_files(self):
        shutil.rmtree(self.signing)
        result = self.invoke("android-key", J2PLAY_DEV_SHELL="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        key = self.signing / "release.keystore"
        password = self.signing / "release.keystore.password"
        original = (key.read_bytes(), password.read_bytes())
        self.assertEqual(self.signing.stat().st_mode & 0o777, 0o700)
        self.assertEqual(key.stat().st_mode & 0o777, 0o600)
        self.assertEqual(password.stat().st_mode & 0o777, 0o600)
        self.assertGreaterEqual(len(original[1].strip()), 48)
        self.assertNotIn(original[1].decode().strip(), self.log.read_text() + result.stdout + result.stderr)
        self.assertIn("Back up both files", result.stdout)
        self.assertEqual(self.calls("cargo-apk2"), [])
        retry = self.invoke("android-key", J2PLAY_DEV_SHELL="1")
        self.assertNotEqual(retry.returncode, 0)
        self.assertIn("Nothing was replaced", retry.stderr)
        self.assertEqual((key.read_bytes(), password.read_bytes()), original)
        self.assertEqual(len(self.calls("keytool")), 1)

    def test_partial_signing_identity_stops_without_replacing_the_key(self):
        key = self.signing / "release.keystore"
        (self.signing / "release.keystore.password").unlink()
        result = self.invoke("android", J2PLAY_DEV_SHELL="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Restore", result.stderr)
        self.assertEqual(key.read_bytes(), b"existing release key")
        self.assertEqual(self.calls("keytool"), [])
        self.assertEqual(self.calls("nix"), [])

    def test_fresh_checkout_builds_with_its_own_persistent_key_and_notice(self):
        shutil.rmtree(self.signing)
        result = self.invoke("android", J2PLAY_DEV_SHELL="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Creating your own key", result.stdout)
        self.assertIn("cannot update official APKs", result.stdout)
        self.assertTrue((self.root / "builds/android/arm64-v8a/j2play.apk").is_file())
        key = self.signing / "release.keystore"
        password = self.signing / "release.keystore.password"
        original = (key.read_bytes(), password.read_bytes())
        self.assertEqual(self.invoke("android", J2PLAY_DEV_SHELL="1").returncode, 0)
        self.assertEqual((key.read_bytes(), password.read_bytes()), original)
        # One permanent key and one intermediate development key, created once.
        self.assertEqual(len(self.calls("keytool")), 2)

    def test_failed_release_signing_or_verification_preserves_previous_apk(self):
        previous = self.root / "builds/android/arm64-v8a/j2play.apk"
        previous.parent.mkdir(parents=True)
        previous.write_bytes(b"previous signed APK")
        for failure in ("apksigner sign", "apksigner verify"):
            with self.subTest(failure=failure):
                result = self.invoke("android-install", J2PLAY_DEV_SHELL="1", ANDROID_TEST_FAIL=failure)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(previous.read_bytes(), b"previous signed APK")
                self.assertEqual(self.calls("adb"), [])

    def test_failed_build_never_installs(self):
        previous = self.root / "builds/android/arm64-v8a/j2play.apk"
        previous.parent.mkdir(parents=True)
        previous.write_bytes(b"previous successful APK")
        result = self.invoke("android-install", J2PLAY_DEV_SHELL="1", ANDROID_TEST_FAIL="cargo-apk2 apk2")
        self.assertEqual(result.returncode, 9, result.stderr)
        self.assertEqual(self.calls("adb"), [])
        self.assertEqual(previous.read_bytes(), b"previous successful APK")

    def test_missing_legal_inputs_never_publish_or_install(self):
        previous = self.root / "builds/android/arm64-v8a/j2play.apk"
        previous.parent.mkdir(parents=True)
        previous.write_bytes(b"previous successful APK")
        result = self.invoke("android-install", J2PLAY_DEV_SHELL="1",
                             ANDROID_TEST_FAIL="package_legal.py android")
        self.assertEqual(result.returncode, 9, result.stderr)
        self.assertEqual(self.calls("cargo-apk2"), [])
        self.assertEqual(self.calls("adb"), [])
        self.assertEqual(previous.read_bytes(), b"previous successful APK")

    def test_invalid_or_missing_manifest_never_publishes_or_installs(self):
        previous = self.root / "builds/android/arm64-v8a/j2play.apk"
        previous.parent.mkdir(parents=True)
        previous.write_bytes(b"previous successful APK")
        for manifest in ("invalid", "missing"):
            with self.subTest(manifest=manifest):
                result = self.invoke("android-install", J2PLAY_DEV_SHELL="1",
                                     ANDROID_TEST_MANIFEST=manifest)
                self.assertNotEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.calls("adb"), [])
                self.assertEqual(previous.read_bytes(), b"previous successful APK")

    def test_failed_fetch_never_builds_or_generates_a_signing_key(self):
        result = self.invoke("android", J2PLAY_DEV_SHELL="1", ANDROID_TEST_FAIL="cargo fetch")
        self.assertEqual(result.returncode, 9, result.stderr)
        self.assertEqual(self.calls("cargo-apk2"), [])
        self.assertEqual(self.calls("keytool"), [])

    def test_existing_directory_is_renamed_with_key_and_cached_data(self):
        legacy = self.root / ".android-build"
        (legacy / "cargo/registry").mkdir(parents=True)
        (legacy / "cargo/registry/cached.crate").write_bytes(b"cached dependency")
        (legacy / "debug.keystore").write_bytes(b"existing key")
        (legacy / "measurements.log").write_bytes(b"existing diagnostic data")
        result = self.invoke("android", J2PLAY_DEV_SHELL="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        build = self.root / "android-build"
        self.assertFalse(legacy.exists())
        self.assertEqual((build / "debug.keystore").read_bytes(), b"existing key")
        self.assertEqual((build / "cargo/registry/cached.crate").read_bytes(), b"cached dependency")
        self.assertEqual((build / "measurements.log").read_bytes(), b"existing diagnostic data")
        self.assertEqual(self.calls("keytool"), [])

    def test_coexisting_directories_recover_the_key_without_merging_data(self):
        legacy = self.root / ".android-build"
        build = self.root / "android-build"
        legacy.mkdir()
        build.mkdir()
        (legacy / "debug.keystore").write_bytes(b"old key")
        (legacy / "data").write_bytes(b"old data")
        (build / "data").write_bytes(b"new data")
        result = self.invoke("android-prepare", J2PLAY_DEV_SHELL="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((build / "debug.keystore").read_bytes(), b"old key")
        self.assertEqual((legacy / "debug.keystore").read_bytes(), b"old key")
        self.assertEqual((legacy / "data").read_bytes(), b"old data")
        self.assertEqual((build / "data").read_bytes(), b"new data")

    def test_existing_new_key_is_never_overwritten_by_legacy_key(self):
        for name, key in ((".android-build", b"old key"), ("android-build", b"new key")):
            directory = self.root / name
            directory.mkdir()
            (directory / "debug.keystore").write_bytes(key)
        result = self.invoke("android-prepare", J2PLAY_DEV_SHELL="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "android-build/debug.keystore").read_bytes(), b"new key")

    def test_common_builder_linux_selection_uses_reported_target_and_skips_android(self):
        result = self.invoke("linux", J2PLAY_DEV_SHELL="1", ANDROID_TEST_ELF_MACHINE="183")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.root / "builds/linux/aarch64/j2play").is_file())
        self.assertFalse((self.root / "builds/linux/x86_64").exists())
        self.assertFalse((self.root / "android-build").exists())
        self.assertEqual(self.calls("nix"), [])
        self.assertEqual(self.calls("cargo-apk2"), [])

    def test_common_builder_android_selection_skips_linux(self):
        result = self.invoke("android", J2PLAY_DEV_SHELL="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.root / "builds/linux").exists())
        self.assertTrue((self.root / "builds/android/arm64-v8a/j2play.apk").is_file())
        self.assertFalse(any(call["args"][0] == "build" for call in self.calls("cargo")))

    def test_common_builder_stops_on_failure_and_preserves_previous_outputs(self):
        previous = self.root / "builds/linux/x86_64/j2play"
        previous.parent.mkdir(parents=True)
        previous.write_bytes(b"previous successful executable")
        result = self.invoke("all", J2PLAY_DEV_SHELL="1", ANDROID_TEST_FAIL="cargo build")
        self.assertEqual(result.returncode, 9, result.stderr)
        self.assertEqual(previous.read_bytes(), b"previous successful executable")
        self.assertEqual(self.calls("cargo-apk2"), [])
        self.assertFalse((self.root / "android-build").exists())

    def test_repeated_builds_move_outputs_and_failed_rebuild_keeps_them(self):
        for _ in range(2):
            result = self.invoke("all", J2PLAY_DEV_SHELL="1")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(Path(self.environment["ANDROID_TEST_LINUX_BINARY"]).exists())
            self.assertFalse((self.root / "android-build/target/release/apk/j2play.apk").exists())
        for platform, relative, failure in (
            ("linux", "linux/x86_64/j2play", "cargo build"),
            ("android", "android/arm64-v8a/j2play.apk", "cargo-apk2 apk2"),
        ):
            with self.subTest(platform=platform):
                ready = self.root / "builds" / relative
                previous = ready.read_bytes()
                result = self.invoke(platform, J2PLAY_DEV_SHELL="1", ANDROID_TEST_FAIL=failure)
                self.assertEqual(result.returncode, 9, result.stderr)
                self.assertEqual(ready.read_bytes(), previous)


class BuildOutputTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="j2play-build-output-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        root_patch = mock.patch.object(build_outputs, "ROOT", self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        self.source = self.root / "target/release/apk/j2play.apk"
        self.source.parent.mkdir(parents=True)
        self.source.write_bytes(b"new APK")
        self.destination = self.root / "builds/android/arm64-v8a/j2play.apk"
        self.destination.parent.mkdir(parents=True)
        self.destination.write_bytes(b"previous APK")

    def test_same_filesystem_moves_the_file_without_copying(self):
        inode = self.source.stat().st_ino
        self.source.chmod(0o600)
        output = build_outputs.publish(self.source, "android")
        self.assertEqual(output, self.destination)
        self.assertEqual(output.read_bytes(), b"new APK")
        self.assertEqual(output.stat().st_ino, inode)
        self.assertEqual(output.stat().st_mode & 0o777, 0o644)
        self.assertFalse(self.source.exists())

    def test_fresh_cargo_hard_link_is_removed_without_losing_published_output(self):
        header = bytearray(20)
        header[:6] = b"\x7fELF\x02\x01"
        header[18:20] = (62).to_bytes(2, "little")
        binary = self.root / "target/release/j2play"
        binary.write_bytes(header)
        output = build_outputs.publish(binary, "linux")
        self.assertTrue(os.access(output, os.X_OK))
        binary.hardlink_to(output)
        self.assertEqual(build_outputs.publish(binary, "linux"), output)
        self.assertEqual(output.read_bytes(), header)
        self.assertFalse(binary.exists())

    def test_already_published_path_is_not_removed(self):
        output = build_outputs.publish(self.destination, "android")
        self.assertEqual(output.read_bytes(), b"previous APK")

    def test_invalid_executable_preserves_source_and_previous_output(self):
        output = self.root / "builds/linux/x86_64/j2play"
        output.parent.mkdir(parents=True)
        output.write_bytes(b"previous executable")
        with self.assertRaises(ValueError):
            build_outputs.publish(self.source, "linux")
        self.assertEqual(self.source.read_bytes(), b"new APK")
        self.assertEqual(output.read_bytes(), b"previous executable")

    def test_symbolic_link_is_not_moved_into_ready_builds(self):
        link = self.root / "linked.apk"
        link.symlink_to(self.source)
        with self.assertRaises(ValueError):
            build_outputs.publish(link, "android")
        self.assertTrue(link.is_symlink())
        self.assertEqual(self.source.read_bytes(), b"new APK")
        self.assertEqual(self.destination.read_bytes(), b"previous APK")

    def test_failed_move_preserves_source_and_previous_output(self):
        with mock.patch.object(Path, "replace", side_effect=PermissionError(errno.EACCES, "denied")):
            with self.assertRaises(PermissionError):
                build_outputs.publish(self.source, "android")
        self.assertEqual(self.source.read_bytes(), b"new APK")
        self.assertEqual(self.destination.read_bytes(), b"previous APK")

    def cross_filesystem(self, path, destination):
        if path == self.source:
            raise OSError(errno.EXDEV, "different filesystem")
        return self.replace(path, destination)

    def test_cross_filesystem_move_removes_source_after_publication(self):
        self.replace = Path.replace
        with mock.patch.object(Path, "replace", autospec=True, side_effect=self.cross_filesystem):
            output = build_outputs.publish(self.source, "android")
        self.assertEqual(output.read_bytes(), b"new APK")
        self.assertFalse(self.source.exists())
        self.assertEqual(list(output.parent.iterdir()), [output])

    def test_failed_cross_filesystem_copy_keeps_source_and_previous_output(self):
        self.replace = Path.replace
        def partial_copy(incoming, outgoing, **kwargs):
            outgoing.write(incoming.read(2))
            raise OSError(errno.ENOSPC, "disk full")
        with mock.patch.object(Path, "replace", autospec=True, side_effect=self.cross_filesystem):
            with mock.patch.object(build_outputs.shutil, "copyfileobj", side_effect=partial_copy):
                with self.assertRaises(OSError):
                    build_outputs.publish(self.source, "android")
        self.assertEqual(self.source.read_bytes(), b"new APK")
        self.assertEqual(self.destination.read_bytes(), b"previous APK")
        self.assertEqual(list(self.destination.parent.iterdir()), [self.destination])

    def test_failed_cross_filesystem_publication_keeps_source_and_previous_output(self):
        with mock.patch.object(Path, "replace", side_effect=[
            OSError(errno.EXDEV, "different filesystem"), PermissionError(errno.EACCES, "denied"),
        ]):
            with self.assertRaises(PermissionError):
                build_outputs.publish(self.source, "android")
        self.assertEqual(self.source.read_bytes(), b"new APK")
        self.assertEqual(self.destination.read_bytes(), b"previous APK")
        self.assertEqual(list(self.destination.parent.iterdir()), [self.destination])


if __name__ == "__main__":
    unittest.main()
