"""Execute owned Rust fixtures against the exact prepared winit sources."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import prepare_ui_sources


class WinitPatchesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.winit = prepare_ui_sources.check_cargo_home(
            prepare_ui_sources.MANIFEST, Path(os.environ["CARGO_HOME"])
        ) / "winit/src/platform_impl"

    def run_fixture(self, source):
        with tempfile.TemporaryDirectory(prefix="j2play-winit-test-") as directory:
            source_path = Path(directory) / "fixture.rs"
            binary = Path(directory) / "fixture-tests"
            source_path.write_text(source, encoding="utf-8")
            for command in (
                ["rustc", "--edition=2024", "--test", str(source_path), "-o", str(binary)],
                [str(binary)],
            ):
                result = subprocess.run(
                    command, capture_output=True, text=True, timeout=30, check=False
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_android_poll_timeout(self):
        # Extract the prepared source independently of patch context length.
        lines = (self.winit / "android/mod.rs").read_text().splitlines()
        start = lines.index("        timeout =")
        end = next(index for index in range(start, len(lines))
                   if lines[index].startswith("        let app ="))
        fixture = (ROOT / "tests/unit/j2play-android/poll_timeout.rs").read_text()
        self.run_fixture(fixture.replace(
            "// PRODUCTION_TIMEOUT_SELECTION", "\n".join(lines[start:end])
        ))

    def test_wayland_bounded_uri_stream_reader(self):
        reader = self.winit / "linux/wayland/file_drop_read.rs"
        tests = ROOT / "tests/unit/j2play-linux/wayland_file_drop.rs"
        self.run_fixture(
            f"#[path = {json.dumps(str(reader))}] mod reader;\n"
            f"#[path = {json.dumps(str(tests))}] mod tests;\n"
        )


if __name__ == "__main__":
    unittest.main()
