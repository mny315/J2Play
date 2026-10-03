#!/usr/bin/env python3
"""Build J2Play's fixed-size BMP bitmap fallback from GNU Unifont HEX."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
from pathlib import Path


SOURCE_SHA256 = "7b182454966046d35482469b979edce7d262fab5c53c2180e9b1fbb5d0b5e574"
MAGIC = b"E240UF01"
ENTRY_SIZE = 33
BMP_CODEPOINTS = 0x10000


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path, help="unifont_all-17.0.05.hex.gz")
    parser.add_argument("output", type=Path, help="generated unifont-bmp.bin")
    args = parser.parse_args()

    compressed = args.source.read_bytes()
    digest = hashlib.sha256(compressed).hexdigest()
    if digest != SOURCE_SHA256:
        raise SystemExit(f"unexpected GNU Unifont source sha256: {digest}")

    output = bytearray(MAGIC)
    output.extend(BMP_CODEPOINTS.to_bytes(4, "big"))
    output.extend(ENTRY_SIZE.to_bytes(4, "big"))
    output.extend(bytes(BMP_CODEPOINTS * ENTRY_SIZE))
    seen: set[int] = set()

    with gzip.open(io.BytesIO(compressed), "rt", encoding="ascii") as source:
        for line_number, line in enumerate(source, 1):
            codepoint_text, bitmap_text = line.rstrip("\n").split(":", 1)
            codepoint = int(codepoint_text, 16)
            if codepoint >= BMP_CODEPOINTS:
                continue
            if codepoint in seen:
                raise SystemExit(f"duplicate BMP code point on line {line_number}")
            seen.add(codepoint)
            bitmap = bytes.fromhex(bitmap_text)
            if len(bitmap) not in (16, 32):
                raise SystemExit(f"unsupported bitmap size on line {line_number}")

            offset = 16 + codepoint * ENTRY_SIZE
            width = 8 if len(bitmap) == 16 else 16
            output[offset] = width
            if width == 8:
                for row, value in enumerate(bitmap):
                    output[offset + 1 + row * 2] = value
            else:
                output[offset + 1 : offset + ENTRY_SIZE] = bitmap

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(output)
    print(f"wrote {args.output} ({len(output)} bytes, {len(seen)} BMP glyphs)")


if __name__ == "__main__":
    main()
