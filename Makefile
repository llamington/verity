.PHONY: all nds test clean emu-harness emu-accept emu-test emu-ui emu-link emu-flashback emu-keys emu-unova emu-keys-all emu-ingame emu-font emu-tour

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

# Boot acceptance (needs owner-dumped roms/ and bios/; skips otherwise).
emu-accept: emu-harness
	@for g in pt b2; do \
	  if [ -f bios/bios7.bin ] && [ -f roms/$$( [ $$g = pt ] && echo platinum || echo black2 ).nds ]; then \
	    tools/emu-harness/build/emu-harness tools/emu-harness/scripts/boot-$$g.json --out build/emu/boot-$$g || exit 1; \
	  else echo "skip boot-$$g: ROM or BIOS missing"; fi; \
	done

# Two consoles over local wireless: a genuine Memory Link.
emu-link: emu-harness
	rm -rf build/emu/memlink
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/memlink.json --out build/emu/memlink
	python3 tools/emu-harness/check_memlink.py corpus/real/pkhex-avalonia/gen5_white2.sav build/emu/memlink/inst0/b2-after.sav

# Retail Black 2 plays a flashback from a save linked by memlink::apply (needs roms/, bios/).
emu-flashback: emu-harness
	python3 tools/emu-harness/memlink_flashback.py

# Retail Black 2's Key System settings on a save that received keys (needs roms/, bios/).
emu-keys: emu-harness
	python3 tools/emu-harness/key_system_check.py

# In-game acceptance on every game with a ROM in roms/; the report goes to corpus/reports/ingame.
emu-ingame: emu-harness nds
	python3 tools/emu-harness/ingame_suite.py

# "Enable all keys" on a new game's first save, then Challenge Mode in retail Black 2.
emu-keys-all: emu-harness nds
	python3 tools/emu-harness/key_enable_all_check.py

# The DS screen links a save and writes it to the SD card (needs corpus/real).
emu-unova: emu-harness nds
	cargo build -q --release -p verity --example keys_receive
	rm -rf build/emu/unova-sd build/emu/unova-sd.img build/emu/unova-sd.img.idx build/emu/ui-unova
	mkdir -p build/emu/unova-sd/saves
	cp corpus/real/pkhex-avalonia/gen5_white2.sav "corpus/real/pkmds-blazor/Black - Full Completion.sav" build/emu/unova-sd/saves/
	target/release/examples/keys_receive corpus/real/pkhex-avalonia/gen5_white2.sav build/emu/unova-sd/saves/partner.sav partner
	tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-unova.json --out build/emu/ui-unova \
	  $(if $(UPDATE),--update)
	python3 tools/emu-harness/check_unova.py corpus/real/pkhex-avalonia/gen5_white2.sav build/emu/ui-unova/sd/saves/gen5_white2.sav

# Front-end walk-throughs (screenshots in build/emu/<script>), on a fresh copy of sd/ in
# build/emu/sd so runs never touch sd/ (melonDS folder sync also compares timestamps, and
# the harness clock is fixed, so a reused image would not sync rewritten files back).
# ROM font and icons: first boot builds the caches from a ROM on the SD card. Runs once with
# a Gen 5 ROM and once with a Gen 4 ROM (stand-ins holding only what verity reads, standin_rom.py).
# Their needles show ROM fonts and icons, so they live in the gitignored needles-local/<rom>.
EMU_FONT_TESTS := ui-keyboard-jp ui-moves ui-moves5 ui-items ui-items5 ui-ribbons
emu-font: emu-harness nds
	for g in black2 platinum; do \
	  rm -rf build/emu/sd build/emu/sd.img build/emu/sd.img.idx && mkdir -p build/emu && \
	  cp -r sd build/emu/sd && rm -rf build/emu/sd/_nds && mkdir -p build/emu/sd/roms && \
	  python3 tools/emu-harness/standin_rom.py roms/$$g.nds build/emu/sd/roms/$$g.nds && \
	  tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-rom-font.json --out build/emu/ui-rom-font-$$g && \
	  test -s build/emu/sd/_nds/verity/font.vfnt && test -s build/emu/sd/_nds/verity/icons.vicn && \
	  ls build/emu/sd/_nds/verity/items?.vitm >/dev/null && test -s build/emu/sd/_nds/verity/ribbons.vrib || exit 1; \
	  tools/emu-harness/build/emu-harness $(EMU_FONT_TESTS:%=tools/emu-harness/scripts/%.json) \
	    --needles tools/emu-harness/needles-local/$$g --out build/emu/font-$$g \
	    --junit build/emu/font-$$g/junit.xml $(if $(UPDATE),--update) || exit 1; \
	done

# DS UI tests: scripts that check needles (tools/emu-harness/needles) and wait on the app's
# verity_test_scene, each on its own copy of the SD card (build/emu/tests/<script>/sd). Report in
# build/emu/tests/junit.xml. UPDATE=1 rewrites needles that don't match (review before
# committing). With roms/platinum.nds and bios/, retail Platinum then loads ui-add's save.
EMU_TESTS := ui-start ui-edit ui-boxes ui-touch ui-ribbons ui-add ui-add-hgss ui-add-w2 ui-gifts \
  ui-gifts-w2 ui-egg ui-pal-park ui-closest-ivs ui-misc-tabs ui-bag ui-leave ui-gen5 ui-transfer
emu-test: emu-harness nds
	rm -rf build/emu/sd build/emu/sd.img build/emu/sd.img.idx
	mkdir -p build/emu && cp -r sd build/emu/sd && rm -rf build/emu/sd/_nds
	tools/emu-harness/build/emu-harness $(EMU_TESTS:%=tools/emu-harness/scripts/%.json) \
	  --out build/emu/tests --junit build/emu/tests/junit.xml $(if $(UPDATE),--update)
	@if [ -f roms/platinum.nds ] && [ -f bios/bios7.bin ]; then \
	  tools/emu-harness/build/emu-harness tools/emu-harness/scripts/pt-loads-written.json --out build/emu/pt-loads-written; \
	else echo "skip pt-loads-written: ROM or BIOS missing"; fi

# The old name for the walk-throughs, now the tests above.
emu-ui: emu-test

emu-tour: emu-harness nds
	tools/emu-harness/tour.sh
