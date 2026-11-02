.PHONY: all nds test clean emu-harness

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
