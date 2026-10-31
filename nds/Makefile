# SPDX-License-Identifier: GPL-3.0-only

BLOCKSDS	?= /opt/blocksds/core

NAME		:= verity
GAME_TITLE	:= verity
GAME_SUBTITLE	:= Gen 4/5 save editor
GAME_AUTHOR	:= verity contributors

SOURCEDIRS	:= source
INCLUDEDIRS	:= include
GFXDIRS		:= graphics

RUST_LIB	:= ../target/armv5te-none-eabi/release/libverity_ffi.a

LIBS		:= -lverity_ffi -lnds9
LIBDIRS		:= $(BLOCKSDS)/libs/libnds $(CURDIR)/rust

include $(BLOCKSDS)/sys/default_makefiles/rom_arm9/Makefile

rust/libverity_ffi.a: FORCE
	cd .. && cargo build-ds
	@mkdir -p rust
	cp $(RUST_LIB) rust/libverity_ffi.a

$(ROM): rust/libverity_ffi.a

.PHONY: FORCE
FORCE:
