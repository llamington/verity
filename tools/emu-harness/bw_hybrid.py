#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Copy save blocks between BW saves, keeping checksums valid (P10 differential runs).

bw_hybrid.py <base.sav> <donor.sav> <out.sav> all | <block containing this offset>...
"""
import os
import re
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TABLE = os.path.join(HERE, "..", "..", "crates", "verity", "src", "save", "gen5_blocks.rs")


def blocks():
    text = open(TABLE).read()
    text = text[text.index("BLOCKS_BW") : text.index("BLOCKS_B2W2")]
    row = r"offset: (0x[0-9A-F]+), length: (0x[0-9A-F]+), chk: (0x[0-9A-F]+), chk_mirror: (0x[0-9A-F]+)"
    return [tuple(int(v, 16) for v in m) for m in re.findall(row, text)]


def crc16(data):
    crc = 0xFFFF
    for byte in data:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021 if crc & 0x8000 else crc << 1) & 0xFFFF
    return crc


def main(base, donor, out, *which):
    data = bytearray(open(base, "rb").read())
    other = open(donor, "rb").read()
    table = blocks()
    for offset, length, _, _ in table[:-1]:  # the last block is the checksum table itself
        if "all" in which or any(offset <= int(w, 16) < offset + length for w in which):
            data[offset : offset + length] = other[offset : offset + length]
    for offset, length, chk, mirror in table:
        value = struct.pack("<H", crc16(data[offset : offset + length]))
        data[chk : chk + 2] = value
        data[mirror : mirror + 2] = value
    open(out, "wb").write(data)


if __name__ == "__main__":
    main(*sys.argv[1:])
