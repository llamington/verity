# ROM box icons (2026-11-10)

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

## Item icons

`verity-rom::items` extracts each held item's icon into `/_nds/verity/items4.vitm` or `items5.vitm`
(item IDs differ between the generations), from the best ROM of that generation, as the move
caches are. About 164 KB for Platinum; shared icons (the TM disc, Plates) are stored once. Format
in `CachedItems`. The editor shows the icon beside the Main tab's rows and in a top-screen panel
of the held-item picker; without a ROM of the save's generation the panel says so. `make emu-font`
runs `ui-items.json` (Gen 4 save) and `ui-items5.json` (Gen 5 save) against both stand-in ROMs.

| Games | Archive | Members | Table rows (items) |
| --- | --- | --- | --- |
| Diamond/Pearl | `itemtool/itemdata/item_icon.narc` | not checked (no ROM) | up to 464 |
| Platinum | `itemtool/itemdata/item_icon.narc` | 711 | 468 (0-467) |
| HeartGold/SoulSilver | `a/0/1/8` | 797 | 537 (0-536) |
| Black/White | `a/0/2/5` | 1005 | 627 (0-626; the table ends there) |
| Black 2/White 2 | `a/0/2/5` | 1025 | 639 (0-638) |

Each item has its own icon NCGR (0x30 header, one 32x32 frame, 4 bpp, 1D; the art fills the
top-left 24x24) and its own NCLR (16 colours). The ARM9 table names both, per item ID:

- Gen 4 (pokeplatinum `src/item.c`, `sItemArchiveIDs`): `u16 {data, icon, palette, gen3 item}`.
  Item 0 points at the empty icon past the others (Platinum `{0, 707, 708, 0}`), then
  `{1, 2, 3, 1}`, `{2, 4, 5, 2}`. Platinum ARM9 offset 986308, HeartGold 1048980 (after BLZ).
- Gen 5 (pokebw2 `src/system/pml_item.c`, `ITEM_GRAPHICS_CHARS_AND_PALETTES`): `u16 {icon,
  palette}`. Item 0 is `{members - 4, members - 3}`, then `{2, 3}`, `{4, 5}`, `{6, 7}`.

verity finds the table by that start and reads it until a row points outside the archive, capped
at the game's last item (PKHeX `MaxItemID_*`).

## Ribbons

`verity-rom::ribbons` extracts the summary screen's 80 ribbon images into
`/_nds/verity/ribbons.vrib` (about 28 KB, kept whole in memory) from the best ROM, like the box
icons. The editor's ribbon list shows the highlighted ribbon's image, name and state on the top
screen. Diamond/Pearl aren't supported (no ROM to check).

| Games | Table | Row | Archive | Palette member | Param bases (Sinnoh, Hoenn, contest) |
| --- | --- | --- | --- | --- | --- |
| Platinum | ARM9 (offset 1003948) | 12 | `graphic/pl_pst_gra.narc` (images 72-121) | 136 | 25, 78, 123 |
| HeartGold/SoulSilver | ARM9 (offset 1069096) | 12 | `a/0/3/9` | 136 | 25, 78, 123 |
| Black/White | overlay 131 | 16 | `a/0/7/8` | 6 | 25, 77, 120 |
| Black 2/White 2 | overlay 207 | 16 | `a/0/7/7` (images from 18) | 8 | 25, 77, 120 |

Rows are `{u32 param, u16 image, u16 palette, u16 name, u16 description}` (Gen 5 adds a category
byte), found by row `k` naming text line `k` and description `80 + k`. A parameter minus its set's
base is the bit in that ribbon set, which gives verity's list position. Palettes 0-4 of the
palette NCLR colour the images (the contest ribbons share four images, one per rank, coloured by
category). Member 69 of Platinum's archive looks like a candidate but isn't: its palettes 3-4 are
empty. `nds::Rom::overlay` reads overlays; fixed along the way: BLZ refused a literal at the
point where the output meets the input (White's overlay 131).
