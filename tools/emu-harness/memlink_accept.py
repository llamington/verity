#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Memory Link acceptance: genuine Memory Links for varied saves, for tests/memlink.rs to compare.

Each case gets build/emu/memlink-accept/<name>/ with bw.sav, b2-before.sav and inst0/b2-after.sav.
A case the game won't link leaves no b2-after.sav and is reported as refused.
"""
import concurrent.futures
import os
import shutil
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.dont_write_bytecode = True
sys.path.insert(0, HERE)
import bw_hybrid  # noqa: E402

OUT = os.path.join(ROOT, "build", "emu", "memlink-accept")
BLACK = os.path.join(ROOT, "corpus/real/pkhex-avalonia/gen5_black.sav")
FULL = os.path.join(ROOT, "corpus/real/pkmds-blazor/Black - Full Completion.sav")
B2 = os.path.join(ROOT, "corpus/real/pkhex-avalonia/gen5_white2.sav")

EVENT_FLAGS = 0x20100 + 0x27C
UNLOCK_WORD = 0x21204
MUSICAL = 0x1F700
PLAYER = 0x19404


def flag(save, number, value):
    mask = 1 << (number % 8)
    save[EVENT_FLAGS + number // 8] = (save[EVENT_FLAGS + number // 8] & ~mask) | (mask if value else 0)


def word(save, offset, value):
    save[offset : offset + 2] = struct.pack("<H", value)


def clear(*flags):
    def edit(save):
        for number in flags:
            flag(save, number, False)

    return edit


def set_flags(save):
    for number in range(681, 686):
        flag(save, number, True)


def unlock_word(value):
    def edit(save):
        save[UNLOCK_WORD : UNLOCK_WORD + 4] = struct.pack("<I", value)

    return edit


def event_vars(save):
    word(save, 0x20100 + 0x30 * 2, 2)
    word(save, 0x20100 + 0x8E * 2, 0)
    word(save, 0x20100 + 0xBC * 2, 3)


def musical_on(save):
    word(save, MUSICAL + 0x252, 3)
    word(save, MUSICAL + 0x256, 99)


def hall_zero(save):
    save[0x23B00 : 0x23B00 + 0x168] = bytes(0x168)


def female_white(save):
    save[PLAYER + 0x1D] = 1
    save[PLAYER + 0x1B] = 0x14


def renamed(save):
    name = "Tess".encode("utf-16le") + b"\xff\xff"
    save[PLAYER : PLAYER + 16] = name.ljust(16, b"\0")
    save[PLAYER + 0x10 : PLAYER + 0x14] = struct.pack("<I", 0x12345678)


def b2_downloaded(save):
    save[MUSICAL + 0x2A0] = 1


def b2_same_props(bw_path):
    def edit(save):
        props = open(bw_path, "rb").read()[MUSICAL + 0x258 : MUSICAL + 0x278]
        save[MUSICAL + 0x258 : MUSICAL + 0x278] = props

    return edit


def nothing(_):
    pass


# name: (BW source, BW edit, B2W2 edit)
CASES = {
    "black": (BLACK, nothing, nothing),
    "full": (FULL, nothing, nothing),
    "no2400": (BLACK, clear(2400), nothing),
    "no2401": (BLACK, clear(2401), nothing),
    "no2427": (BLACK, clear(2427), nothing),
    "set681_685": (BLACK, set_flags, nothing),
    "word0": (BLACK, unlock_word(0), nothing),
    "word55": (BLACK, unlock_word(0x55), nothing),
    "vars": (BLACK, event_vars, nothing),
    "musical_on": (BLACK, musical_on, nothing),
    "hall_zero": (BLACK, hall_zero, nothing),
    "female_white": (BLACK, female_white, nothing),
    "renamed": (BLACK, renamed, nothing),
    "full_no2400": (FULL, clear(2400), nothing),
    "b2_downloaded": (FULL, nothing, b2_downloaded),
    "b2_same_props": (BLACK, musical_on, b2_same_props(BLACK)),
}


def link(name, bw_data, b2_data):
    out = os.path.join(OUT, name)
    os.makedirs(out)
    bw_path = os.path.join(out, "bw.sav")
    b2_path = os.path.join(out, "b2-before.sav")
    open(bw_path, "wb").write(bw_data)
    open(b2_path, "wb").write(b2_data)
    run = [sys.executable, os.path.join(HERE, "memlink_run.py"), bw_path, out, b2_path]
    done = subprocess.run(run, capture_output=True, text=True)
    after = os.path.join(out, "inst0", "b2-after.sav")
    if done.returncode:
        if os.path.exists(after):
            os.remove(after)
        return f"{name}: refused ({done.stderr.strip()[-60:]})"
    return f"{name}: {done.stdout.strip()}"


def first_stage(name):
    source, bw_edit, b2_edit = CASES[name]
    bw = bytearray(open(source, "rb").read())
    bw_edit(bw)
    bw_hybrid.fix_checksums(bw, bw_hybrid.blocks())
    b2 = bytearray(open(B2, "rb").read())
    b2_edit(b2)
    bw_hybrid.fix_checksums(b2, bw_hybrid.blocks("BLOCKS_B2W2"))
    return link(name, bytes(bw), bytes(b2))


def main():
    shutil.rmtree(OUT, ignore_errors=True)
    os.makedirs(OUT)
    with concurrent.futures.ThreadPoolExecutor(4) as pool:
        for line in pool.map(first_stage, CASES):
            print(line, flush=True)
    # Re-link: the same player adds what an earlier link lacked, onto a region that already exists.
    previous = os.path.join(OUT, "word0", "inst0", "b2-after.sav")
    if os.path.exists(previous):
        print(link("relink", open(BLACK, "rb").read(), open(previous, "rb").read()), flush=True)
    print("done", flush=True)


if __name__ == "__main__":
    main()
