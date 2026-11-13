// SPDX-License-Identifier: GPL-3.0-only
#include "image.h"

#include <algorithm>
#include <cstdio>
#include <stdexcept>

#include <png.h>

namespace image {

namespace {

struct File
{
    FILE* fp;
    explicit File(FILE* f) : fp(f) {}
    ~File()
    {
        if (fp)
            fclose(fp);
    }
};

Rect clip(Rect r, int w, int h)
{
    int x0 = std::max(r.x, 0), y0 = std::max(r.y, 0);
    int x1 = std::min(r.x + r.w, w), y1 = std::min(r.y + r.h, h);
    return {x0, y0, std::max(x1 - x0, 0), std::max(y1 - y0, 0)};
}

bool inside(int x, int y, const std::vector<Rect>& rects)
{
    for (auto& r : rects)
        if (x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h)
            return true;
    return false;
}

}  // namespace

Image read_png(const std::string& path)
{
    File f(fopen(path.c_str(), "rb"));
    if (!f.fp)
        throw std::runtime_error("cannot read " + path);
    png_structp png = png_create_read_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
    png_infop info = png_create_info_struct(png);
    if (setjmp(png_jmpbuf(png)))
    {
        png_destroy_read_struct(&png, &info, nullptr);
        throw std::runtime_error("bad PNG " + path);
    }
    png_init_io(png, f.fp);
    png_read_info(png, info);
    // Normalise every format to 8-bit RGBA.
    png_set_expand(png);
    png_set_strip_16(png);
    png_set_gray_to_rgb(png);
    png_set_add_alpha(png, 0xFF, PNG_FILLER_AFTER);
    png_read_update_info(png, info);

    Image img;
    img.w = (int)png_get_image_width(png, info);
    img.h = (int)png_get_image_height(png, info);
    std::vector<png_byte> data((size_t)img.w * img.h * 4);
    std::vector<png_bytep> rows(img.h);
    for (int y = 0; y < img.h; y++)
        rows[y] = data.data() + (size_t)y * img.w * 4;
    png_read_image(png, rows.data());
    png_destroy_read_struct(&png, &info, nullptr);

    img.px.resize((size_t)img.w * img.h);
    for (size_t i = 0; i < img.px.size(); i++)
    {
        const png_byte* p = &data[i * 4];
        img.px[i] = (uint32_t)p[3] << 24 | (uint32_t)p[0] << 16 | (uint32_t)p[1] << 8 | p[2];
    }
    return img;
}

void write_png(const Image& img, const std::string& path)
{
    File f(fopen(path.c_str(), "wb"));
    if (!f.fp)
        throw std::runtime_error("cannot write " + path);
    png_structp png = png_create_write_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
    png_infop info = png_create_info_struct(png);
    if (setjmp(png_jmpbuf(png)))
    {
        png_destroy_write_struct(&png, &info);
        throw std::runtime_error("png error writing " + path);
    }
    png_init_io(png, f.fp);
    png_set_IHDR(png, info, img.w, img.h, 8, PNG_COLOR_TYPE_RGB, PNG_INTERLACE_NONE,
                 PNG_COMPRESSION_TYPE_DEFAULT, PNG_FILTER_TYPE_DEFAULT);
    png_write_info(png, info);
    std::vector<png_byte> row((size_t)img.w * 3);
    for (int y = 0; y < img.h; y++)
    {
        for (int x = 0; x < img.w; x++)
        {
            uint32_t v = img.at(x, y);
            row[x * 3 + 0] = (v >> 16) & 0xFF;
            row[x * 3 + 1] = (v >> 8) & 0xFF;
            row[x * 3 + 2] = v & 0xFF;
        }
        png_write_row(png, row.data());
    }
    png_write_end(png, nullptr);
    png_destroy_write_struct(&png, &info);
}

Image stack_screens(const uint32_t* top, const uint32_t* bottom)
{
    Image img;
    img.w = 256;
    img.h = 384;
    img.px.resize(256 * 384);
    for (int i = 0; i < 256 * 192; i++)
    {
        img.px[i] = top[i] | 0xFF000000u;
        img.px[256 * 192 + i] = bottom[i] | 0xFF000000u;
    }
    return img;
}

double similarity(const Image& a, const Image& b, const std::vector<Rect>& areas,
                  const std::vector<Rect>& exclude)
{
    if (a.w != b.w || a.h != b.h)
        return 0.0;
    long long total = 0, same = 0;
    for (Rect r : areas)
    {
        r = clip(r, a.w, a.h);
        for (int y = r.y; y < r.y + r.h; y++)
            for (int x = r.x; x < r.x + r.w; x++)
            {
                if (inside(x, y, exclude))
                    continue;
                total++;
                same += (a.at(x, y) & 0xFFFFFF) == (b.at(x, y) & 0xFFFFFF);
            }
    }
    return total ? (double)same / total : 1.0;
}

Image diff(const Image& actual, const Image& want, const std::vector<Rect>& areas,
           const std::vector<Rect>& exclude)
{
    Image out = actual;
    for (auto& p : out.px)
    {
        uint32_t r = (p >> 16) & 0xFF, g = (p >> 8) & 0xFF, b = p & 0xFF;
        p = 0xFF000000u | (r / 3) << 16 | (g / 3) << 8 | (b / 3);
    }
    bool comparable = actual.w == want.w && actual.h == want.h;
    for (Rect r : areas)
    {
        r = clip(r, out.w, out.h);
        for (int y = r.y; y < r.y + r.h; y++)
            for (int x = r.x; x < r.x + r.w; x++)
            {
                if (inside(x, y, exclude))
                {
                    out.px[(size_t)y * out.w + x] |= 0xFF000080u;
                    continue;
                }
                bool edge = x == r.x || y == r.y || x == r.x + r.w - 1 || y == r.y + r.h - 1;
                bool differs = !comparable
                               || (actual.at(x, y) & 0xFFFFFF) != (want.at(x, y) & 0xFFFFFF);
                if (differs)
                    out.px[(size_t)y * out.w + x] = 0xFFFF2020u;
                else if (edge)
                    out.px[(size_t)y * out.w + x] = 0xFFFFD000u;
            }
    }
    return out;
}

std::optional<std::pair<int, int>> find(const Image& img, const Image& tmpl, Rect within)
{
    within = clip(within, img.w, img.h);
    for (int y = within.y; y + tmpl.h <= within.y + within.h; y++)
        for (int x = within.x; x + tmpl.w <= within.x + within.w; x++)
        {
            bool ok = true;
            for (int ty = 0; ty < tmpl.h && ok; ty++)
                for (int tx = 0; tx < tmpl.w; tx++)
                {
                    uint32_t t = tmpl.at(tx, ty);
                    if ((t >> 24) == 0)
                        continue;
                    if ((img.at(x + tx, y + ty) & 0xFFFFFF) != (t & 0xFFFFFF))
                    {
                        ok = false;
                        break;
                    }
                }
            if (ok)
                return std::make_pair(x, y);
        }
    return std::nullopt;
}

}  // namespace image
