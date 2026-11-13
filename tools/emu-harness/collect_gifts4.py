#!/usr/bin/env python3
"""Mystery Gift acceptance: injects Gen 4 gifts, collects each from the deliveryman in a retail game under
the headless harness, saves in-game, and asks PKHeX whether the result is right.

Needs owner ROMs and BIOS (roms/, bios/), real saves (corpus/real/), data blobs and the oracle.
Run from the repo root: python3 -I tools/emu-harness/collect_gifts4.py [out-dir] [only-game]

The player starts inside a Poke Mart (the save's position is patched by the gift4_inject helper)
and walks out and back in, because the game only decides whether to show the deliveryman on
entering a mart. Map IDs and coordinates are from the pret decompilations.
"""
import json
import os
import shutil
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
ORACLE = os.path.join(ROOT, "tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle")
HARNESS = os.path.join(ROOT, "tools/emu-harness/build/emu-harness")
INJECT = os.path.join(ROOT, "target/debug/examples/gift4_inject")
ENV = dict(os.environ, DOTNET_ROOT=os.path.expanduser("~/.dotnet"), CLEAR_SLOTS="1")
RANGER = 0xFFFF

# mgdb index -> (description, kind). Kinds: pokemon, item, egg.
GIFTS = {
    "platinum": [
        (17, "Member Card", "item"),
        (15, "Secret Key", "item"),
        (21, "Oak's Letter", "item"),
        (19, "Arceus", "pokemon"),
        (22, "Pichu", "pokemon"),
        (24, "Darkrai", "pokemon"),
        (25, "Riolu", "pokemon"),
        (28, "Jirachi", "pokemon"),
        (296, "Liechi Berry", "item"),
        (299, "Enigma Berry", "item"),
        (RANGER, "Ranger Manaphy", "egg"),
    ],
    "heartgold": [
        (30, "Yellow Forest course", "item"),
        (34, "Enigma Stone", "item"),
        (24, "Darkrai", "pokemon"),
        (25, "Riolu", "pokemon"),
        (28, "Jirachi", "pokemon"),
        (32, "Eevee", "pokemon"),
        (33, "Mew", "pokemon"),
        (296, "Liechi Berry", "item"),
        (299, "Enigma Berry", "item"),
        (RANGER, "Ranger Manaphy", "egg"),
    ],
}


def press(key, wait):
    return [{"press": [key], "frames": 2}, {"wait": wait}]


def walk(key, tiles=1):
    return [s for _ in range(tiles) for s in ({"hold": [key], "frames": 16}, {"wait": 4})]


GAMES = {
    "platinum": {
        "rom": "roms/platinum.nds",
        "save": "corpus/real/pkmds-blazor/Pokemon Platinum.sav",
        "map": (4, 3, 10),  # Jubilife Mart, the tile above the door
        "party": None,
        "boot": [{"wait": 1500}]
        + press("Start", 120)
        + press("A", 150)
        + press("A", 300)
        + press("A", 300)  # Continue opens the journal
        + press("B", 400),
        # Deliveryman at (4,7), facing south: stand at (3,7) and face right.
        "approach": walk("Up", 4) + [{"wait": 30}] + press("Right", 20),
        "to_save": ["Down"] * 4,  # the field menu is one column
    },
    "heartgold": {
        "rom": "roms/heartgold.nds",
        "save": "corpus/real/pkhex-avalonia/gen4_heartgold.sav",
        "map": (68, 4, 10),  # Cherrygrove Mart, the tile above the door
        "party": 5,
        "boot": [{"wait": 1500}] + press("Start", 200) + press("Start", 200) + press("A", 500),
        # Deliveryman at (2,8): stand at (2,9) and face up.
        "approach": walk("Up", 1) + walk("Left", 1) + [{"wait": 30}] + press("Up", 20),
        "to_save": ["Right", "Down"],  # the field menu is a grid
    },
}


def script(game, save, shots):
    g = GAMES[game]
    steps = list(g["boot"])
    steps += [{"hold": ["Down"], "frames": 24}, {"wait": 150}]
    steps += [{"hold": ["Up"], "frames": 60}, {"wait": 4}, {"hold": ["Up"], "frames": 60}, {"wait": 300}]
    steps += g["approach"]
    steps += [{"screenshot": "1-before.png"}]
    steps += press("A", 200)
    # B also advances text, and unlike A cannot start the conversation again.
    for i in range(6):
        steps += press("B", 240)
        if shots:
            steps.append({"screenshot": f"2-talk{i}.png"})
    steps += [{"wait": 200}]
    steps += press("X", 60)
    for key in g["to_save"]:
        steps += press(key, 8)
    steps += [{"screenshot": "3-menu.png"}]
    steps += press("A", 200)
    steps += press("A", 120)
    steps += press("A", 700)
    steps += [{"screenshot": "4-saved.png"}, {"dump_save": "after.sav"}]
    return {
        "rom": os.path.join(ROOT, g["rom"]),
        "bios_dir": os.path.join(ROOT, "bios"),
        "save": save,
        "steps": steps,
    }


def oracle(*args):
    out = subprocess.run([ORACLE, *args], env=ENV, capture_output=True, text=True, check=True)
    return json.loads(out.stdout)


def bag_total(info):
    return sum(i[1] for p in info["inventory"] for i in p["items"])


def run_one(game, index, name, kind, out):
    os.makedirs(out, exist_ok=True)
    g = GAMES[game]
    src = os.path.join(ROOT, g["save"])
    injected = os.path.join(out, "injected.sav")
    m, x, z = g["map"]
    cmd = [INJECT, src, injected, os.path.join(ROOT, "data/blobs"), str(index), str(m), str(x), str(z)]
    if g["party"]:
        cmd.append(str(g["party"]))
    subprocess.run(cmd, env=ENV, check=True)
    before = oracle("sav-info", injected)
    spec = script(game, injected, shots=True)
    path = os.path.join(out, "script.json")
    json.dump(spec, open(path, "w"))
    subprocess.run([HARNESS, path, "--out", out], env=ENV, check=True, capture_output=True)
    after_path = os.path.join(out, "after.sav")
    after = oracle("sav-info", after_path)
    result = {"game": game, "index": index, "name": name}
    pending = [p["type"] for p in after["mystery"]["pgt"] if p["type"]]
    result["collected"] = not pending
    result["checksums"] = "valid" in after["checksum_info"].lower()
    if kind in ("pokemon", "egg"):
        dump = os.path.join(out, "dump")
        shutil.rmtree(dump, ignore_errors=True)
        slots = oracle("sav-dump", after_path, dump)["slots"]
        party = [s for s in slots if s["name"].startswith("party_")]
        new = party[-1]
        report = oracle("legality", os.path.join(dump, new["name"] + ".pk4"))
        result.update(
            party_grew=after["party_count"] == before["party_count"] + 1,
            species=new["species"],
            is_egg=new["is_egg"],
            legal=report["valid"],
            encounter=report.get("encounter_match"),
            report=report["report"][:200],
        )
        result["pass"] = bool(
            result["collected"] and result["checksums"] and result["party_grew"] and result["legal"]
        )
    else:
        changed = bag_total(after) != bag_total(before) or after.get("event_flags_set") != before.get(
            "event_flags_set"
        )
        result["bag_or_flags_changed"] = changed
        result["pass"] = bool(result["collected"] and result["checksums"] and changed)
    return result


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "build/emu/collect-gifts4")
    only = sys.argv[2] if len(sys.argv) > 2 else None
    results = []
    for game, gifts in GIFTS.items():
        if only and game != only:
            continue
        for index, name, kind in gifts:
            r = run_one(game, index, name, kind, os.path.join(out, game, str(index)))
            print(json.dumps(r, ensure_ascii=False), flush=True)
            results.append(r)
    ok = sum(r["pass"] for r in results)
    print(f"{ok}/{len(results)} passed")
    json.dump(results, open(os.path.join(out, "results.json"), "w"), indent=1, ensure_ascii=False)
    sys.exit(0 if ok == len(results) else 1)


main()
