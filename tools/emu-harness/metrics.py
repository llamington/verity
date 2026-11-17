#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Summarises the DS build's self-measurements (the harness's "metrics" lines in run.jsonl).

    metrics.py build/emu/tests [more dirs or run.jsonl files] [--frame-budget LINES]

Per script: heap in use at the start and end, the heap's high-water mark, the least free stack,
the worst frame (in scanlines, 263 a frame) and frames that ran late. A scene whose heap in use
grows after its first return (the first visit may fill caches) is listed as a possible leak. Exits 1 if a frame ran late or
passed the budget.
"""

import argparse
import json
import sys
from pathlib import Path

LINES = 263


def runs(paths):
    for p in map(Path, paths):
        if p.is_file():
            yield p.parent.name, p
        else:
            for f in sorted(p.rglob("run.jsonl")):
                yield f.parent.name, f


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("paths", nargs="+")
    ap.add_argument("--frame-budget", type=int, default=LINES)
    args = ap.parse_args()

    bad = False
    for name, path in runs(args.paths):
        # Before the first frame the hooks are still zero.
        rows = [json.loads(l) for l in path.open() if '"op":"metrics"' in l]
        rows = [r for r in rows if r["heap_used"] and r["stack_free"]]
        if not rows:
            continue
        worst = max(rows, key=lambda r: r["frame_lines_max"])
        late = rows[-1]["frames_late"]
        print(f"{name}: heap {rows[0]['heap_used']} -> {rows[-1]['heap_used']} B "
              f"(arena {max(r['heap_arena'] for r in rows)} B), "
              f"stack free {min(r['stack_free'] for r in rows)} B, "
              f"worst frame {worst['frame_lines_max']} lines ({worst['frame_lines_max'] * 100 // LINES}%) "
              f"at step {worst['step']} in {worst['scene']}, late {late}")
        if late or worst["frame_lines_max"] > args.frame_budget:
            bad = True

        # The heap each time a scene is back on top: after a visit elsewhere it should match.
        visits = {}
        prev = None
        for r in rows:
            if r["scene"] != prev:
                visits.setdefault(r["scene"], []).append(r)
            prev = r["scene"]
        for scene, rs in visits.items():
            used = [r["heap_used"] for r in rs]
            if len(used) > 2 and used[-1] > used[1]:
                print(f"  {scene}: heap {' -> '.join(map(str, used))} B on each return "
                      f"(steps {', '.join(str(r['step']) for r in rs)})")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
