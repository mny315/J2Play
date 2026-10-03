import contextlib
import gzip
import hashlib
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
import build_unifont_fallback as unifont


class UnifontFallbackTest(unittest.TestCase):
    def test_generation_uses_verified_bytes_after_source_is_replaced(self):
        narrow = bytes(range(16))
        wide = bytes(range(32))
        verified = gzip.compress(f"0041:{narrow.hex()}\n4E00:{wide.hex()}\n".encode("ascii"))
        replacement = gzip.compress(b"0041:" + b"FF" * 16 + b"\n")
        open_gzip = gzip.open
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.hex.gz"
            output = Path(directory) / "fallback.bin"
            source.write_bytes(verified)

            def replace_source_then_open(*args, **kwargs):
                source.write_bytes(replacement)
                return open_gzip(*args, **kwargs)

            with (
                patch.object(sys, "argv", ["build_unifont_fallback.py", str(source), str(output)]),
                patch.object(unifont, "SOURCE_SHA256", hashlib.sha256(verified).hexdigest()),
                patch.object(unifont.gzip, "open", side_effect=replace_source_then_open),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                unifont.main()

            generated = output.read_bytes()
            self.assertEqual(len(generated), 16 + 0x10000 * 33)
            self.assertEqual(generated[:16], b"E240UF01\x00\x01\x00\x00\x00\x00\x00\x21")
            narrow_offset = 16 + 0x41 * 33
            self.assertEqual(
                generated[narrow_offset : narrow_offset + 33],
                b"\x08" + b"".join(bytes((value, 0)) for value in narrow),
            )
            wide_offset = 16 + 0x4E00 * 33
            self.assertEqual(generated[wide_offset : wide_offset + 33], b"\x10" + wide)
            self.assertEqual(generated[16 : 16 + 33], bytes(33))

    def test_hash_mismatch_preserves_existing_output(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.hex.gz"
            output = Path(directory) / "fallback.bin"
            source.write_bytes(gzip.compress(b"0041:" + b"00" * 16 + b"\n"))
            output.write_bytes(b"previous output")
            with patch.object(sys, "argv", ["build_unifont_fallback.py", str(source), str(output)]):
                with self.assertRaisesRegex(SystemExit, "unexpected GNU Unifont source sha256"):
                    unifont.main()
            self.assertEqual(output.read_bytes(), b"previous output")


if __name__ == "__main__":
    unittest.main()
