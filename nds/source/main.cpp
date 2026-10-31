// SPDX-License-Identifier: GPL-3.0-only

#include <stdio.h>
#include <string.h>

#include <nds.h>

#include "verity.h"

extern "C" void verity_on_panic(void)
{
    consoleDemoInit();
    BG_PALETTE_SUB[0] = RGB15(16, 0, 0);
    printf("\x1b[2J\x1b[10;8Hverity: core panic\n");
    while (1)
        swiWaitForVBlank();
}

int main(int argc, char **argv)
{
    consoleDemoInit();

    static const char check[] = "123456789";
    uint16_t crc = verity_crc16_ccitt((const uint8_t *)check, strlen(check));

    printf("verity\n\n");
    printf("crc16(\"%s\") = %04X\n", check, crc);
    printf("%s\n", crc == 0x29B1 ? "OK" : "MISMATCH");

    while (1)
    {
        swiWaitForVBlank();
        scanKeys();
        if (keysDown() & KEY_START)
            break;
    }
    return 0;
}
