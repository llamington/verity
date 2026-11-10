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

## Not done

- The Key System (R5). `+0x0C` is preserved or initialised, never edited.
- The in-game check that the Memory Link indicator shows and a flashback can be triggered.
- A DS screen for this.
- Linking a Japanese or Korean BW save, which hasn't been tried.

## Harness reliability

Links can still drop mid-download; the game then says "Unable to connect" and leaves the save
untouched. At 4 parallel runs this cost a retry in 4 of the 17 acceptance links. `memlink_run.py`
retries up to 3 times when the save comes back unchanged.
