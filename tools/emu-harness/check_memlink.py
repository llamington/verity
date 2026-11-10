#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Check a Memory Link run: the B2W2 save must change, and only inside the Memory Link region."""
import sys

START, SIZE = 0x7E000, 0xA20


def main(before_path, after_path):
    before = open(before_path, "rb").read()
    after = open(after_path, "rb").read()
    if len(before) != len(after):
        sys.exit("save size changed")
    changed = [i for i in range(len(before)) if before[i] != after[i]]
    if not changed:
        sys.exit("Memory Link wrote nothing")
    outside = [i for i in changed if not START <= i < START + SIZE]
    if outside:
        sys.exit(f"{len(outside)} bytes changed outside the region, first at {outside[0]:#x}")
    print(f"ok: {len(changed)} bytes changed in {changed[0]:#x}..{changed[-1]:#x}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
