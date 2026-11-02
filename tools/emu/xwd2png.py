#!/usr/bin/env python3
"""Convert an X11 XWD dump (ZPixmap, 24/32 bpp) to PNG. Usage: xwd2png.py in.xwd out.png [scale]"""
import struct, sys
from PIL import Image

def main():
    data = open(sys.argv[1], 'rb').read()
    f = struct.unpack('>25I', data[:100])
    header_size, _, _, _, w, h, _, byte_order, _, _, _, bpp, bpl = f[:13]
    ncolors = f[19]
    off = header_size + ncolors * 12
    raw = data[off:off + bpl * h]
    mode = 'BGRX' if byte_order == 0 else 'XRGB'
    if bpp == 24:
        mode = 'BGR' if byte_order == 0 else 'RGB'
    im = Image.frombuffer('RGB', (w, h), raw, 'raw', mode, bpl, 1)
    scale = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    if scale != 1:
        im = im.resize((w * scale, h * scale), Image.NEAREST)
    im.save(sys.argv[2])
    print(w, h, bpp)

main()
