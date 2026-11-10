#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""P11 in-game acceptance: a Pokémon added on the DS build survives the retail game.

For each game whose ROM is present: add a Pokémon with the wizard, stand next to a PC, open the box,
open the summary, save in-game and dump the save. The dump must still open in verity and PKHeX, and
the Pokémon must still be legal. Output goes to build/emu/p11; the report to corpus/reports/p11.
"""
import datetime
import glob
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(ROOT, "build/emu/p11")
HARNESS = os.path.join(HERE, "build/emu-harness")
PLACE = os.path.join(ROOT, "target/release/examples/p11_place")
ORACLE = os.path.join(ROOT, "tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle")
REPORT = os.path.join(ROOT, "corpus/reports/p11")
ROMS = os.environ.get("PKDS_ROM_DIR", os.path.join(ROOT, "roms"))
BIOS = os.environ.get("PKDS_BIOS_DIR", os.path.join(ROOT, "bios"))
CORPUS = "corpus/real/"


def press(key, wait, frames=4):
    return [{"press": [key], "frames": frames}, {"wait": wait}]


GEN5_BOOT = [{"wait": 1850}] + press("Start", 120, 2) + press("A", 300, 2) + press("A", 500)
GEN5_BOOT += press("Down", 30) + press("A", 500) + press("A", 600)

WHITE_BOOT = GEN5_BOOT + press("Down", 30) + press("A", 500) + press("A", 600)

GAMES = {
    "platinum": {"rom": "platinum.nds", "save": CORPUS + "pkhex-avalonia/gen4_platinum.sav", "add": "ui-add",
                 "at": (6, 12, 4),
                 "boot": [{"wait": 1500}] + press("Start", 120) + press("A", 150) + press("A", 300) + press("A", 300) + press("B", 400)},
    "heartgold": {"rom": "heartgold.nds", "save": CORPUS + "pkhex-avalonia/gen4_heartgold.sav", "add": "ui-add-hgss",
                  "at": (69, 11, 13),
                  "boot": [{"wait": 1500}] + press("Start", 200) + press("Start", 200) + press("A", 500)},
    "white": {"rom": "white.nds", "save": CORPUS + "pkhex-avalonia/gen5_black.sav", "add": "ui-add-w2",
              "at": (41, 4, 12), "boot": WHITE_BOOT, "save_downs": 2},
    "black2": {"rom": "black2.nds", "save": CORPUS + "pkhex-avalonia/gen5_white2.sav", "add": "ui-add-w2",
               "at": (41, 4, 12), "boot": GEN5_BOOT, "clear": (23, 19)},
}
MISSING = ["diamond", "pearl", "soulsilver", "black", "white2"]


def run(cfg, name, out):
    os.makedirs(out, exist_ok=True)
    script = os.path.join(out, name + ".json")
    json.dump(cfg, open(script, "w"))
    code = subprocess.run([HARNESS, script, "--out", os.path.join(out, name)], stdout=subprocess.DEVNULL,
                          stderr=subprocess.DEVNULL).returncode
    if code:
        raise RuntimeError(f"{name}: harness exit {code}")


def wizard(game, g, out):
    """Adds a Pokémon on the DS build; returns (before, edited) save paths."""
    text = open(os.path.join(HERE, "scripts", g["add"] + ".json")).read()
    cfg = json.loads(re.sub(r"^\s*//.*$", "", text, flags=re.M))
    sd = os.path.join(out, "sd")
    shutil.rmtree(sd, ignore_errors=True)
    os.makedirs(os.path.join(sd, "saves"))
    for f in ("sd.img", "sd.img.idx"):
        if os.path.exists(os.path.join(out, f)):
            os.remove(os.path.join(out, f))
    before = os.path.join(ROOT, g["save"])
    edited = os.path.join(sd, "saves", "game.sav")
    if "clear" in g:
        cleared = os.path.join(out, "cleared.sav")
        subprocess.check_call([PLACE, "clear", before, cleared, *map(str, g["clear"])])
        before = cleared
    shutil.copy(before, edited)
    steps, first_a = cfg["steps"], next(i for i, s in enumerate(cfg["steps"]) if s.get("press") == ["A"])
    # The SD card holds one save, so the picker needs no moving.
    steps = [s for i, s in enumerate(steps) if not (i < first_a and s.get("press") == ["Down"])]
    if "save_downs" in g:  # the script walks the Black 2/White 2 menu, which has one more row
        last_down = max(i for i, s in enumerate(steps) if s.get("press") == ["Down"])
        del steps[last_down]
    run({"rom": os.path.join(ROOT, "nds/verity.nds"), "sd_dir": sd, "sd_image": os.path.join(out, "sd.img"),
         "rtc": cfg.get("rtc", [2026, 11, 10, 12, 0, 0]), "steps": steps}, "wizard", out)
    return before, edited


def tiles(key, n):
    return [x for _ in range(n) for x in ({"hold": [key], "frames": 16}, {"wait": 6})]


def in_game_save(game):
    if game == "platinum":
        return press("X", 60) + press("Down", 8) * 4
    if game == "heartgold":
        return press("X", 60) + press("Right", 8) + press("Down", 8)
    return press("X", 120) + [{"touch": [60, 140], "frames": 4}, {"wait": 150}] + press("A", 60)


def pc_flow(game, g):
    steps = list(g["boot"]) + press("Up", 30, 6) + [{"screenshot": "0-at-pc.png"}]
    steps += press("A", 170) * 4 + press("Down", 30) + press("A", 400) + [{"screenshot": "1-box.png"}]
    steps += press("A", 90) + [{"screenshot": "2-menu.png"}] + press("Down", 30) + press("A", 400)
    steps += [{"screenshot": "3-summary.png"}]
    # Enough B presses to leave the PC in every game; in the field they do nothing.
    for i in range(9):
        steps += press("B", 200) + [{"screenshot": f"4-exit{i}.png"}]
    steps += [{"wait": 120}] + in_game_save(game) + [{"screenshot": "5-menu.png"}]
    if game in ("platinum", "heartgold"):
        steps += press("A", 200) + press("A", 120) + press("A", 700)
    else:
        steps += press("A", 200) + press("A", 200) + press("A", 900)
    return steps + [{"screenshot": "6-saved.png"}, {"dump_save": "after.sav"}]


def oracle(*args):
    env = dict(os.environ, DOTNET_ROOT=os.path.expanduser("~/.dotnet"))
    done = subprocess.run([ORACLE, *args], env=env, capture_output=True, text=True, check=True)
    return done.stdout


def checkpoints_differ(out):
    """A frozen game shows the same picture at every checkpoint."""
    names = ["0-at-pc", "1-box", "3-summary", "6-saved"]
    pictures = [open(os.path.join(out, "game", n + ".png"), "rb").read() for n in names]
    return len(set(pictures)) == len(pictures)


def run_game(game, g):
    out = os.path.join(OUT, game)
    before, edited = wizard(game, g, out)
    if open(before, "rb").read() == open(edited, "rb").read():
        return "the wizard wrote nothing"
    placed = os.path.join(out, "placed.sav")
    res = subprocess.run([PLACE, before, edited, placed, *map(str, g["at"])], capture_output=True, text=True, check=True)
    box, pid = res.stdout.split()
    run({"rom": os.path.join(ROMS, g["rom"]), "bios_dir": BIOS, "save": placed, "steps": pc_flow(game, g)}, "game", out)
    if not checkpoints_differ(out):
        return "the screen stopped changing"
    after = os.path.join(out, "game", "after.sav")
    res = subprocess.run([PLACE, "check", placed, after, box, pid], capture_output=True, text=True)
    if res.returncode:
        return res.stdout.strip()[-120:] or res.stderr[-120:]
    dump = os.path.join(out, "dump")
    shutil.rmtree(dump, ignore_errors=True)
    oracle("sav-dump", after, dump)
    found = [f for f in glob.glob(os.path.join(dump, f"box{int(box):02d}_00.*")) if not f.endswith(".enc")]
    if len(found) != 1:
        return "PKHeX did not export the slot"
    verdict = json.loads(oracle("legality", found[0]))
    return None if verdict.get("valid") else "PKHeX says it is not legal"


EXTRAS = [
    ("Mystery gift collection", ["python3", "tools/emu-harness/collect_gifts4.py"], ("platinum.nds", "heartgold.nds")),
    ("Memory Link flashback", ["python3", "tools/emu-harness/memlink_flashback.py"], ("black2.nds",)),
    ("Key System exchange", ["python3", "tools/emu-harness/key_system_check.py"], ("black2.nds",)),
    ("Enable all keys", ["python3", "tools/emu-harness/key_enable_all_check.py"], ("black2.nds",)),
]


def main():
    subprocess.check_call(["cargo", "build", "-q", "--release", "-p", "verity", "--example", "p11_place"], cwd=ROOT)
    results = {}
    for game, g in GAMES.items():
        if not os.path.exists(os.path.join(ROMS, g["rom"])):
            results[game] = "skipped: no ROM"
        else:
            try:
                results[game] = run_game(game, g) or "pass"
            except (RuntimeError, subprocess.CalledProcessError) as e:
                results[game] = f"error: {e}"[:160]
        print(f"{game}: {results[game]}", flush=True)
    for game in MISSING:
        results[game] = "skipped: no ROM or setup for this game yet"
        print(f"{game}: {results[game]}", flush=True)
    if "--games-only" not in sys.argv:
        for name, cmd, roms in EXTRAS:
            if not all(os.path.exists(os.path.join(ROMS, r)) for r in roms):
                results[name] = "skipped: no ROM"
            else:
                done = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
                results[name] = "pass" if done.returncode == 0 else f"fail: {done.stdout[-120:]}{done.stderr[-120:]}"
            print(f"{name}: {results[name]}", flush=True)
    os.makedirs(REPORT, exist_ok=True)
    stamp = datetime.date.today().isoformat()
    json.dump({"date": stamp, "results": results}, open(os.path.join(REPORT, f"{stamp}.json"), "w"), indent=1)
    failed = [k for k, v in results.items() if v != "pass" and not v.startswith("skipped")]
    print(f"{len(failed)} failed" if failed else "all run checks passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
