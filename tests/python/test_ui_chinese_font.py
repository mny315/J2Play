import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
import build_ui_chinese_font as chinese_font


class ChineseFontTest(unittest.TestCase):
    def test_requested_characters_include_common_names_and_catalogs(self):
        catalogs = chinese_font.ROOT / "crates/frontend-ui/locales"
        points = chinese_font.requested_codepoints(catalogs)
        han = {point for point in points if 0x4e00 <= point <= 0x9fff}
        self.assertGreaterEqual(len(han), 6763)
        self.assertTrue(set(map(ord, "龙猫麒麟龟蛇【】！１２３")).issubset(points))
        self.assertIn(ord("龠"), points)  # Last character in GB 2312.
        self.assertNotIn(0x20000, points)  # No blanket CJK extension ranges.

    def test_unverified_source_is_rejected_before_subsetting(self):
        with self.assertRaisesRegex(ValueError, "unexpected Noto CJK source sha256"):
            chinese_font.build_font(b"wrong source", Path("unused"))


if __name__ == "__main__":
    unittest.main()
