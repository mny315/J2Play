import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
import build_text_layout_data as text_data


def character(codepoint, category, decomposition=""):
    return f"{codepoint:04X};TEST;{category};0;L;{decomposition};;;;N;;;;;\n"


class TextLayoutDataTest(unittest.TestCase):
    def test_source_checksums_match_packaged_provenance(self):
        root = Path(__file__).resolve().parents[2]
        provenance = json.loads((root / "legal/sources.json").read_text())["text_layout"]
        sources = {item["url"].rsplit("/", 1)[1]: item["sha256"] for item in provenance["sources"]}
        self.assertEqual(sources, text_data.SOURCES)
        self.assertTrue((root / provenance["generator"]).is_file())
        self.assertTrue((root / provenance["output"]).is_file())

    def test_joining_overrides_and_contextual_forms_use_ucd_semantics(self):
        data = "".join((
            character(0x0301, "Mn"),
            character(0x064E, "Mn"),
            character(0x200C, "Cf"),
            character(0x200D, "Cf"),
            character(0xFE8F, "Lo", "<isolated> 0628"),
            character(0xFE90, "Lo", "<final> 0628"),
            character(0xFE91, "Lo", "<initial> 0628"),
            character(0xFE92, "Lo", "<medial> 0628"),
        ))
        result = text_data.generate("0028; 0029\n0029; 0028\n",
                                    "0628; BEH; D; BEH\n200C; ZWNJ; U; No_Joining_Group\n"
                                    "200D; ZWJ; C; No_Joining_Group\n", data)
        self.assertIn("(0x0028, 0x0029)", result)
        self.assertIn("(0x0301, 0x0301)", result)
        self.assertIn("(0x064e, 0x064e, 4)", result)
        self.assertIn("(0x200d, 0x200d, 3)", result)
        self.assertNotIn("(0x200c,", result)
        self.assertIn("(0x0628, [0xfe8f, 0xfe90, 0xfe91, 0xfe92])", result)

    def test_hash_failure_preserves_existing_output(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "input.txt"
            output = Path(directory) / "output.rs"
            source.write_bytes(b"incorrect source")
            output.write_bytes(b"previous output")
            with patch.object(sys, "argv", ["build_text_layout_data.py", *([str(source)] * 3), str(output)]):
                with self.assertRaisesRegex(ValueError, "unexpected Unicode 17 source sha256"):
                    text_data.main()
            self.assertEqual(output.read_bytes(), b"previous output")

    def test_verified_bytes_are_parsed_without_reopening_the_source(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "input.txt"
            source.write_bytes(b"0028; 0029\n")
            with patch.dict(text_data.SOURCES, {"BidiMirroring.txt": hashlib.sha256(source.read_bytes()).hexdigest()}):
                verified = text_data.read_verified(source, "BidiMirroring.txt")
            source.write_bytes(b"incorrect source")
            self.assertEqual(verified, "0028; 0029\n")


if __name__ == "__main__":
    unittest.main()
