.PHONY: all nds test clean emu-harness emu-accept emu-ui

all: nds

nds:
	$(MAKE) -C nds

test:
	cargo test --workspace

clean:
	cargo clean
	$(MAKE) -C nds clean

emu-harness:
	cmake -S tools/emu-harness -B tools/emu-harness/build -G Ninja -DCMAKE_BUILD_TYPE=Release
	ninja -C tools/emu-harness/build emu-harness

# Phase H acceptance (needs owner-dumped roms/ and bios/; skips otherwise).
emu-accept: emu-harness
	@for g in pt b2; do \
	  if [ -f bios/bios7.bin ] && [ -f roms/$$( [ $$g = pt ] && echo platinum || echo black2 ).nds ]; then \
	    tools/emu-harness/build/emu-harness tools/emu-harness/scripts/boot-$$g.json --out build/emu/boot-$$g || exit 1; \
	  else echo "skip boot-$$g: ROM or BIOS missing"; fi; \
	done

# Front-end walk-throughs (screenshots in build/emu/<script>), on a fresh copy of sd/ in
# build/emu/sd so runs never touch sd/ (melonDS folder sync also compares timestamps, and
# the harness clock is fixed, so a reused image would not sync rewritten files back).
emu-ui: emu-harness nds
	rm -rf build/emu/sd build/emu/sd.img build/emu/sd.img.idx
	mkdir -p build/emu && cp -r sd build/emu/sd && rm -rf build/emu/sd/_nds
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-boxes.json --out build/emu/ui-boxes
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-add.json --out build/emu/ui-add
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-edit.json --out build/emu/ui-edit
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-add-hgss.json --out build/emu/ui-add-hgss
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-gifts.json --out build/emu/ui-gifts
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-egg.json --out build/emu/ui-egg
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-pal-park.json --out build/emu/ui-pal-park
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-closest-ivs.json --out build/emu/ui-closest-ivs
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-misc-tabs.json --out build/emu/ui-misc-tabs
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-gen5.json --out build/emu/ui-gen5
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-add-w2.json --out build/emu/ui-add-w2
	@if [ -f roms/platinum.nds ] && [ -f bios/bios7.bin ]; then \
	  tools/emu-harness/build/emu-harness tools/emu-harness/scripts/pt-loads-written.json --out build/emu/pt-loads-written; \
	else echo "skip pt-loads-written: ROM or BIOS missing"; fi
