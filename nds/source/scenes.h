// SPDX-License-Identifier: GPL-3.0-only
#pragma once

#include "ui.h"

namespace scenes {

ui::Scene* file_picker();
ui::Scene* overview();
ui::Scene* box_view();
// Add-Pokémon wizard; the result goes to (box_index, slot) (0xFF box = party).
ui::Scene* add_wizard(uint8_t box_index, uint8_t slot);

}  // namespace scenes
