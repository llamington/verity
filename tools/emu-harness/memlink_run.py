#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Run a genuine Memory Link with a given BW save: memlink_run.py <bw.sav> <out dir> [b2w2.sav]

Writes the linked B2W2 save to <out dir>/inst0/b2-after.sav. Used for P10's differential runs.
"""
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
ATTEMPTS = 3


def main(bw, out, b2w2=None):
    text = open(os.path.join(HERE, "scripts", "memlink.json")).read()
    cfg = json.loads(re.sub(r"^\s*//.*$", "", text, flags=re.M))
    for inst in cfg["instances"]:
        for key in ("rom", "bios_dir"):
            inst[key] = os.path.normpath(os.path.join(HERE, "scripts", inst[key]))
    cfg["instances"][1]["save"] = os.path.abspath(bw)
    cfg["instances"][0]["save"] = (
        os.path.abspath(b2w2)
        if b2w2
        else os.path.normpath(os.path.join(HERE, "scripts", cfg["instances"][0]["save"]))
    )
    os.makedirs(out, exist_ok=True)
    path = os.path.join(out, "run.json")
    with open(path, "w") as f:
        json.dump(cfg, f)
    harness = os.path.join(HERE, "build", "emu-harness")
    log = open(os.path.join(out, "stderr.log"), "w")
    before = open(cfg["instances"][0]["save"], "rb").read()
    # The link can still drop mid-download; the game then leaves the save untouched.
    for attempt in range(1, ATTEMPTS + 1):
        done = subprocess.run([harness, path, "--out", out], stdout=subprocess.DEVNULL, stderr=log)
        if done.returncode:
            sys.exit(done.returncode)
        if open(os.path.join(out, "inst0", "b2-after.sav"), "rb").read() != before:
            print(f"linked on attempt {attempt}")
            return
    sys.exit(f"no Memory Link written in {ATTEMPTS} attempts")


if __name__ == "__main__":
    main(*sys.argv[1:])
