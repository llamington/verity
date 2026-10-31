# verity

A legality-aware Pokémon generation and save management engine for Gen 4 and Gen 5, featuring a portable `#![no_std]` Rust core (`pkcore`), mathematical RNG inversion, and PKHeX legality checker parity.

Designed to target Nintendo DS homebrew (`.nds`), WebAssembly, and portable embedded targets without I/O or floating point dependencies.

## Architecture

- **`crates/pkcore`**: Core logic (Rust `#![no_std] + alloc`, zero I/O, pure byte buffers)
- **`crates/pkcore-ffi`**: C ABI static library for cross-language embedding
- **`nds/`**: Nintendo DS front-end (BlocksDS / C++)
- **`tools/pkhex-oracle/`**: Reference oracle test runner powered by `PKHeX.Core`
- **`tools/emu-harness/`**: Headless melonDS emulator runner for automated verification

## Documentation

See [PLAN.md](PLAN.md) for the implementation plan, architecture specifications, and acceptance milestones.

## Credits & Licensing

- Licensed under **GPLv3**.
- Logic and legality rules ported from [PKHeX](https://github.com/kwsch/PKHeX) (kwsch and contributors).
- Memory Link research and references adapted from [BW_tool](https://github.com/suloku/BW_tool) (suloku).
