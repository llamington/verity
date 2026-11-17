//! The DS front end: Rust over libnds (through `blocksds`) and the core (through verity-ffi).
//! On a host the crate is empty: only the DS build has BlocksDS.

#![cfg_attr(target_os = "none", no_std)]
#![cfg(target_os = "none")]

extern crate alloc;

mod cell;
mod data;
mod display;
mod ffi;
mod hooks;
mod logo;
mod scenes;
mod text;
mod ui;

use core::fmt::Write as _;

use blocksds::video::{self, Screens};

/// The panic handler (verity-ffi's) calls this: shows the panic screen and halts.
#[unsafe(no_mangle)]
pub extern "C" fn verity_on_panic() -> ! {
    video::set_brightness(Screens::Both, 0);
    let mut console = blocksds::console::demo_init();
    video::bg_palette(blocksds::Engine::Sub).write(0, video::rgb15(16, 0, 0));
    let _ = console.write_str("\x1b[2J\x1b[10;8HVerity: core panic\n");
    loop {
        blocksds::wait_for_vblank();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main() -> i32 {
    hooks::paint_stack();
    hooks::count_vblanks();
    ui::sprites_init();
    blocksds::input::set_repeat(15, 4);
    if blocksds::fs::init_fat().is_err() {
        ui::at(ui::top(), 0, 0, b"Verity: no SD card / flashcart");
        ui::at(ui::top(), 0, 2, b"(FAT init failed)");
        ui::present();
        loop {
            blocksds::wait_for_vblank();
        }
    }
    ui::at(ui::top(), 0, 0, b"Verity: reading font and icons...");
    ui::present();
    data::rom_assets_init();
    ui::push(scenes::file_picker());
    ui::run();
    0
}
