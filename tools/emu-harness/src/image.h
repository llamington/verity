// SPDX-License-Identifier: GPL-3.0-only
// RGB images for the harness: PNG I/O, region comparison (needles), exact template search.
#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace image {

// Pixels are 0xAARRGGBB. Alpha 0 marks a pixel a template ignores; screens are fully opaque.
struct Image
{
    int w = 0, h = 0;
    std::vector<uint32_t> px;

    uint32_t at(int x, int y) const { return px[(size_t)y * w + x]; }
};

struct Rect
{
    int x = 0, y = 0, w = 0, h = 0;
};

Image read_png(const std::string& path);
void write_png(const Image& img, const std::string& path);

// Both DS screens stacked (top above bottom), 256x384, from melonDS framebuffers (0x??RRGGBB).
Image stack_screens(const uint32_t* top, const uint32_t* bottom);

// The share of pixels in `areas` that are equal in both images (1.0 = identical). The images must
// have the same size; areas are clipped to it.
double similarity(const Image& a, const Image& b, const std::vector<Rect>& areas);

// `actual` dimmed, with the pixels that differ from `want` inside `areas` in red and the areas
// outlined in yellow, for failure reports.
Image diff(const Image& actual, const Image& want, const std::vector<Rect>& areas);

// The top-left corner of the first exact match of `tmpl` inside `within` of `img`, scanning rows
// top to bottom. Template pixels with alpha 0 match anything.
std::optional<std::pair<int, int>> find(const Image& img, const Image& tmpl, Rect within);

}  // namespace image
