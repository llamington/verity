"""verity's logo: the mark drawn on a 16x16 pixel grid, and the wordmark beside it.

Run `python3 -I assets/logo/mark.py` after changing GRID; it rewrites every generated file
(listed in OUTPUTS) where it is used.

N navy outline, R red shield, C cream check, G gold sparkle, . transparent.
"""
from pathlib import Path

GRID = [
    "......NNNN......",
    "..NNNNRRRRNNNN..",
    ".NRRRRRRRRRGRRN.",
    ".NRRRRRRRRGGGRN.",
    ".NRRRRRRRRRGRRN.",
    ".NRRRRRRRRRRRRN.",
    ".NRRRRRRRRRCCRN.",
    ".NRRRRRRRRCCCRN.",
    ".NRCCRRRRCCCRRN.",
    ".NRCCCRRCCCRRRN.",
    "..NRCCCCCCRRRN..",
    "...NRCCCCRRRN...",
    "....NRRCCRRN....",
    ".....NRRRRN.....",
    "......NRRN......",
    ".......NN.......",
]
COLOURS = {"N": "#1F2537", "R": "#E3350D", "C": "#F8F4EA", "G": "#F5B83D"}
assert len(GRID) == 16 and all(len(r) == 16 for r in GRID)


def rects(ox=0, oy=0):
    """One rect per horizontal run of a colour."""
    out = []
    for y, row in enumerate(GRID):
        x = 0
        while x < 16:
            c = row[x]
            end = x
            while end < 16 and row[end] == c:
                end += 1
            if c != ".":
                out.append(f'<rect x="{x + ox}" y="{y + oy}" width="{end - x}" height="1" fill="{COLOURS[c]}"/>')
            x = end
    return "\n  ".join(out)


def svg(w, h, body, title="verity"):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" '
        f'shape-rendering="crispEdges"><title>{title}</title>\n  {body}\n</svg>\n'
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


ROOT = Path(__file__).resolve().parents[2]
OUTPUTS = {
    "assets/logo/mark.svg": lambda p: p.write_text(svg(16, 16, rects())),
    "assets/logo/lockup.svg": lambda p: p.write_text(lockup(COLOURS["N"])),
    "assets/logo/lockup-dark.svg": lambda p: p.write_text(lockup(COLOURS["C"])),
    "assets/logo/mark-512.png": lambda p: image(32).save(p),
    "web/src/lib/assets/favicon.svg": lambda p: p.write_text(svg(16, 16, rects())),
    "web/static/apple-touch-icon.png": lambda p: touch_icon(p),
    "nds/icon.gif": ds_icon,
}

if __name__ == "__main__":
    for rel, write in OUTPUTS.items():
        write(ROOT / rel)
        print(rel)
