# ROM text: move descriptions (M8, 2026-11-10)

The move cache (`moves4.vmov` / `moves5.vmov`, version 2) carries each move's description from the
ROM's message bank, next to category, power and accuracy. Format in `moves::CachedMoves`; decoding
in `verity-rom::text`. The descriptions are in the ROM's language. Gen 4 codes become Unicode
through the core crate's table (in `verity-ffi`); verity-rom stays independent of the core crate.

| Games | Archive | Bank | Lines |
| --- | --- | --- | --- |
| Diamond/Pearl | `msgdata/msg.narc` | 587 (pokediamond `narc_0587.gmm`; no ROM here) | 468 |
| Platinum | `msgdata/pl_msg.narc` | 646 (`TEXT_BANK_MOVE_DESCRIPTIONS`) | 468 |
| HeartGold/SoulSilver | `a/0/2/7` | 749 | 468 |
| Black/White | `a/0/0/2` | 202 | 560 |
| Black 2/White 2 | `a/0/0/2` | 402 | 560 |

Found by searching every bank for Tackle's text. Gen 4 breaks a description into 5 short lines and
Gen 5 into 3 longer ones; verity joins them with spaces (none after a hyphen), drops control codes
and trims, and the DS re-wraps by pixel width (`ui::wrap`). The longest is about 125 bytes; the
move panel has four description rows.

Formats (see the `text` module docs): Gen 4 banks encrypt the entry table with the bank seed and
each string with a key from its index (pokeplatinum `src/message.c`); Gen 5 strings are UTF-16
XORed from the end with a rotating key (pokebw2 `tools/text/msgdata.py`). Both have 9-bit packed
lines (0xF100).

Item descriptions are in the same archives (Platinum bank 391, `TEXT_BANK_ITEM_DESCRIPTIONS`) and
aren't used yet.
