import copy
import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
from validate_cldc_fixtures import JAR, MANIFEST, validate


class CldcMethodFixturesTest(unittest.TestCase):
    def test_changed_guest_archive_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "changed.jar"
            archive.write_bytes(JAR.read_bytes() + b"changed")
            with self.assertRaisesRegex(ValueError, "fixture archive SHA-256 mismatch"):
                validate(archive, MANIFEST)

    def test_validation_uses_verified_bytes_after_source_is_replaced(self):
        expected_count = validate()
        open_zip = zipfile.ZipFile
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "fixture.jar"
            source.write_bytes(JAR.read_bytes())

            def replace_source_then_open(*args, **kwargs):
                source.write_bytes(b"replaced after SHA-256 verification")
                return open_zip(*args, **kwargs)

            with patch("validate_cldc_fixtures.zipfile.ZipFile", side_effect=replace_source_then_open):
                self.assertEqual(validate(source, MANIFEST), expected_count)

    def test_missing_method_fixture_is_rejected(self):
        document = json.loads(MANIFEST.read_text(encoding="utf-8"))
        document["fixtures"].pop()
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "fixtures.json"
            manifest.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "method fixture mismatch"):
                validate(JAR, manifest)

    def test_duplicate_method_fixture_is_rejected(self):
        document = json.loads(MANIFEST.read_text(encoding="utf-8"))
        document["fixtures"][1] = copy.deepcopy(document["fixtures"][0])
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "fixtures.json"
            manifest.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "duplicate method fixture"):
                validate(JAR, manifest)

    def test_checkpoints_capture_method_results_not_fixture_ordinals(self):
        document = json.loads(MANIFEST.read_text(encoding="utf-8"))
        fixtures = {item["signature"]: item for item in document["fixtures"]}
        self.assertEqual(fixtures["java/lang/String::length()I"]["expected"], 3)
        self.assertEqual(
            fixtures["java/lang/Integer::parseInt(Ljava/lang/String;)I"]["expected"],
            1,
        )
        self.assertEqual(fixtures["java/util/Vector::size()I"]["expected"], 2)
        self.assertEqual(
            fixtures["java/lang/Float::toString(F)Ljava/lang/String;"]["expected"],
            48563,
        )


if __name__ == "__main__":
    unittest.main()
