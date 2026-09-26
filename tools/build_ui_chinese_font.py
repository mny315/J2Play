#!/usr/bin/env python3
"""Subset the pinned Noto CJK source for J2Play's Chinese UI and common names."""

import argparse
import hashlib
import io
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE_SHA256 = "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b"
FONTTOOLS_VERSION = "4.63.0"


def requested_codepoints(catalogs: Path) -> set[int]:
    # GB 2312 includes 6,763 common simplified Han characters, punctuation,
    # pinyin and other symbols. Keep names beyond the current translated UI.
    points = set(range(0x20, 0x7f))
    for lead in range(0xa1, 0xf8):
        for trail in range(0xa1, 0xff):
            try:
                points.add(ord(bytes((lead, trail)).decode("gb2312")))
            except UnicodeDecodeError:
                continue
    points.update(range(0x3000, 0x3040))  # CJK punctuation
    points.update(range(0xff00, 0xfff0))  # Fullwidth forms
    for path in sorted(catalogs.glob("*.json")):
        for text in json.loads(path.read_text(encoding="utf-8")).values():
            points.update(map(ord, text))
    return points


def build_font(source: bytes, catalogs: Path) -> bytes:
    digest = hashlib.sha256(source).hexdigest()
    if digest != SOURCE_SHA256:
        raise ValueError(f"unexpected Noto CJK source sha256: {digest}")

    # Only regeneration needs FontTools; normal builds use the checked-in asset.
    import fontTools
    from fontTools import subset
    from fontTools.ttLib import TTFont

    if fontTools.__version__ != FONTTOOLS_VERSION:
        raise ValueError(f"fontTools {FONTTOOLS_VERSION} is required for reproducible output")
    font = TTFont(io.BytesIO(source), recalcTimestamp=False)
    points = requested_codepoints(catalogs).intersection(font.getBestCmap())
    options = subset.Options()
    options.name_IDs = ["*"]  # Preserve copyright and license metadata.
    # The source's default outlines are simplified Chinese. The horizontal UI
    # does not need alternate regional CJK glyphs or vertical substitutions.
    options.layout_features = [
        feature for feature in options.layout_features
        if feature not in ("locl", "vert", "vrt2")
    ]
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=points)
    subsetter.subset(font)
    names = {
        1: "J2Play Chinese",
        3: "J2PlayChinese-Regular-1",
        4: "J2Play Chinese Regular",
        6: "J2PlayChinese-Regular",
    }
    for record in font["name"].names:
        if record.nameID in names:
            record.string = names[record.nameID].encode(record.getEncoding())
    cff = font["CFF "]
    cff.cff.fontNames = [names[6]]
    cff.cff.topDictIndex[0].FamilyName = names[1]
    cff.cff.topDictIndex[0].FullName = names[4]
    output = io.BytesIO()
    font.save(output)
    result = output.getvalue()
    restored = TTFont(io.BytesIO(result))
    if not points.issubset(restored.getBestCmap()):
        raise ValueError("subset lost requested characters")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="original NotoSansCJKsc-Regular.otf")
    parser.add_argument("output", type=Path, help="J2PlayChinese-Regular.otf")
    args = parser.parse_args()
    result = build_font(args.source.read_bytes(), ROOT / "crates/frontend-ui/locales")
    args.output.write_bytes(result)
    print(f"wrote {args.output} ({len(result)} bytes, sha256 {hashlib.sha256(result).hexdigest()})")


if __name__ == "__main__":
    main()
