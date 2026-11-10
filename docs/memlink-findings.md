# Memory Link findings (P10)

Evidence comes from genuine Memory Links in the harness (`make emu-link`, `memlink_run.py`): Black 2
(IREO) hosting, White (IRAO) receiving over DS Download Play. Saves used: `gen5_black.sav`
("RoC"), `Black - Full Completion.sav` ("Lynx"), and hybrids of the two (`bw_hybrid.py`). Links run
under a fixed RTC, and repeat runs give identical bytes.

Status: partly decoded. This covers two real saves and their hybrids, not the 10 diverse saves P10's
acceptance needs. `memlink::build` is not written yet.

## Layout, all confirmed

The link changes only `0x7E000..0x7EA1F`. Offsets below are relative to the region or block.

| Part | Offset | Size | Notes |
|---|---|---|---|
| Block 1 | `+0x000` | `0x374` | CRC16-CCITT of `+0xC..+0x374` at `+0x8`; seed at `+0x370` |
| Mirror | `+0x400` | `0x374` | Byte-identical to block 1, including the seed |
| Block 2 | `+0x800` | `0x220` | Unencrypted. CRC of `+0xC..+0x220` at `+0x8`; `+0xA` holds `0x214`, the CRC'd length |

Both blocks start with the bytes `ae 49 0f a1`, the same for both saves. `+0x4` is 1 in block 1 and 0
in block 2.

The seed has the form `rand16 << 16` (`0x7D7D0000` under the fixed RTC).

Block 1's crypto covers `+0xC..+0x370`. BW_tool's loop treats the length as an end offset and stops
at `+0x364`; that round-trips in its own tool but leaves 12 bytes wrong against the game's format.
Block 1's plaintext is all zeros for both real saves, so its content is still unknown.

PKHeX labels `0x7E000` as `Link1Data` (`0x400`). The real write is `0xA20` long. Not yet compared
with PKHeX's source in detail.

## Block 2 fields

| Offset | Value | Source in the BW save |
|---|---|---|
| `+0x40` | `e0 f8 9a 90` | Constant across both saves; not found in the BW save |
| `+0x44`, `+0x46`, `+0x48` | TID, SID, name | `0x19414`, `0x19416`, `0x19404` |
| `+0x5C`, `+0x5E`, `+0x60` | TID, SID, name, conditional | Written only if `0x1F952` is nonzero |
| `+0x74`, `+0x76`, `+0x78` | TID, SID, name | Same as the first set |
| `+0x8D` | Flashback bits | `0xFF` for both saves; rules unknown |
| `+0x8E` | Starter | `0x20160` |
| `+0x90` | `1` | Constant across both saves |
| `+0x94` | `0x13` bytes, conditional | `0x1F958`, copied only if `0x1F952` is nonzero |
| `+0xB4` | u16 | `0x1F956` |
| `+0xB6` | u16 | `0x1F952` |
| `+0xB8` | `0x168` bytes | Hall of Fame, `0x23B00` |

BW_tool writes all three name slots and always copies the props, so its output differs from the
game's when `0x1F952` is zero.

How `0x1F952` was found: copying every block from the Full Completion save reproduces its output.
Bisecting the 69 data blocks isolates `0x1F700`, and patching just `0x1F952` into `gen5_black` fills
the slot and props. Patching only `0x1F956` does neither. What the game stores at `0x1F952` hasn't
been identified. The block's `0x1F958` field is presumably Musical props, which is unconfirmed.

## Still unknown

- The rule behind each flashback bit (both real saves give `0xFF`; needs early-progress saves).
- Block 1's content, and what makes it nonzero, if anything.
- `+0x58` (low byte 0 or 1) and `+0x8C`, which change with blocks not yet identified.
- The Key System (R5).

## Harness reliability

A link can drop mid-download; Black 2 then says "Unable to connect" and leaves the save untouched.
This happened in about 2 of 20 parallel runs at melonDS's 25 ms timeout. At 150 ms, 4 of 4 parallel
runs linked on the first try. `memlink_run.py` retries when the save comes back unchanged.
