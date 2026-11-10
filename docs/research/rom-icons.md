# ROM box icons (M8, 2026-11-10)

`verity-rom::icons` extracts every box icon from the user's ROM into `/_nds/verity/icons.vicn`
(about 280 KB for Gen 4, 370 KB for Gen 5; format in `CachedIcons`). The DS keeps the head
(palettes and index) in memory and reads each icon's 512 tile bytes from the file. Without a ROM
the box view keeps the placeholder icons. `make emu-font` checks both generations.

## Files

| Games | Icons | Shared files | Order after the species |
| --- | --- | --- | --- |
| Platinum | `poketool/icongra/pl_poke_icon.narc` (547) | 7 (palette NCLR, cells, animations) | egg 494, Manaphy egg 495, then forms |
| HeartGold/SoulSilver | `a/0/2/0` (551) | 7 | same as Platinum; 540-543 are battle-only icons |
| Diamond/Pearl | `poketool/icongra/poke_icon.narc` | 7 | not checked (no ROM); forms past the file count fall back |
| Black/White | `a/0/0/7` (1431) | 7 | two files per icon (the second the female's, often empty); eggs, then forms |
| Black 2/White 2 | `a/0/0/7` (1510) | 8 (a second palette for boxes) | same |

Each icon NCGR is 0x30 bytes of header and two 32x32 frames, 4 bpp in 1D tile order; verity keeps
frame 0. Palettes: the first NCLR, three 16-colour palettes.

Gen 4 forms (`pokeplatinum res/pokemon/meson.build`, `pokeheartgold src/pokemon_icon_idx.c`): form 1
of Deoxys 496, Unown 500 (form 0 is the species icon, A), Burmy 527, Wormadam 529, Shellos 531,
Gastrodon 532, Giratina 533, Shaymin 534, Rotom 535.

Gen 5 forms (`fuddlesworth/pokebw2 src/system/poke_icon.c`): species data `a/0/1/6` gives each
species' sprite offset (`u16` at 0x1E), form count (0x20) and palette-only forms (0x21 bit 7). Form
icons are at `forms_start + offset + form - 1`, where `forms_start` is the icon count minus the
largest `offset + count - 1`; the two eggs are just before.

## Palette table

A byte per icon in the ARM9 code (`sPokemonIconPaletteIndex` / `sPokeIconPalettes`), not a file.
HeartGold's and the Gen 5 ARM9 are BLZ-compressed (`lz::blz_decompress_in_place`); Platinum's is
plain. verity finds the table by its start: species 0, then Bulbasaur to Blastoise (palettes
1 1 1 0 0 0 0 2 2); Gen 5 packs male | female << 4. Found once in each of Platinum (ARM9 offset
984960), HeartGold, White and Black 2. Black 2/White 2's table has two more entries than icons from
the eggs on (Black/White none); verity finds that shift from the eggs' palettes (1, and 2 for
Manaphy's), which hold in all four games.

## Speed on the DS

Reading each file once (not per member) matters: thousands of seeks into a large ROM on a FAT card
took 8-16 s for font and icons together; whole-file reads take 2-4 s (melonDS, Black 2 stand-in).
