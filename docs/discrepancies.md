# Discrepancies with PKHeX

This document records any deliberate or known discrepancies between `verity` and PKHeX.
Rule 7: When the plan and PKHeX disagree, PKHeX wins. Any deviation from PKHeX must be justified and recorded here.

## Writing empty slots

PKHeX `SaveFile.SetBoxSlot` always writes `pk.EncryptedBoxData`, which refreshes the PKM checksum
before encrypting. An all-zero empty slot decrypts (seed 0) to non-zero garbage, so moving an
empty slot through PKHeX writes non-zero bytes. verity's `Save::set_slot_raw` copies bytes
unchanged. Differential tests therefore only move occupied slots; clearing a slot will be a
separate operation that matches PKHeX's delete behaviour.

## Notes for upstream

- `PlayerData5` (PKHeX 25.12.21) reads a `u32` at `0x28` but its setter writes `0x2C`.

## Unedited export

Verity aims for open → export with no edits to be byte-identical. PKHeX itself is not: for HGSS
it always sets `FlagsBoxContentChanged = 0x3FFFF` (`SAV4HGSS.GetFinalData`) so a single
in-game save rewrites every box. verity does the same. The acceptance test compares verity's
unedited export with PKHeX's export, which equals the input for every save except real HGSS
saves (3 flag bytes + storage CRC).

## Move edits and PP

`Pkm::set_moves` writes move IDs only; `Pkm::set_moves_with_pp` matches PKHeX `PKM.SetMoves`
(refills PP via `SetMaximumPPCurrent`) and is what the differential test uses.

## Pokédex registration

`Save::register_dex` ports PKHeX `Zukan4.SetDex` / `Zukan5.SetDex` byte-for-byte, including
behaviour that looks unintended and should be checked against the game in the harness (`make emu-ingame`) and
reported upstream:

- `Zukan4.SetDexFormValues` ORs `0xFF << (bits * i)` for empty form slots, spilling into
  neighbouring bits.
- `Zukan5.SetDex` marks the species' *base* form index as seen (`SetFormSeen(formIndex, …)`)
  rather than `formIndex + form`.
- `Zukan5.GetFormDisplayed` checks the non-shiny displayed region twice (the shiny check reads
  `FormDex + 2 * FormLen` again), and rejects `formIndex0 + formCount == FormLen * 8`.
- `Zukan5.GetLanguageIndex` maps Korean (8) to -1, so Korean language flags are never set.

## LeadFinder radar shiny

PKHeX 25.12.21 `LeadFinder.TryGetMatchRadarShiny` assigns `result` inside its loop and then
unconditionally sets `result = default; return false;`, so a shiny Method 1 match is never
re-checked as a Poké Radar chain shiny. `method_j::lead_info4_dppt` mirrors this (the branch is
omitted). Report upstream.

## Nickname and OT word filter

PKHeX (default settings) runs its Nintendo Switch word filter, and older console filters by
lookback, over every nickname and player OT name including Gen 4 origins, which had no in-game
filter. verity does not apply word filters yet (regex lists are costly on the DS); it does apply
the digit-count rule (.NET `char.IsNumber` over the Gen 4 charset). The fuzz parity tests skip
samples PKHeX flags only through a word-filter pattern.

## Gen 4 trash bytes

PKHeX has a Wonder Card trash-byte check (`TrashByteVerifier.VerifyTrashBytesPCD`: OT and
nickname buffers must equal the card's), but `LegalityAnalysis` only runs the trash verifier for
format 7+, so a PK4 is never checked. verity follows PKHeX and does not check them either.

## Game-safe validator bounds

`safety/` rejects values a legitimately obtained Pokémon cannot carry, not only values known to
crash: met/egg locations outside PKHeX's Gen 4/5 location tables (Gen 4 D/P/Pt field 0x80/0x7E
≤ 111 or 2000–2014 / 3000–3076; the Pt/HG/SS fields ≤ 234 or those ranges; Gen 5 ≤ 153,
30001–30015, 40001–40109, 60001–60003, plus Gen 4 egg locations on Gen 4 origins), DP/Pt ball
byte 1–16 and HG/SS ball byte ≤ 24, PP above the move's max. What each game actually tolerates past
these bounds needs the emulator harness; widen only with evidence.

Party stats: the games recompute stats only on level-up or a box round trip, so a real save can
hold stale party stats (seen in `pkhex-everywhere/savedata_4hgss.dsv`). `check_pkm` accepts stale
stats (it requires the level byte to match EXP, non-zero stats and HP ≤ max HP); verity's own party
writes must have exactly computed stats (`party_stats_exact`).

## Method K re-roll seed

PKHeX 25.12.21 `MethodK.TryGetMatch`, Synchronize branch: `IsSlotValidRegular(ctx, out seed)`
reuses `seed` as its out parameter, so the following `Recurse4x` call (HG/SS Safari Zone and
Bug-Catching Contest re-rolls) starts from the slot seed, or 0 when no slot matched, instead of
the frame's seed. `rng::method_k` mirrors this. Report upstream.

## Mystery Gift card link on Platinum

PKHeX `MysteryBlock4.UpdateSlotPGT` says a pending gift's link to its Wonder Card counts from 1 on
D/P/Pt (no card = 4) and from 0 on HG/SS (no card = 3). The Platinum decompilation writes the
0-based slot into a 2-bit field (`MysteryGift_TrySavePgt`), so `4` could not be stored. verity
follows the decompilation on Platinum and PKHeX on D/P. Retail Platinum collected 11 of 11 gifts
this way. See `docs/research/events-region.md`.

## Linked harness runs are threaded

The harness was meant to step two instances in lockstep. melonDS's `LocalMP` waits on semaphores with a
25 ms timeout, so each console needs its own thread, and the frame lead between them is only softly
limited. Linked runs aren't deterministic; single-console runs still are. See
`docs/research/emu-harness.md`.

## Memory Link refusals

The original plan had `memlink::build` refuse a BW save that hasn't reached minimum progress or whose
region or language doesn't match. The game links a save with the Hall of Fame flag (2400) cleared,
leaving out the Hall of Fame part, so `build` does too. The decompiled host code has no language
check and verity adds none; other-language links are untested. verity is stricter than the game in
one way: it refuses corrupt outside blocks, which the game clears and starts again.

## Keys without a partner save

The original plan was to unlock keys only under the game's exchange rules. As a deliberate addition
(2026-11-10), the DS screen also offers "Enable all keys", which needs no partner. It sets only the
enabled state, so nothing is marked as earned.

## Empty party

The whole-save check accepts an empty party only when every box is empty too, the state of a game
before its first Pokémon. Edits still can't empty a party (`check_party`).
