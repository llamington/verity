# Synthetic corpus

`pkhex-oracle generate-synthetic` writes one save per supported game into `corpus/synthetic/`,
each holding 66 Pokémon (party + 2 boxes) produced by `EncounterMovesetGenerator` and checked
Legal by `LegalityAnalysis`.

- PKHeX.Core 25.12.21 (source reference: `~/src/PKHeX` at tag `25.12.21`).
- Gen 5 uses `BlankSaveFile.Get`.
- Gen 4 blank saves in PKHeX have no exportable buffer (`SAV4(int gSize, int sSize)` allocates
  only the block buffers), so the oracle builds a 0x80000 buffer itself: 0xFF fill, zeroed
  general/storage blocks in both partitions, footer counters at `^0x14`/`^0x10`, size at `^0xC`,
  magic `0x20060623` at `^0x8` (`SaveUtil.IsValidGeneralFooter2`, `SAV4.GetActiveBlock`).
- These saves are structurally valid for PKHeX but are **not** valid game saves; they exercise
  the save/PKM codec only. In-game validity needs real or harness-created saves.
- PKM PIDs come from PKHeX's internal RNG, so regenerated saves differ run to run.
