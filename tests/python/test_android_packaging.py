"""Check the NativeActivity contract in Android packaging metadata."""

from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]


class AndroidPackagingTest(unittest.TestCase):
    def test_android_native_activity_handles_all_target_sdk_config_changes(self):
        with (ROOT / "crates/j2play-android/Cargo.toml").open("rb") as source:
            android = tomllib.load(source)["package"]["metadata"]["android"]

        self.assertEqual(android["sdk"]["target_sdk_version"], 36)
        self.assertEqual(android["application"]["activity"][0]["config_changes"], "allKnown")
        self.assertNotIn("java_sources", android)


if __name__ == "__main__":
    unittest.main()
