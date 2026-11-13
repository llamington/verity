#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""In-game Key System check: retail Black 2's settings screens, on a save with no keys and on
one that received all five through `memlink::keys::receive`. Locked options must differ on each
screen. Output goes to build/emu/keys/{blank,receive}.
"""
import hashlib
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(ROOT, "build/emu/keys")
SAVE = os.path.join(ROOT, "corpus/real/pkhex-avalonia/gen5_white2.sav")
HELPER = os.path.join(ROOT, "target/release/examples/keys_receive")
HARNESS = os.path.join(HERE, "build/emu-harness")
SCREENS = ["difficulty", "city", "chamber"]


def press(key, wait, frames=4):
    return [{"press": [key], "frames": frames}, {"wait": wait}]


def steps():
    out = [{"wait": 1850}] + press("Start", 120, 2) + press("A", 300, 2)
    out += press("Down", 30) * 3 + press("A", 400) + press("A", 400) + press("Down", 30) + press("A", 400)
    for i, name in enumerate(SCREENS):
        out += press("Down", 30) * min(i, 1) + press("A", 300) + [{"screenshot": name + ".png"}]
        out += press("B", 300)
    return out


def digest(mode, name):
    return hashlib.sha256(open(os.path.join(OUT, mode, name + ".png"), "rb").read()).hexdigest()


def main():
    os.makedirs(OUT, exist_ok=True)
    subprocess.check_call(["cargo", "build", "-q", "--release", "-p", "verity", "--example", "keys_receive"], cwd=ROOT)
    runs = []
    for mode in ("blank", "receive"):
        save = os.path.join(OUT, mode + ".sav")
        subprocess.check_call([HELPER, SAVE, save, mode], stdout=subprocess.DEVNULL)
        script = os.path.join(OUT, mode + ".json")
        cfg = {"rom": os.path.join(ROOT, "roms/black2.nds"), "bios_dir": os.path.join(ROOT, "bios"),
               "save": save, "steps": steps()}
        json.dump(cfg, open(script, "w"))
        runs.append(subprocess.Popen([HARNESS, script, "--out", os.path.join(OUT, mode)], stdout=subprocess.DEVNULL))
    if any(p.wait() for p in runs):
        sys.exit("harness run failed")
    same = [s for s in SCREENS if digest("blank", s) == digest("receive", s)]
    if same:
        sys.exit(f"no difference on: {same}")
    print("ok: every settings screen changed")


if __name__ == "__main__":
    main()
