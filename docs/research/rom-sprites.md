# ROM sprites, type and category icons (2026-11-10)

`verity-rom::sprites` copies every Pokémon front sprite (forms, female sprites, eggs, normal and
shiny palettes) and the type and move category icons from the user's ROM into one cache per
generation: `/_nds/verity/sprites4.vspr` (about 1 MB) and `sprites5.vspr` (about 0.8 MB). Built on
first start from the best ROM of each generation, like the move and item caches. Nothing from a
ROM is in the repo. Checked by `cargo test -p verity-rom --test sprites`, which writes contact
sheets to `target/sprites-<rom>.ppm`.

## Sprite layout

Every cached sprite is 96x96, 4 bpp, in four 1D sprite blocks: 64x64, 32x64, 64x32, 32x32 (4608
bytes). That is how the Gen 5 games store a front sprite whole, so the DS draws any sprite as four
16-colour objects with no conversion.

## Gen 4

| Games | Sprites | Forms and eggs | Icons |
| --- | --- | --- | --- |
| Platinum | `poketool/pokegra/pl_pokegra.narc` | `poketool/pokegra/pl_otherpoke.narc` | `battle/graphic/pl_batt_obj.narc` |
| HeartGold/SoulSilver | `a/0/0/4` | `a/1/1/4` | `a/0/0/8` |
| Diamond/Pearl | `poketool/pokegra/pokegra.narc` | not read (no ROM to check) | `battle/graphic/batt_obj.narc` |

- Six files per species: back female, back male, front female, front male, normal palette, shiny
  palette (pokeplatinum `BuildPokemonSpriteTemplate`). Single-gender species leave the other front
  empty; most others store the same sprite twice, so verity keeps a female sprite only when it
  differs.
- Each NCGR is 160x80 scanned (two 80x80 frames side by side), encrypted: every u16 XORed with an
  LCG (`key = key * 0x4E6D + 0x6073`) seeded from the first word, walking forwards. Diamond/Pearl
  reportedly walk backwards from the last word; verity picks the direction that leaves more of the
  image transparent there. Frame 0 goes into the 96x96 layout at (8, 16), so it stands on the
  bottom edge like a Gen 5 sprite.
- Forms (Deoxys, Unown, Castform, Burmy, Wormadam, Shellos, Gastrodon, Cherrim, Arceus, Shaymin,
  Rotom, Giratina, HGSS's spiky-eared Pichu) and the two eggs come from the form archive by the
  formulas in `FORMS4`. Platinum has 154 sprites before its palettes, HeartGold 158, so its palette
  numbers are 4 higher; verity finds the first palette by member size. Form palettes there may be
  LZ-compressed.
- Spinda's spots aren't drawn (the game paints them from the PID).
- Type and category icons: the ARM9 has, in a row, the category members (physical, special,
  status; u32), the type members (u32 by Gen 4 type: 18 types with ??? at 9, then 5 contest
  categories) and the type palette bytes (`0, 0, 1, 1, 0, 0, 2, 1, 0, 2, ...`), which is how
  verity finds them (pokeplatinum `sMoveTypeIconIndex`). The three palettes are the archive's only
  NCLR with a 0x78-byte TTLP (member 74 in both games). Category palettes `0, 1, 0`.

## Gen 5

- `a/0/0/4`: 20 files per group. 0 front, 1 female front (empty when none), 18 normal and 19 shiny
  palette; the rest are back sprites and animation data. Fronts are LZ11-compressed NCGRs already in
  the 96x96 block layout, and the cache keeps them compressed.
- Groups 0-649 are species, then the egg and Manaphy's egg, then forms at `forms_start + offset +
  form - 1`, with the offset from species data (`a/0/1/6` 0x1E), as for icons. The archive ends
  with palettes for Arceus's types (normal and shiny per form, form 1 first). `forms_start` is the
  whole groups before those palettes minus the largest `offset + count - 1`: 652 in White, 685 in
  Black 2.
- Icons (pokebw2 `AppMenuCommon_*`): `a/0/8/3` in Black/White, `a/0/8/2` in Black 2/White 2.
  Palettes 0x21, type t at 0x22 + t, category c (status, physical, special) at 0x38 + c, ball
  icons at 0x6F + ball (not used: verity draws balls from the item icon cache).

## Cache (`VSPR` version 1)

Sprite records first (64 palette bytes, normal then shiny, and the tile data), then the icon block
(three palettes, then a palette number and 256 tile bytes for each of 17 types and 3 categories,
in verity's numbering), then the index (16 bytes per sprite: species, form, flags female 1/egg 2,
palette offset, tile offset, tile length; sorted), then a 24-byte footer (magic, version,
generation, count, index offset, icon block offset, game code). The index is last because the cache
is written while the ROM is read: a Gen 4 cache doesn't fit in the DS's memory. Gen 4 tile data is
compressed with a runs-only LZ10 (`compress_runs`, about four to one).

The DS reads the footer, keeps the icon block, index and footer (about 20 KB) in memory and reads
each sprite by offset. Lookups fall back to the male sprite, then the base form.

## Speed on the DS

The build reads members in archive order, so the card only seeks forwards: a backwards FAT seek
walks the cluster chain from the file's start. Gen 5 reads only each group's fronts and palettes.
With the stand-in ROMs in melonDS, first start (font, icons, items, moves, ribbons and both sprite
caches) takes about 20 s. Without the sprite caches it took 3-4 s.
