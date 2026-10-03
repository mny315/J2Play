# Embedded Unicode fallback

`unifont-bmp.bin` is a deterministic bitmap subset derived from GNU Unifont
17.0.05. It contains the Basic Multilingual Plane in an indexed 8x16/16x16
format used directly by J2Play's LCDUI renderer; it does not use host fonts.

Source:
`https://unifoundry.com/pub/unifont/unifont-17.0.05/font-builds/unifont_all-17.0.05.hex.gz`

Source SHA-256:
`7b182454966046d35482469b979edce7d262fab5c53c2180e9b1fbb5d0b5e574`

Generated asset SHA-256:
`2d0c2ff1dfdb790b4de0a49ce733b09cade9172e945211c9b2a5a420e6f1e446`

Regenerate from the repository root:

```sh
python3 tools/build_unifont_fallback.py \
  unifont_all-17.0.05.hex.gz \
  crates/vm/assets/fonts/unifont-bmp.bin
```

GNU Unifont is copyright (C) 1998-2026 Roman Czyborra, Paul Hardy,
Qianqian Fang, Andrew Miller, Johnnie Weaver, David Corbett,
Ælla Chiana Moskopp, Rebecca Bettencourt, Minseo Lee, Ho-Seok Ee, et al.,
and is dual-licensed. This derived
font data is distributed under the SIL Open Font License 1.1; see
`OFL-1.1.txt`. No Reserved Font Name is declared by this derivative.

The copyright attribution is retained from the 17.0.05 BDF header in the
[upstream source release](https://unifoundry.com/pub/unifont/unifont-17.0.05/unifont-17.0.05.tar.gz)
(SHA-256 `f287cffb26e22723aa36e6684869b0f3ff3bfb822c4b01008bd847911ec1b631`).
Its `COPYING` explicitly offers OFL-1.1 for the font data separately from
the GPL-licensed utility programs. J2Play uses its own Python conversion
script; those upstream programs are not part of J2Play.

The conversion selects BMP glyphs, stores widths and bitmap rows in a
J2Play-specific binary format, and omits the source's upper-plane data.
Both Android and portable Linux packages carry this provenance and the full
copyright/OFL text in their legal collections.
