# Discrepancies with PKHeX

This document records any deliberate or known discrepancies between `verity` and PKHeX.
Rule 7: When the plan and PKHeX disagree, PKHeX wins. Any deviation from PKHeX must be justified and recorded here.

## Writing empty slots (P2)

PKHeX `SaveFile.SetBoxSlot` always writes `pk.EncryptedBoxData`, which refreshes the PKM checksum
before encrypting. An all-zero empty slot decrypts (seed 0) to non-zero garbage, so moving an
empty slot through PKHeX writes non-zero bytes. verity's `Save::set_slot_raw` copies bytes
unchanged. Differential tests therefore only move occupied slots; clearing a slot will be a
separate operation that matches PKHeX's delete behaviour.

## Notes for upstream

- `PlayerData5` (PKHeX 25.12.21) reads a `u32` at `0x28` but its setter writes `0x2C`.

## Unedited export (P2 acceptance)

PLAN P2 asks that open → export with no edits is byte-identical. PKHeX itself is not: for HGSS
it always sets `FlagsBoxContentChanged = 0x3FFFF` (`SAV4HGSS.GetFinalData`) so a single
in-game save rewrites every box. verity does the same. The acceptance test compares verity's
unedited export with PKHeX's export, which equals the input for every save except real HGSS
saves (3 flag bytes + storage CRC).

## Move edits and PP (P2)

PKHeX `PKM.SetMoves` also refills PP from move data (`SetMaximumPPCurrent`). Move data arrives in
P3, so `Pkm::set_moves` currently writes move IDs only and the differential test sets raw move
IDs on the PKHeX side. A PP-aware setter follows with P3.

## Pokédex registration (P2)

`Save::register_dex` ports PKHeX `Zukan4.SetDex` / `Zukan5.SetDex` byte-for-byte, including
behaviour that looks unintended and should be checked against the game in the harness (P11) and
reported upstream:

- `Zukan4.SetDexFormValues` ORs `0xFF << (bits * i)` for empty form slots, spilling into
  neighbouring bits.
- `Zukan5.SetDex` marks the species' *base* form index as seen (`SetFormSeen(formIndex, …)`)
  rather than `formIndex + form`.
- `Zukan5.GetFormDisplayed` checks the non-shiny displayed region twice (the shiny check reads
  `FormDex + 2 * FormLen` again), and rejects `formIndex0 + formCount == FormLen * 8`.
- `Zukan5.GetLanguageIndex` maps Korean (8) to -1, so Korean language flags are never set.
