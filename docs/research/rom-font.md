# ROM fonts (M8, 2026-11-10)

Checked on the owner's ROMs before building `crates/verity-rom` (the question was whether an English
ROM's font covers Japanese and Korean names).

| Game | Path | Format | Coverage |
| --- | --- | --- | --- |
| Diamond/Pearl | `graphic/font.narc` (pokediamond `files/graphic/font`) | Gen 4 font | not checked, no ROM |
| Platinum | `graphic/pl_font.narc` | Gen 4 font | kana + Latin, no Hangul |
| HeartGold/SoulSilver | `a/0/1/6` (pokeheartgold `filesystem.csv`) | Gen 4 font | kana + Latin, no Hangul |
| Black/White, Black 2/White 2 | `a/0/2/3` | NFTR | kana, 1,946 kanji, 2,355 Hangul, Latin |

**Result:** any Gen 5 ROM (even English) covers every name a Gen 4 or 5 save can hold. An English
Gen 4 ROM covers kana and Latin only; Hangul then needs a Korean Gen 4 ROM (not checked, none here)
or any Gen 5 ROM. Gen 4 has no kanji in names, so kana is enough for Japanese Gen 4 saves. The
loader therefore prefers a Gen 5 ROM, then a Korean Gen 4 ROM, then any Gen 4 ROM.

## Gen 4 font (`pokeplatinum src/font_manager.c`, `text.c`)

Member 0 of the NARC (FONT_SYSTEM). Header (16 bytes, LE): header size `u32` (16), width table
offset `u32`, glyph count `u32` (509), max width `u8`, max height `u8`, glyph width in tiles `u8`
(2), glyph height in tiles `u8` (2). Glyph `c` (the game's character code, 1-based) is at
`16 + (c-1) * 64`: four 8x8 tiles in the order TL, TR, BL, BR, 2 bits per pixel. Each tile row is a
little-endian `u16` read high byte first, each byte most significant pixel first
(`Text_DecompressGlyph`). The width table is one advance byte per glyph. Codes above the count
draw as `?`. Pixel values: 1 stroke, 2 shadow, 3 background inside the cell, 0 outside.

## Gen 5 font (NFTR)

Members 0 (12x15 cells), 1 (10x10, the small font: Latin capitals 8 px) and 2 (11x13). Standard
Nitro sections: FINF, CGLP, CWDH, CMAP (types 0 direct, 1 table, 2 scan). Glyph bitmaps are one
big-endian bit stream per glyph, row-major, 2 bits per pixel, but each glyph record starts with
3 bytes of metrics (placement offset, ink width, advance; space is 2/0/2, `!` 1/2/3; the
bitmap's ink starts at its column 0), so the record size is
3 + ceil(w*h/4) (28 for 10x10, 48 for 12x15). The CWDH section does not follow the standard
layout, so the loader takes metrics from these prefixes. Pixel values: 1 stroke, 2 shadow; the
shadow is part of the letter shape (corners of `9` are shadow pixels), so verity draws both. No
Hangul glyph is blank.

## Fit to the DS UI (8 px rows)

Ink rows (stroke and shadow), small Gen 5 font: kana and kanji 0..7, Latin 0..7, Hangul 0..9
(overhangs 2 px). Gen 4: kana (codes 2..=0xA1) 2..12, so verity keeps rows 2..12 (`GEN4_ROWS`), overhanging 3 px;
the descenders of fullwidth lowercase letters (to row 14) are cut. ASCII always uses the console
font.
