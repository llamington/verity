.PHONY: all nds test clean emu-harness emu-accept

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
