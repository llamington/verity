# Memory Link findings (P10)

`verity::memlink` reproduces the game's Memory Link byte for byte. `memlink_accept.py` records
genuine links in the harness (Black 2 IREO hosting, White IRAO receiving over DS Download Play), and
`tests/p10_memlink.rs` compares the whole save: 17 of 17 match. Random seeds are taken from the
genuine result, since they can't be reproduced.

The 17 cases are 2 real saves, 14 single edits of them (each event flag group, the unlock word, event
variables, musical, Hall of Fame, gender, version, name and ID, and two Black 2 musical states), and
a second link onto an already linked save.

Sources: the field meanings come from the `fuddlesworth/pokebw2` decompilation, read for facts only
(`src/ov332/wb_save_convert.c`, `src/system/memory_link_savedata.c`, `src/ov331/save_outside.c`).
Its names are inferred, so the evidence for each rule is the match against genuine links.

## Region layout

`0x7E000..0x7EA1F` is the game's "outside" data, in three blocks. Each has the header
`u32 magic (0xA10F49AE)`, `u32 save count`, `u16 CRC16-CCITT of the data`, `u16 size`.

| Block | Offset | Data size | Contents |
|---|---|---|---|
| Gifts | `0x7E000` | `0x368` | Outside mystery gifts, encrypted |
| Gifts backup | `0x7E400` | `0x368` | Same; the newer valid copy is written to both |
| Keys | `0x7E800` | `0x214` | `KeySystemData`: Key System info, then the Memory Link data |

PKHeX's `Link1Data`/`Link2Data` (`0x400` each) are the two gift copies. The Memory Link data is the
keys block, so PKHeX isn't wrong about those offsets. BW_tool's "block 1" is really the gifts.

The header bytes follow from how the game loads the blocks. A block with no valid magic keeps the
raw count it read, so erased flash gives a keys count of 0 after `0xFFFFFFFF + 1`. New gifts are
zeroed first, giving count 1 and size 0.

New gifts are encrypted from `+0xC` to `+0x370` with the seed `checksum + (tail << 16)`, stored at
`+0x370`. `tail` is one frame-counter byte repeated, and never 0. BW_tool's loop stops 12 bytes short.

## Keys block

| Offset | Size | Meaning | Source in the BW save |
|---|---|---|---|
| `+0x0C` | `0x38` | Key System info; the last `u32` (`+0x40`) is its seed | Not from BW |
| `+0x44` | `0x18` each | 3 player records: events, musical, Hall of Fame | `0x19404` block, see below |
| `+0x8C` | 2 | 16 flag bits | See below |
| `+0x8E` | 6 | 3 event variables | Event work `0x4030`, `0x408E`, `0x40BC` |
| `+0x94` | `0x20` | Musical props | `0x1F958` |
| `+0xB4` | 2 | Musical total points | `0x1F956` |
| `+0xB6` | 2 | Musical shows played | `0x1F952` |
| `+0xB8` | `0x168` | Hall of Fame | `0x23B00` |

Player record: `u32 id`, 8 UTF-16 name units, gender, version, linked, padding. Sources are `0x19414`,
`0x19404`, `0x19421` and `0x1941F`.

Flag bits 0–7 come from event flags 2400, 2401, 2427 and, inverted, 681–685. Bits 8–15 are the low
byte of the word at `0x21204`.

## Merge rules

- Flags only get set. Variable 1 is skipped when the BW value is 0.
- Musical data is copied only if the BW show count is nonzero, the B2W2 save's "downloaded props" byte
  (`0x1F700 + 0x2A0`) is 0, and the B2W2 props differ from BW's or B2W2 already owns all 100.
- Hall of Fame is copied only if flag bit 0 is set.
- A player record is written only for a part that changed.
- If any part is already linked and this save would change a part owned by a different player, the
  game asks before replacing. `apply` returns `DifferentPlayer` unless `replace` is set.

The 8 flashbacks unlock on flag bits 8, 13, 0, 15, 12 and 0, then "musical player linked", then flag
bit 0. BW_tool's "8 flashback flags" at `+0x8D` are flag bits 8–15, not the 8 flashbacks.

## Refusals

verity links what the game links, including a save without event flag 2400: the game skips the Hall
of Fame part and the flashbacks that need that flag stay locked. It refuses:

- a source that isn't a valid Black or White save, or a target that isn't a valid B2W2 save
- corrupt gift or key blocks, which the game would clear and start again
- overwriting another player's data, unless `replace` is set (the game asks)

Only finished saves with this one flag cleared were tested, not saves that are really early in the
story. The download program on the BW console can report its own errors, and its code wasn't found.

### What the game does with corrupt blocks

Tested by flipping one byte in the keys block and in both gift copies of a linked save, then booting
retail Black 2 in the harness. After A on the title screen, it shows these on a blue screen, one per A
press, with no choice offered, before the normal menu:

> Your Mystery Gift data has been erased.

> Your Key System and Memory Link data has been erased.

The save was still byte-identical after the menu and Continue, so at that point the data is still
there. The decompilation (`SaveControl_ContinueSave`) clears the blocks on the next in-game save;
that step wasn't run. The messages come from one table entry per broken block
(`sBrokenSaveMessages`); only the case with both blocks corrupt was tested.

verity refuses instead, because the game's path destroys data the player can't choose to keep.

## In-game check

`make emu-flashback` boots retail Black 2 on a save linked by `memlink::apply` (source: a finished
Black save, trainer Lynx) and on an unlinked control. Both copies have event flag 2400 set and start
in Nuvema Town, one zone south of the trigger. The two runs are identical until Route 1. Then:

- Linked: Bianca's flashback starts, names "Lynx", and runs through to its trainer battle.
- Control: the player walks over the trigger and nothing happens.
- The Memory Link menu has a MEMORY LIST entry on the linked save only.

The script that arms the flashback runs on entering the zone, not on Continue, so a save placed
directly on Route 1 shows nothing. Flag 2400 is set by the helper because the test save hasn't got
it; the map script requires it. One of the 8 flashbacks was played, on Black 2 only.

## Key System (R5)

The game keeps a copy of the key info (`+0x0C` in the keys block) outside the save, but reads main
block 69 (`0x25800`, key info at `+0x28`) whenever a save exists, so `memlink::keys` writes that one.
Each of the 5 keys (Easy, Challenge, City, Iron, Iceberg) has two states, stored exclusive-ored with
the seed at `+0x34`. PKHeX calls them obtained and unlocked; the decompilation calls them unlocked
and enabled, used here.

Rules of the exchange (`KeySystemKeyState_Receive`), which `receive` follows:

- A key is shareable if the partner save *unlocked* it, meaning that save earned it.
- Receiving sets only *enabled*, so a received key can't be passed on.
- The city key needs the other version: Black 2 to White 2, or the reverse. Every other key doesn't.
- The sender's save doesn't change.

Keys are earned at the end of the story or in the Underground Ruins, and verity doesn't grant them:
there must be a partner save that earned the key. `make emu-keys` confirms the difficulty, city and
chamber settings unlock in retail Black 2 for all five keys, against a save with none.

## DS screen

On a B2W2 save, "Memory Link and keys" lists the other saves on the SD card. A Black or White save
offers Memory Link and a Black 2 or White 2 save offers its keys, with a preview before anything
changes. `make emu-unova` drives both and checks the file written to the SD card.

## Not done

- Linking a Japanese or Korean BW save, which hasn't been tried.

## Harness reliability

Links can still drop mid-download; the game then says "Unable to connect" and leaves the save
untouched. At 4 parallel runs this cost a retry in 4 of the 17 acceptance links. `memlink_run.py`
retries up to 3 times when the save comes back unchanged.
