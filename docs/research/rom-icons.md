# ROM box icons (M8, research started 2026-11-10)

Not built yet. What is known so far, from the owner's ROMs and `pret/pokeplatinum src/pokemon_icon.c`:

- **Platinum:** `poketool/icongra/pl_poke_icon.narc`, 547 members: 0 NCLR (`shared_pals`, 3
  palettes), 1-6 NANR/NCER cells and animations, then 540 icon NCGRs (1072 bytes: 32x64, two 32x32
  frames, 4 bpp). Icon index = species, with forms and eggs after 494 (`PokeIconSpriteIndex`).
- **Palette per icon:** a byte table (`sPokemonIconPaletteIndex`, values 0-2) compiled into the
  code, not a file. Platinum's ARM9 is uncompressed; the table is at ARM9 offset 984960 and is
  exactly the last 540 bytes of a run of bytes <= 2 (species 0 = 0, Bulbasaur line 1 1 1,
  Charmander line 0 0 0, Squirtle 0, Wartortle/Blastoise 2). Candidate rule: the run of bytes <= 2
  that ends exactly `icon count` bytes after a start whose first byte is 0. Must be checked on
  every ROM before use.
- **HeartGold, White, Black 2:** ARM9 is BLZ-compressed (footer), so the table needs BLZ
  decompression first, and may sit in an overlay instead.
- **Gen 5:** `a/0/0/7`, 1510 members: 0-1 NCLR (158 and 192 bytes), 2-7 NANR/NCER, icon NCGRs
  from 8, interleaved with empty members (likely female variants). Palette table not located yet.

Fallback stays the current placeholder icons.
