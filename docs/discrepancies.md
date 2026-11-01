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

## Move edits and PP

`Pkm::set_moves` writes move IDs only; `Pkm::set_moves_with_pp` matches PKHeX `PKM.SetMoves`
(refills PP via `SetMaximumPPCurrent`) and is what the differential test uses.

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

## LeadFinder radar shiny (P4)

PKHeX 25.12.21 `LeadFinder.TryGetMatchRadarShiny` assigns `result` inside its loop and then
unconditionally sets `result = default; return false;`, so a shiny Method 1 match is never
re-checked as a Poké Radar chain shiny. `method_j::lead_info4_dppt` mirrors this (the branch is
omitted). Report upstream.

## Nickname word filter (P5)

PKHeX (default settings) runs its Nintendo Switch word filter, and older console filters by
lookback, over every nickname including Gen 4 origins, which had no in-game filter. verity does not
apply word filters yet (regex lists are costly on the DS); it does apply the digit-count rule.
