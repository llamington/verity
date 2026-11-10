#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""P10: "Enable all keys" on a save that has just started.

Starts a new game in retail Black 2 and saves in the player's room, enables every key on the DS
build of verity, then boots the written save in retail Black 2 and picks Challenge Mode. An
untouched copy of the early save is the control. Output goes to build/emu/keys-all.
"""
import json
import os
import shutil
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(ROOT, "build/emu/keys-all")
HARNESS = os.path.join(HERE, "build/emu-harness")
ROM = {"rom": os.path.join(ROOT, "roms/black2.nds"), "bios_dir": os.path.join(ROOT, "bios")}
UNOVA_DOWNS = 3
CHALLENGE = 0x49589


def press(key, wait, frames=4):
    return [{"press": [key], "frames": frames}, {"wait": wait}]


def title():
    return [{"wait": 1850}] + press("Start", 120, 2) + press("A", 200, 2)


def new_game():
    steps = title()
    for _ in range(12):
        steps += press("A", 70) * 12
    steps += press("X", 120)
    steps += press("A", 70) * 10 + [{"wait": 120}] + press("X", 120) + press("B", 90)
    steps += press("A", 70) * 10 + [{"wait": 120}] + press("X", 120)
    steps += [{"screenshot": "menu.png"}, {"touch": [60, 92], "frames": 4}, {"wait": 150}]
    # After a tap, the first button press only shows the cursor.
    steps += [{"screenshot": "ask.png"}] + press("A", 60) + press("A", 900) + [{"screenshot": "saved.png"}]
    return steps + press("A", 120) + [{"dump_save": "early.sav"}]


def verity_ui():
    steps = [{"wait": 90}, {"hold": ["L"], "frames": 40}, {"wait": 2}]
    steps += press("A", 60, 2) + press("Down", 5, 2) * 2 + press("A", 120, 2)
    steps += [{"screenshot": "list.png"}] + press("A", 60, 2) + [{"screenshot": "preview.png"}]
    steps += press("A", 30, 2) + [{"screenshot": "done.png"}] + press("B", 30, 2)
    return steps + press("Down", 5, 2) + press("A", 240, 2) + [{"screenshot": "saved.png"}]


def retail():
    steps = [{"wait": 1850}] + press("Start", 120, 2) + press("A", 300, 2)
    steps += [{"screenshot": "continue.png"}]
    steps += press("Down", 30) * UNOVA_DOWNS + press("A", 400) + [{"screenshot": "unova.png"}]
    steps += press("A", 400) + press("Down", 30) + press("A", 400)
    steps += press("A", 300) + [{"screenshot": "difficulty.png"}]
    steps += press("Down", 30) * 2 + press("A", 300) + [{"screenshot": "choose1.png"}]
    steps += press("A", 400) + [{"screenshot": "choose2.png"}]
    steps += press("A", 400) + [{"screenshot": "choose3.png"}]
    return steps + press("B", 300) * 3 + [{"screenshot": "left.png"}, {"dump_save": "after.sav"}]


def run(name, cfg, wait=True):
    script = os.path.join(OUT, name + ".json")
    json.dump(cfg, open(script, "w"))
    p = subprocess.Popen([HARNESS, script, "--out", os.path.join(OUT, name)], stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL)
    if wait and p.wait():
        sys.exit(f"{name} failed")
    return p


def main():
    os.makedirs(OUT, exist_ok=True)
    early = os.path.join(OUT, "new-game", "early.sav")
    if not os.path.exists(early):
        run("new-game", {**ROM, "steps": new_game()})
    sd = os.path.join(OUT, "sd")
    shutil.rmtree(sd, ignore_errors=True)
    os.makedirs(os.path.join(sd, "saves"))
    for f in ("sd.img", "sd.img.idx"):
        if os.path.exists(os.path.join(OUT, f)):
            os.remove(os.path.join(OUT, f))
    written = os.path.join(sd, "saves", "early.sav")
    shutil.copy(early, written)
    run("ui", {"rom": os.path.join(ROOT, "nds/verity.nds"), "sd_dir": sd,
               "sd_image": os.path.join(OUT, "sd.img"), "rtc": [2026, 11, 10, 12, 0, 0],
               "steps": verity_ui()})
    if open(early, "rb").read() == open(written, "rb").read():
        sys.exit("the DS build did not change the save")
    runs = [run("enabled", {**ROM, "save": written, "steps": retail()}, False),
            run("control", {**ROM, "save": early, "steps": retail()}, False)]
    if any(p.wait() for p in runs):
        sys.exit("retail run failed")
    key_info = 0x25800 + 0x28

    def difficulty(name):
        save = open(os.path.join(OUT, name, "after.sav"), "rb").read()
        value, seed = struct.unpack_from("<I", save, key_info + 0x28)[0], struct.unpack_from("<I", save, key_info + 0x34)[0]
        return value ^ seed if value else 0

    def shot(name):
        return open(os.path.join(OUT, name, "difficulty.png"), "rb").read()

    if shot("enabled") == shot("control"):
        sys.exit("the difficulty screen is the same on both saves")
    if difficulty("enabled") != CHALLENGE:
        sys.exit(f"Challenge Mode was not stored: {difficulty('enabled'):#x}")
    if difficulty("control") == CHALLENGE:
        sys.exit("the control save reached Challenge Mode")
    print("ok: retail Black 2 stored Challenge Mode on a just-started save")


if __name__ == "__main__":
    main()
