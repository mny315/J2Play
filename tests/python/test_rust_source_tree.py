import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
from validate_rust_source_tree import (
    validate,
)


class RustSourceTreeTest(unittest.TestCase):
    def test_sdl_is_confined_to_linux_shell_even_through_a_workspace_alias(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text(
                '[workspace.dependencies]\nmedia = { package = "sdl2", version = "=0.38.0" }\n',
                encoding="utf-8",
            )
            source = root / "crates/platform/Cargo.toml"
            source.parent.mkdir(parents=True)
            source.write_text('[dependencies]\nmedia.workspace = true\n', encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "SDL dependency outside Linux shell"):
                validate(root)
            shell = root / "crates/j2play-linux/Cargo.toml"
            shell.parent.mkdir()
            source.rename(shell)
            with shell.open("a", encoding="utf-8") as output:
                output.write('[[bin]]\nname = "j2play"\npath = "src/main.rs"\n')
            self.assertGreater(validate(root), 0)

    def test_linux_gui_owns_the_executable_name(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/j2play-linux/Cargo.toml"
            source.parent.mkdir(parents=True)
            source.write_text('[[bin]]\nname = "j2play-linux"\npath = "src/main.rs"\n', encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "Linux GUI must own"):
                validate(root)

    def test_retired_frontend_sources_cannot_return(self):
        for name, content in (
            ("crates/j2play-cli/Cargo.toml", '[package]\nname = "j2play-cli"\n'),
            ("play-games.sh", "#!/bin/sh\n"),
            ("tools/run-x11-game.sh", "#!/bin/sh\n"),
        ):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source = root / name
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_text(content, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "retired frontend"):
                    validate(root)

    def test_product_cannot_depend_on_test_helpers_through_a_workspace_alias(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text(
                '[workspace.dependencies]\nfixtures = { package = "j2play-conformance", path = "tests/conformance" }\n',
                encoding="utf-8",
            )
            source = root / "crates/shell/Cargo.toml"
            source.parent.mkdir(parents=True)
            source.write_text('[dependencies]\nfixtures.workspace = true\n', encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "product crate depends on conformance"):
                validate(root)

    def test_product_cannot_depend_on_test_helpers_through_a_normalized_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/shell/Cargo.toml"
            source.parent.mkdir(parents=True)
            source.write_text(
                '[dependencies]\nfixtures = { path = "../../tests/./conformance" }\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "product crate depends on conformance"):
                validate(root)

    def test_conformance_cannot_launch_a_product_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "tests/conformance/main.rs"
            source.parent.mkdir(parents=True)
            source.write_text('env!("CARGO_BIN_EXE_j2play");', encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "conformance depends on a frontend"):
                validate(root)

    def test_product_cannot_depend_on_test_helpers_through_an_alias(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/shell/Cargo.toml"
            source.parent.mkdir(parents=True)
            source.write_text(
                '[target.\'cfg(unix)\'.dependencies]\nfixtures = { package = "j2play-conformance", path = "../../tests/conformance" }\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "product crate depends on conformance"):
                validate(root)

    def test_validation_does_not_traverse_ignored_user_data(self):
        import os

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ignored = root / "games"
            ignored.mkdir()
            (root / "main.rs").write_text("fn main() {}", encoding="utf-8")
            scandir = os.scandir

            def guarded_scandir(path):
                self.assertNotEqual(Path(path), ignored)
                return scandir(path)

            with patch("os.scandir", side_effect=guarded_scandir):
                self.assertEqual(validate(root), 1)

    def test_rust_test_bodies_must_live_in_the_central_tree(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#[test]\nfn probe() {}\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "Rust test outside tests/"):
                validate(root)
            destination = root / "tests/unit/example/mod.rs"
            destination.parent.mkdir(parents=True)
            source.rename(destination)
            self.assertEqual(validate(root), 1)

    def test_external_unit_module_hooks_are_allowed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text(
                '#[cfg(test)]\n#[path = "../../../tests/unit/example/mod.rs"]\nmod tests;\n',
                encoding="utf-8",
            )
            self.assertEqual(validate(root), 1)

    def test_android_has_no_java_source_exception(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/j2play-android/java/io/github/mny315/j2play/J2PlayActivity.java"
            source.parent.mkdir(parents=True)
            source.write_text("class J2PlayActivity extends NativeActivity {}", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "source file"):
                validate(root)

    def test_android_callback_exports_do_not_allow_unsafe_operations(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/j2play-android/src/android_adapter/ffi.rs"
            source.parent.mkdir(parents=True)
            source.write_text('#[unsafe(no_mangle)]\nextern "system" fn callback() {}\n', encoding="utf-8")
            self.assertEqual(validate(root), 1)
            source.write_text("fn callback() { unsafe {} }", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "unsafe Android entry operation"):
                validate(root)

    def test_android_entry_allows_only_the_required_export_attribute(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/j2play-android/src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text(
                "#[unsafe(no_mangle)]\nextern \"Rust\" fn android_main() {}\n",
                encoding="utf-8",
            )
            self.assertEqual(validate(root), 1)
            source.write_text("fn bad() { unsafe {} }\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "unsafe Android entry operation"):
                validate(root)

    def test_source_file_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "fixtures" / "Legacy.java"
            source.parent.mkdir()
            source.write_text("class Legacy {}", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "source file"):
                validate(root)

    def test_numeric_aot_unsafe_is_confined_to_executable_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/native-code/src/executable.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn invoke() { unsafe {} }\n", encoding="utf-8")
            self.assertEqual(validate(root), 1)
            source.rename(source.with_name("compiler.rs"))
            with self.assertRaisesRegex(ValueError, "unsafe numeric compiler operation"):
                validate(root)

    def test_kotlin_and_gradle_are_rejected(self):
        for name in ("Bridge.kt", "build.gradle.kts", "gradlew"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / name).write_text("not allowed", encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "source file|Android build system file"):
                    validate(root)

    def test_amr_unsafe_is_confined_to_codec_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/amr-nb/src/ffi.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn decode() { unsafe {} }\n", encoding="utf-8")
            self.assertEqual(validate(root), 1)
            source.rename(source.with_name("lib.rs"))
            with self.assertRaisesRegex(ValueError, "unsafe operation outside AMR codec boundary"):
                validate(root)

    def test_toolchain_package_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "flake.nix").write_text("packages = [ jdk21_headless ];", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "toolchain package"):
                validate(root)

    def test_exact_android_build_host_jdk_is_allowed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "flake.nix").write_text("packages = [ jdk17_headless ];", encoding="utf-8")
            self.assertEqual(validate(root), 1)

    def test_toolchain_command_is_rejected(self):
        for command in (
            "javac Fixture.java",
            "./gradlew assemble",
            "env gradle build",
            "command kotlinc Bridge.kt",
        ):
            with self.subTest(command=command), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                script = root / "dev.sh"
                script.write_text(f"#!/bin/sh\n  {command}\n", encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "toolchain command"):
                    validate(root)

    def test_rust_sources_are_not_rejected_by_line_count(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates" / "runtime" / "src" / "inventory.rs"
            source.parent.mkdir(parents=True)
            source.write_text("// cohesive inventory\n" * 10_000, encoding="utf-8")
            self.assertEqual(validate(root), 1)

    def test_guest_archive_is_an_allowed_emulator_input(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "fixture.jar"
            with zipfile.ZipFile(archive, "w") as output:
                output.writestr("Fixture.class", b"\xca\xfe\xba\xbe")
            self.assertEqual(validate(root), 1)

    def test_local_runtime_and_commercial_game_trees_are_out_of_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in (
                ".android-build",
                "android-build",
                "linux-build",
                "builds",
                "games",
                "games1",
                "j2play-data",
                "logs",
                "target",
                "out",
            ):
                source = root / name / "Private.java"
                source.parent.mkdir()
                source.write_text("class Private {}", encoding="utf-8")
            self.assertEqual(validate(root), 0)


if __name__ == "__main__":
    unittest.main()
