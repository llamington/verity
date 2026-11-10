#!/bin/bash
# Runs every DS walk-through (ui-*.json) with stand-in Black 2 and Platinum ROMs on the SD card,
# so the ROM font, icons, ribbons, items and move data are loaded: `make emu-tour`. The caches are
# built once (ui-rom-font.json) and copied in; each script gets fresh saves. Screenshots land in
# build/emu/tour/<script>/. A script that finishes says "ok"; check its screenshots for the result.
set -u
cd ~/verity
H=tools/emu-harness/build/emu-harness
T=build/emu/tour; rm -rf $T; mkdir -p $T
fresh() { rm -rf build/emu/sd build/emu/sd.img build/emu/sd.img.idx; cp -r sd build/emu/sd; rm -rf build/emu/sd/_nds;
  mkdir -p build/emu/sd/roms; cp $T/../tour-roms/*.nds build/emu/sd/roms/; }
mkdir -p build/emu/tour-roms
for g in black2 platinum; do python3 tools/emu-harness/standin_rom.py roms/$g.nds build/emu/tour-roms/$g.nds; done
fresh; $H tools/emu-harness/scripts/ui-rom-font.json --out $T/ui-rom-font > /dev/null 2>&1
cp -r build/emu/sd/_nds $T/../tour-nds; rm -f $T/../tour-nds/verity/last.txt
ls $T/../tour-nds/verity
for f in tools/emu-harness/scripts/ui-*.json; do n=$(basename $f .json); [ $n = ui-rom-font ] && continue
  fresh; cp -r $T/../tour-nds build/emu/sd/_nds
  r=$($H $f --out $T/$n 2>&1 | tail -1); echo "$n: $r"; done
