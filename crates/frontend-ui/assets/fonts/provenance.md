# Product UI fonts

Noto fonts and the J2Play Chinese subset, distributed under the SIL Open Font License 1.1.
Copyright and license text are in `OFL.txt` and `OFL-CJK.txt`; both texts are
also included in the Android and Linux packaged legal notices.
These fonts render only the host UI and do not change guest phone fonts.

| File | Pinned upstream source | SHA-256 |
| --- | --- | --- |
| NotoSans-Regular.ttf | [source](https://github.com/notofonts/noto-fonts/blob/ffebf8c1ee449e544955a7e813c54f9b73848eac/hinted/ttf/NotoSans/NotoSans-Regular.ttf) | `b85c38ecea8a7cfb39c24e395a4007474fa5a4fc864f6ee33309eb4948d232d5` |
| NotoSansArabic-Regular.ttf | [source](https://github.com/notofonts/noto-fonts/blob/ffebf8c1ee449e544955a7e813c54f9b73848eac/hinted/ttf/NotoSansArabic/NotoSansArabic-Regular.ttf) | `ceea25b464a656dc3b26849bab9356740401af62aedf1bfa8b7f0d9b75925b1b` |
| J2PlayChinese-Regular.otf | subset of [Noto Sans CJK SC](https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf) | `8f0473e150f456e348d5bc27b4c12de9f0f06e17cd0b00855cc6f351ab538492` |
| NotoSansDevanagari-Regular.ttf | [source](https://github.com/notofonts/noto-fonts/blob/ffebf8c1ee449e544955a7e813c54f9b73848eac/hinted/ttf/NotoSansDevanagari/NotoSansDevanagari-Regular.ttf) | `385e78e6359a9d88a0f243d53b1209d7548361ba2194e2b9ec779bcaa7e8949d` |
| NotoSansThai-Regular.ttf | [source](https://github.com/notofonts/noto-fonts/blob/ffebf8c1ee449e544955a7e813c54f9b73848eac/hinted/ttf/NotoSansThai/NotoSansThai-Regular.ttf) | `404ddfb5ed0aaa6b6ec8a85700d682978992062d67da93903967b56cbd9a4acc` |

## Chinese subset

Copyright 2014-2021 Adobe (http://www.adobe.com/). The derived font is renamed
J2Play Chinese and retains the source's OFL license and copyright metadata.
Other fonts in the table are unmodified.

The subset covers GB 2312 (including 6,763 common simplified Han characters),
CJK punctuation, fullwidth forms, and characters from all UI catalogs that the
source supports. The other bundled fonts cover the remaining UI scripts.
This preserves common Chinese game/file names beyond the translated strings;
rare Han extensions and the complete traditional Chinese/Japanese/Korean
repertoires are outside this subset. Default simplified Chinese outlines and
hinting are retained; regional alternates and vertical layout substitutions
are removed from this horizontal UI font.

Regenerate from the linked upstream OTF with Python and **FontTools 4.63.0**
(available in the repository's pinned nixpkgs as `python3Packages.fonttools`):

```sh
python3 tools/build_ui_chinese_font.py /path/to/NotoSansCJKsc-Regular.otf \
  crates/frontend-ui/assets/fonts/J2PlayChinese-Regular.otf
```

The script verifies source SHA-256
`2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`
before parsing and preserves the source timestamp for reproducible output.
Normal builds use the checked-in subset and do not need FontTools or a download.
After changing catalogs, regenerate if the font-coverage test reports a missing
Chinese glyph, update this checksum, and run the `frontend-ui` i18n tests.
The font has a 2 MiB regression budget to prevent accidentally embedding the
16.4 MB full CJK source again.
