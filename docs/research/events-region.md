# Gen 4 event availability and delivery (R3, R4)

How Verity finds each event's region and language, and how Mystery Gift is unlocked, for Gen 4 and Gen 5.

## R3: which events belong on which save

PKHeX's database (`MGDB_G4`, 590 cards) has no language or region data. EventsGallery does, in
its folder and file names: `Released/Gen 4/Wondercards/<LANG>/<nnn> <games> - <name> (<region>)
(<LANG>).wc4`.

Method: `scripts/gen-events4-manifest.py` joins the two **byte for byte** and writes
`data/events4.tsv`. The join is exact: 590 files, 590 cards, one file per card, one language per
card. Each row carries a hash of PKHeX's bytes and `datagen` refuses to build if a future PKHeX
update changes a card, so the two cannot drift apart silently. The manifest is committed, so a
build does not need the EventsGallery checkout; it records the commit it came from.

| Field | Source | Notes |
|---|---|---|
| Language | Top-level folder | JPN, ENG, FRE, ITA, GER, SPA, KOR (35 Korean) |
| Games | The `<games>` token (`DP`, `Pt`, `DPPtHGSS`, `HGSS`, `D`, `P`) | See below |
| Region | `(NA)`, `(UK)`, `(AU)`, `(EU)` in the name | 17 cards; shown on screen |

**Games come from the file name, not the card.** The card's `validGames` field (0x14C, a bitmask
of `1 << version`) is unreliable here: 310 `DPPtHGSS` Battle Revolution item cards store `0xC00`
(D/P only), because they were converted from PGTs with placeholder metadata, as the gallery README
explains. A few values are garbage (`0xBD00`, `0xD700`, `0xDA00`).

**Region is displayed, not filtered.** A Gen 4 save does not record a console region, and within
one language the DS is region-free, so a save cannot say whether its owner is North American or
British. Cards tagged NA/UK/AU are therefore all offered to an English save and labelled. Only one
of them can be received, because they share a card ID.

**D/P and HG/SS pairs are not distinguished.** The save does not record which of the pair it is, so
either bit counts. This only matters for the two Japanese single-version cards (Festa Electabuzz
and Magmar), which are offered on any D/P save.

**Excluded (11 cards):** those with card ID 0 (no real Wonder Card, re-created by the community,
including the "Simulated" entries). They remain in the legality database.

Ranger Manaphy has no Wonder Card and is not in the database; it is added as a separate record for
every language except Korean (PKHeX `PGT.IsRangerManaphy`).

## R4: unlocking and the received flags

From PKHeX (`Saves/SAV4.cs`) and the pret decompilations (`pokeplatinum` `src/mystery_gift.c`,
`src/main_menu/mystery_gift_app.c`), checked against genuine saves:

- General block byte 72 bit 0: `IsMysteryGiftUnlocked`.
- Received flags: 2048 bits at the start of the Mystery Gift block, indexed by **card ID**. A
  genuine Platinum save that received cards 17, 18 and 20 has exactly those bits set.
- Bit 2047: PKHeX's `IsDeliveryManActive`, pret's `MysteryGiftUnlockedFlag`. Set along with the
  byte above.
- Slots: 8 pending gifts (PGT, 0x104 bytes) then 3 Wonder Cards (PCD, 0x358 bytes). D/P also has 11
  `u32` markers between the flags and the slots, `0xEDB88320` when a slot is in use.

What delivery writes, following `SaveReceivedGift`:

1. Encrypted inner PK4 for Pokémon gifts (PKHeX `VerifyPKEncryption`).
2. The card, in the first slot whose type is not 1 to 15, with `timesShared` 0, `sharesLeft` 0
   unless shareable, and `receivedDate` (`s32`, days since 2000-01-01, at 0x354).
3. A copy of the gift in the first free pending slot, its 2-bit `wondercardSlot` (byte 2) pointing
   at the card. Ranger Manaphy has no card and goes in the pending slot alone.
4. The received flag, bit 2047, the unlock bit, and the D/P markers.

Confirmed against genuine saves: comparing cards in real saves with the database, the only metadata
that differs is the received date (and the type, cleared on used cards). Every card in the database
has `hasWonderCard` and `savePgt` set, so card-only and gift-only paths are not needed.

The decompilation also shows a per-block CRC after the Mystery Gift block. All six saves tested
store zeros there, so it is not used by the retail game and is not written.

**The Platinum link base differs from PKHeX.** PKHeX's comment says D/P/Pt count from 1 and only
HG/SS from 0. The decompilation shows Platinum writes the 0-based slot, and the field is 2 bits
wide, so `4` could not be stored. verity follows the decompilation for Platinum and PKHeX for D/P.

## Why no direct "add event Pokémon" path

The deliveryman generates random-PID gifts (PID, IVs, shiny lock) himself at collection, using the
trainer's own IDs. Generating them ourselves would only apply to a path that writes the Pokémon
straight to a box, which the preferred delivery route does not need, so it was not built.

## In-game acceptance

`tools/emu-harness/collect_gifts4.py` injects a gift with `gift4_inject`, boots the retail game
headless, walks to the deliveryman, collects, saves in-game and checks the dumped save with PKHeX.
The deliveryman only appears when the game itself finds a valid pending gift, so his presence is
already evidence that the block is well formed.

Result (2026-11-09): 21 of 21 on retail Platinum and HeartGold: Member Card, Secret Key, Oak's
Letter, a Pokéwalker course, Enigma Stone, berries, Ranger Manaphy eggs, and fixed and random-PID
Pokémon. Every Pokémon is PKHeX-Legal and matched to its card.

Not run in-game: Diamond, Pearl and SoulSilver (no ROMs here). D/P is the one layout that differs,
with the markers; it is covered only by the PKHeX read-back.

# Gen 5 (PGF)

## Availability

`scripts/gen-events5-manifest.py` joins PKHeX's `MGDB_G5` to EventsGallery's
`Released/Gen 5/Wondercards` byte for byte, as for Gen 4: 709 files, 709 cards, one each, written
to `data/events5.tsv`. Language comes from the folder and games from the file name (`W`, `B`, `W2`,
`B2` and their combinations). Unlike Gen 4, a Gen 5 save records which game it is (PlayerData
0x1F), so a White-only card isn't offered on Black.

## Delivery

From PKHeX `MysteryBlock5` and genuine saves:

- Block 34 at 0x1C800, 0xA94 bytes. The first 0xA90 are one LCRNG-encrypted stream, keyed by the
  u32 stored after them. verity keeps the existing seed.
- Received flags: 2048 bits at the start, indexed by card ID. Then 12 cards of 0xCC bytes.
- Comparing cards in genuine saves with the database: only the received date (0xAC day, 0xAD
  month, 0xAE u16 year) and the state byte 0xB4 differ. Bit 1 of 0xB4 is set once collected, so
  a waiting card is stored exactly as distributed. There's no separate pending-gift list as in Gen 4.

Delivery writes the card to the first slot with card type 0, dates it, and sets its flag.

Verified end to end: PKHeX read back 31 deliveries across four real saves, and in Black 2 a White 2
save written this way had a waiting Meloetta collected from the deliveryman (the blue-capped man
near the Pokémon Center entrance), who then reported no other gifts. He refuses while the party
is full.
