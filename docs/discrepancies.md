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
