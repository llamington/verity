"""verity's logo: the mark drawn on a 16x16 pixel grid, and the wordmark beside it.

Run `python3 -I assets/logo/mark.py` after changing GRID; it rewrites every generated file
(listed in OUTPUTS) where it is used.

N navy outline, R red shield, L its lit top and left edges, D its shaded right and bottom edges
and the check's drop shadow, C cream check, G gold sparkle, . transparent.
"""
from pathlib import Path

GRID = [
    ".NNNNNNNNNNNNNN.",
    ".NLLLLLLLLLLLLN.",
    ".NLRGRRRRRRRRDN.",
    ".NLGGGRRRRRRRDN.",
    ".NLRGRRRRRRCCDN.",
    ".NLRRRRRRRCCDDN.",
    ".NLRRRRRRCCDRDN.",
    ".NLCCRRRCCDRRDN.",
    ".NLDCCRCCDRRRDN.",
    ".NLRDCCCDRRRRDN.",
    ".NNDRDCDRRRRDNN.",
    "..NDRRDRRRRRDN..",
    "..NNDRRRRRRDNN..",
    "...NNDDRRDDNN...",
    "....NNNDDNNN....",
    "......NNNN......",
]
# The outline is mirror-symmetric (only the sparkle and check aren't), and each step joins the
# next along an edge, so the diagonals look as heavy as the straight sides.
assert all(len(r) == 16 for r in GRID) and len(GRID) == 16
_SILHOUETTE = ["".join(c if c in "N." else "R" for c in r) for r in GRID]
assert all(r == r[::-1] for r in _SILHOUETTE)
COLOURS = {
    "N": "#1F2537",
    "R": "#E3350D",
    "L": "#F0613A",
    "D": "#A82A0C",
    "C": "#F8F4EA",
    "G": "#F5B83D",
}


# Each layer covers the cells of its colour and of every colour drawn over it, so neighbouring
# colours overlap instead of meeting edge to edge: no background seams at fractional scales.
LAYERS = [("N", "NRLDCG"), ("R", "RLDCG"), ("L", "L"), ("D", "D"), ("C", "C"), ("G", "G")]


def rects(ox=0, oy=0):
    """The mark as one path per colour, painted bottom layer first."""
    out = []
    for colour, cells in LAYERS:
        d = []
        for y, row in enumerate(GRID):
            x = 0
            while x < 16:
                end = x
                while end < 16 and row[end] in cells:
                    end += 1
                if end > x:
                    d.append(f"M{x + ox} {y + oy}h{end - x}v1h{x - end}z")
                    x = end
                else:
                    x += 1
        out.append(f'<path fill="{COLOURS[colour]}" d="{"".join(d)}"/>')
    return "\n  ".join(out)


def svg(w, h, body, title="verity"):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}"'
        # No crispEdges: snapping a 16-pixel grid to a fractional display scale (125%, say)
        # rounds some columns down and others up, so one side of the outline looks thinner.
        f'><title>{title}</title>\n  {body}\n</svg>\n'
    )

def wordmark_path(text="verity", weight=700):
    """The text in Pixelify Sans as one SVG path, in font units; returns (d, bbox, upem)."""
    from fontTools.pens.boundsPen import BoundsPen
    from fontTools.pens.svgPathPen import SVGPathPen
    from fontTools.pens.transformPen import TransformPen
    from fontTools.ttLib import TTFont

    font_dir = Path(__file__).resolve().parents[2] / "node_modules/@fontsource/pixelify-sans/files"
    font = TTFont(font_dir / f"pixelify-sans-latin-{weight}-normal.woff2")
    glyphs, cmap = font.getGlyphSet(), font.getBestCmap()
    pen, bounds = SVGPathPen(glyphs), BoundsPen(glyphs)
    x = 0
    for ch in text:
        name = cmap[ord(ch)]
        # Flip y: fonts grow upwards, SVG downwards.
        glyphs[name].draw(TransformPen(pen, (1, 0, 0, -1, x, 0)))
        glyphs[name].draw(TransformPen(bounds, (1, 0, 0, -1, x, 0)))
        x += glyphs[name].width
    return pen.getCommands(), bounds.bounds, font["head"].unitsPerEm


def lockup(text_colour):
    """The mark beside the wordmark; the text's height is about 11 of the mark's 16 pixels."""
    d, (x0, y0, x1, y1), _ = wordmark_path()
    scale = 11 / (y1 - y0)
    gap = 4
    tx, ty = 16 + gap - x0 * scale, 8 - (y0 + y1) / 2 * scale
    width = 16 + gap + (x1 - x0) * scale
    body = (
        rects()
        + f'\n  <path fill="{text_colour}" transform="translate({tx:.3f} {ty:.3f}) scale({scale:.5f})" d="{d}"/>'
    )
    return svg(round(width + 0.5, 2), 16, body)



def image(scale):
    """The mark as an RGBA image, each grid pixel `scale` pixels wide."""
    from PIL import Image

    im = Image.new("RGBA", (16, 16))
    for y, row in enumerate(GRID):
        for x, c in enumerate(row):
            if c != ".":
                im.putpixel((x, y), tuple(int(COLOURS[c][i : i + 2], 16) for i in (1, 3, 5)) + (255,))
    return im.resize((16 * scale, 16 * scale), Image.NEAREST)


def ds_icon(path):
    """The DS menu icon: 32x32, 16-colour GIF, palette index 0 transparent (ndstool's rule)."""
    from PIL import Image

    rgba = image(2)
    palette = [(255, 0, 255)] + [tuple(int(v[i : i + 2], 16) for i in (1, 3, 5)) for v in COLOURS.values()]
    out = Image.new("P", (32, 32), 0)
    out.putpalette([c for rgb in palette for c in rgb] + [0] * 3 * (256 - len(palette)))
    for y in range(32):
        for x in range(32):
            r, g, b, a = rgba.getpixel((x, y))
            if a:
                out.putpixel((x, y), palette.index((r, g, b)))
    out.save(path, transparency=0)



def touch_icon(path):
    """Home-screen icon: opaque (iOS fills transparency with black), the mark centred on cream."""
    from PIL import Image

    out = Image.new("RGBA", (180, 180), (248, 244, 234, 255))
    mark = image(9)  # 144px, leaving a margin for the rounded corners iOS cuts
    out.alpha_composite(mark, (18, 18))
    out.save(path)



def ds_logo_header(path):
    """The start screen's logo for the DS UI: a 64x64 4 bpp sprite (the mark at 3x, centred) in
    1D tile order, and its palette as BGR555 (index 0 transparent)."""
    keys = list(COLOURS)
    scale, pad = 3, (64 - 48) // 2
    px = [[0] * 64 for _ in range(64)]
    for y, row in enumerate(GRID):
        for x, c in enumerate(row):
            if c != ".":
                for dy in range(scale):
                    for dx in range(scale):
                        px[pad + y * scale + dy][pad + x * scale + dx] = keys.index(c) + 1
    data = bytearray()
    for ty in range(8):
        for tx in range(8):
            for y in range(8):
                for x in range(0, 8, 2):
                    left, right = px[ty * 8 + y][tx * 8 + x], px[ty * 8 + y][tx * 8 + x + 1]
                    data.append(left | right << 4)

    def bgr555(hex_colour):
        r, g, b = (int(hex_colour[i : i + 2], 16) >> 3 for i in (1, 3, 5))
        return r | g << 5 | b << 10

    colours = [0] + [bgr555(COLOURS[k]) for k in keys]
    colours += [0] * (16 - len(colours))
    lines = [", ".join(f"0x{b:02x}" for b in data[i : i + 16]) for i in range(0, len(data), 16)]
    path.write_text(
        "// SPDX-License-Identifier: GPL-3.0-only\n"
        "// Generated by assets/logo/mark.py; don't edit. verity's mark for the start screen:\n"
        "// a 64x64 4 bpp sprite in 1D tile order and its 16 colours (0 transparent).\n"
        "#pragma once\n\n#include <stdint.h>\n\n"
        f"static const uint8_t kLogoTiles[{len(data)}] __attribute__((aligned(4))) = {{\n    "
        + ",\n    ".join(lines)
        + "\n};\n\nstatic const uint16_t kLogoColors[16] = {\n    "
        + ",\n    ".join(", ".join(f"0x{c:04x}" for c in colours[i : i + 8]) for i in (0, 8))
        + "\n};\n"
    )



def app_icon(size, scale, background=None):
    """A web app icon: the mark at `scale` centred on a size x size canvas (transparent, or
    `background` for a maskable icon, whose outer ring Android may crop away)."""
    from PIL import Image

    def write(path):
        out = Image.new("RGBA", (size, size), background or (0, 0, 0, 0))
        mark = image(scale)
        offset = (size - mark.width) // 2
        out.alpha_composite(mark, (offset, offset))
        out.save(path)

    return write


ROOT = Path(__file__).resolve().parents[2]
OUTPUTS = {
    "assets/logo/mark.svg": lambda p: p.write_text(svg(16, 16, rects())),
    "assets/logo/lockup.svg": lambda p: p.write_text(lockup(COLOURS["N"])),
    "assets/logo/lockup-dark.svg": lambda p: p.write_text(lockup(COLOURS["C"])),
    "assets/logo/mark-512.png": lambda p: image(32).save(p),
    "web/src/lib/assets/favicon.svg": lambda p: p.write_text(svg(16, 16, rects())),
    "web/static/apple-touch-icon.png": lambda p: touch_icon(p),
    "web/static/icons/icon-192.png": app_icon(192, 10),
    "web/static/icons/icon-512.png": app_icon(512, 28),
    # The maskable safe zone is the centre 80% circle; the mark (288 px) stays well inside it.
    "web/static/icons/icon-maskable-512.png": app_icon(512, 18, (248, 244, 234, 255)),
    "nds/icon.gif": ds_icon,
    "nds/include/logo.h": ds_logo_header,
}

if __name__ == "__main__":
    for rel, write in OUTPUTS.items():
        (ROOT / rel).parent.mkdir(parents=True, exist_ok=True)
        write(ROOT / rel)
        print(rel)
