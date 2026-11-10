.PHONY: all nds test clean emu-harness emu-accept emu-ui emu-link emu-flashback emu-keys emu-unova

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

# Two consoles over local wireless: a genuine Memory Link (Phase H acceptance, P10 go/no-go).
emu-link: emu-harness
	rm -rf build/emu/memlink
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/memlink.json --out build/emu/memlink
	python3 tools/emu-harness/check_memlink.py corpus/real/pkhex-avalonia/gen5_white2.sav build/emu/memlink/inst0/b2-after.sav

# P10: retail Black 2 plays a flashback from a save linked by memlink::apply (needs roms/, bios/).
emu-flashback: emu-harness
	python3 tools/emu-harness/memlink_flashback.py

# P10: retail Black 2's Key System settings on a save that received keys (needs roms/, bios/).
emu-keys: emu-harness
	python3 tools/emu-harness/key_system_check.py

# P10: the DS screen links a save and writes it to the SD card (needs corpus/real).
emu-unova: emu-harness nds
	cargo build -q --release -p verity --example keys_receive
	rm -rf build/emu/unova-sd build/emu/unova-sd.img build/emu/unova-sd.img.idx build/emu/ui-unova
	mkdir -p build/emu/unova-sd/saves
	cp corpus/real/pkhex-avalonia/gen5_white2.sav "corpus/real/pkmds-blazor/Black - Full Completion.sav" build/emu/unova-sd/saves/
	target/release/examples/keys_receive corpus/real/pkhex-avalonia/gen5_white2.sav build/emu/unova-sd/saves/partner.sav partner
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-unova.json --out build/emu/ui-unova
	python3 tools/emu-harness/check_unova.py corpus/real/pkhex-avalonia/gen5_white2.sav build/emu/unova-sd/saves/gen5_white2.sav

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
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-gifts-w2.json --out build/emu/ui-gifts-w2
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-transfer.json --out build/emu/ui-transfer
	@if [ -f roms/platinum.nds ] && [ -f bios/bios7.bin ]; then \
	  tools/emu-harness/build/emu-harness tools/emu-harness/scripts/pt-loads-written.json --out build/emu/pt-loads-written; \
	else echo "skip pt-loads-written: ROM or BIOS missing"; fi
