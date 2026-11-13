// SPDX-License-Identifier: GPL-3.0-only
//
// verity DS front end. C++ over the Rust core's FFI; see docs/research/ui-stack.md.

#include <stdio.h>

#include <fat.h>
#include <nds.h>

#include "core.h"
#include "scenes.h"
#include "ui.h"
#include "verity.h"

extern "C" void verity_on_panic(void)
{
    setBrightness(3, 0);
    consoleDemoInit();
    BG_PALETTE_SUB[0] = RGB15(16, 0, 0);
    printf("\x1b[2J\x1b[10;8HVerity: core panic\n");
    while (1)
        swiWaitForVBlank();
}

int main(int argc, char **argv)
{
    (void)argc;
    (void)argv;
    ui::sprites_init();
    keysSetRepeat(15, 4);
    if (!fatInitDefault())
    {
        ui::at(ui::top(), 0, 0, "Verity: no SD card / flashcart");
        ui::at(ui::top(), 0, 2, "(FAT init failed)");
        ui::present();
        while (1)
            swiWaitForVBlank();
    }
    ui::at(ui::top(), 0, 0, "Verity: reading font and icons...");
    ui::present();
    core::rom_assets_init();
    ui::push(scenes::file_picker());
    ui::run();
    return 0;
}
