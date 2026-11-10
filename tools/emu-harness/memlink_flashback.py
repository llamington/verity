#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""P10's in-game check: does retail Black 2 honour a save linked by `memlink::apply`?

Walks from Nuvema Town onto Route 1 on a linked save and on an unlinked control. The flashback is
armed by the map's own script on entering the zone, so only the linked run may start it. Also opens
the Unova Link menu on both. Output goes to build/emu/flashback/{linked,control}{,-menu}.
"""
import hashlib
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(ROOT, "build/emu/flashback")
HARNESS = os.path.join(HERE, "build/emu-harness")
HELPER = os.path.join(ROOT, "target/release/examples/memlink_flashback")
BW = os.path.join(ROOT, "corpus/real/pkmds-blazor/Black - Full Completion.sav")
B2W2 = os.path.join(ROOT, "corpus/real/pkhex-avalonia/gen5_white2.sav")
NUVEMA, START_X, START_Z = "389", "788", "741"
ADVANCES = 44


def press(key, wait, frames=4):
    return [{"press": [key], "frames": frames}, {"wait": wait}]


def title():
    return [{"wait": 1850}] + press("Start", 120, 2) + press("A", 300, 2)


def field():
    steps = title() + press("A", 500) + press("Down", 30) + press("A", 500) + press("A", 600)
    steps += [{"screenshot": "00-start.png"}] + press("Up", 240, 176) + [{"screenshot": "01-walked.png"}]
    for i in range(ADVANCES):
        steps += press("A", 120) + [{"screenshot": f"{i + 2:02d}.png"}]
    return steps


def menu():
    steps = title()
    steps += press("Down", 30) * 3 + press("A", 400) + press("Down", 30) + press("A", 300)
    return steps + [{"screenshot": "memory-link.png"}]


def run(name, save, steps):
    script = os.path.join(OUT, name + ".json")
    cfg = {"rom": os.path.join(ROOT, "roms/black2.nds"), "bios_dir": os.path.join(ROOT, "bios"),
           "save": save, "steps": steps}
    json.dump(cfg, open(script, "w"))
    return subprocess.Popen([HARNESS, script, "--out", os.path.join(OUT, name)],
                            stdout=subprocess.DEVNULL)


def digest(name, shot):
    return hashlib.sha256(open(os.path.join(OUT, name, shot), "rb").read()).hexdigest()


def main():
    os.makedirs(OUT, exist_ok=True)
    subprocess.check_call(["cargo", "build", "-q", "--release", "-p", "verity", "--example",
                           "memlink_flashback"], cwd=ROOT)
    linked, control = os.path.join(OUT, "linked.sav"), os.path.join(OUT, "control.sav")
    for bw, out in ((BW, linked), ("-", control)):
        subprocess.check_call([HELPER, bw, B2W2, out, NUVEMA, START_X, START_Z])
    runs = [run("linked", linked, field()), run("control", control, field()),
            run("linked-menu", linked, menu()), run("control-menu", control, menu())]
    if any(p.wait() for p in runs):
        sys.exit("harness run failed")
    if digest("linked", "00-start.png") != digest("control", "00-start.png"):
        sys.exit("the runs differ before the zone boundary")
    if digest("linked", "01-walked.png") == digest("control", "01-walked.png"):
        sys.exit("the flashback did not start on the linked save")
    if digest("linked-menu", "memory-link.png") == digest("control-menu", "memory-link.png"):
        sys.exit("the Memory Link menu is the same on both saves")
    print("ok: flashback and Memory List on the linked save only")


if __name__ == "__main__":
    main()
