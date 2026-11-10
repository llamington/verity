#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Checks the save written by ui-unova.json: only the Memory Link region and the key block may
change, and the link must have enabled the partner's keys. check_unova.py <before> <after>"""
import struct
import sys

REGION = range(0x7E000, 0x7EA20)
KEYS = range(0x25800, 0x25860)
FOOTERS = (0x25862, 0x25863, 0x25F8A, 0x25F8B, 0x25FA2, 0x25FA3)
ENABLED = 0x25800 + 0x28 + 0x14
SEED = 0x25800 + 0x28 + 0x34
ENABLED_MAGIC = (0x93389, 0x22843, 0x34771, 0xAB031, 0xB3818)


def enabled(save):
    seed = struct.unpack_from("<I", save, SEED)[0]
    return [struct.unpack_from("<I", save, ENABLED + i * 4)[0] ^ seed == m
            for i, m in enumerate(ENABLED_MAGIC)]


def main(before_path, after_path):
    before, after = open(before_path, "rb").read(), open(after_path, "rb").read()
    changed = [i for i in range(len(before)) if before[i] != after[i]]
    outside = [i for i in changed if i not in REGION and i not in KEYS and i not in FOOTERS]
    if outside:
        sys.exit(f"{len(outside)} bytes changed elsewhere, first at {outside[0]:#x}")
    if not any(i in REGION for i in changed):
        sys.exit("the Memory Link region did not change")
    was, now = enabled(before), enabled(after)
    if not any(n and not w for w, n in zip(was, now)):
        sys.exit(f"no key was enabled: {was} -> {now}")
    print(f"ok: {len(changed)} bytes changed, keys {was} -> {now}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
